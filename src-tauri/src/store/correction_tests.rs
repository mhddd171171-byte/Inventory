use super::*;
fn fixture()->(tempfile::TempDir,Store){let dir=tempfile::tempdir().unwrap();let store=Store::open(&dir.path().join("inventory.db")).unwrap();(dir,store)}
fn entry(store:&mut Store,code:&str,qty:i64,cost:i64)->Value{store.execute("stock_entry",json!({"item_code":code,"name":format!("Screen {code}"),"model_description":"Test model","quantity":qty,"unit_cost_baisa":cost})).unwrap()}
fn sale(store:&mut Store,code:&str,qty:i64)->Value{store.execute("stock_out",json!({"cr_number":"55","lines":[{"item_code":code,"quantity":qty,"unit_price_baisa":500}]})).unwrap()}
fn snap(store:&Store)->Value{store.snapshot(&json!({})).unwrap()}
fn correction(kind:&str,record:&Value,changes:Value)->Value{json!({"kind":kind,"id":record["id"],"password":"1234","changes":changes})}
fn deletion(kind:&str,record:&Value)->Value{json!({"kind":kind,"id":record["id"],"password":"1234","confirmed":true})}
#[test]fn delete_requires_password_confirmation_and_existing_record(){
 let(_dir,mut store)=fixture();let e=entry(&mut store,"1",5,100);sale(&mut store,"1",1);let before=snap(&store);
 for(kind,record)in [("entry",&e),("product",&before["products"][0]),("sale",&before["sale_lines"][0])]{
  let mut p=deletion(kind,record);p["password"]=json!("wrong");assert!(store.execute("delete_record",p).is_err());
  let mut p=deletion(kind,record);p["confirmed"]=json!(false);assert!(store.execute("delete_record",p).is_err());
 }
 assert!(store.execute("delete_record",json!({"kind":"entry","id":"missing","password":"1234","confirmed":true})).is_err());assert_eq!(snap(&store),before);
}
#[test]fn delete_sale_returns_stock_preserves_other_cr_lines_and_recovery(){
 let(_dir,mut store)=fixture();entry(&mut store,"1",5,100);entry(&mut store,"2",3,200);
 store.execute("stock_out",json!({"cr_number":"55","lines":[{"item_code":"1","quantity":2,"unit_price_baisa":500},{"item_code":"2","quantity":1,"unit_price_baisa":700}]})).unwrap();
 let before=snap(&store);let line=before["sale_lines"].as_array().unwrap().iter().find(|l|l["item_code"]=="1").unwrap();
 let result=store.execute("delete_record",deletion("sale",line)).unwrap();let after=snap(&store);
 assert_eq!(after["summary"]["total_units"],7);assert_eq!(after["sale_lines"].as_array().unwrap().len(),1);assert_eq!(after["sales"][0]["revenue_baisa"],700);assert_eq!(after["sales"][0]["cost_baisa"],200);
 let recovery=Store::open(Path::new(result["pre_delete_backup"].as_str().unwrap())).unwrap();assert_eq!(snap(&recovery),before);
 store.execute("delete_record",deletion("sale",&after["sale_lines"][0])).unwrap();assert_eq!(snap(&store)["sales"],json!([]));validate_database(&store.conn).unwrap();assert!(store.conn.execute("DELETE FROM stock_entries",[]).is_err());
}
#[test]fn delete_entry_reallocates_fifo_and_rejects_shortfall_atomically(){
 let(dir,mut store)=fixture();let e=entry(&mut store,"1",3,100);let other=entry(&mut store,"1",4,200);sale(&mut store,"1",4);
 let before=snap(&store);assert!(store.execute("delete_record",deletion("entry",&other)).unwrap_err().contains("Insufficient"));assert_eq!(snap(&store),before);validate_database(&store.conn).unwrap();
 store.execute("delete_record",deletion("entry",&e)).unwrap();let after=snap(&store);assert_eq!(after["summary"]["total_units"],0);assert_eq!(after["sales"][0]["cost_baisa"],800);validate_database(&store.conn).unwrap();drop(store);assert_eq!(snap(&Store::open(&dir.path().join("inventory.db")).unwrap()),after);
}
#[test]fn delete_product_blocks_history_and_archives_after_last_entry_removed(){
 let(_dir,mut store)=fixture();let e=entry(&mut store,"1",2,100);store.conn.execute("INSERT INTO legacy_archive(kind,record_id,record_json) VALUES('spreadsheet_opening_stock',?1,'{}')",[e["product_id"].as_str().unwrap()]).unwrap();let before=snap(&store);let p=&before["products"][0];assert!(store.execute("delete_record",deletion("product",p)).unwrap_err().contains("history"));assert_eq!(snap(&store),before);
 store.execute("delete_record",deletion("entry",&e)).unwrap();store.execute("delete_record",deletion("product",p)).unwrap();assert_eq!(snap(&store)["products"],json!([]));assert_eq!(snap(&store)["data_quality"]["generated_item_codes"],0);validate_database(&store.conn).unwrap();let audits:i64=store.conn.query_row("SELECT COUNT(*) FROM legacy_archive WHERE kind='record_delete' AND record_json NOT LIKE '%password%'",[],|r|r.get(0)).unwrap();assert_eq!(audits,2);
}
#[test]fn passwords_are_enforced_by_backend_for_all_existing_records(){
 let(_dir,mut store)=fixture();let e=entry(&mut store,"1",5,100);sale(&mut store,"1",1);let s=snap(&store);let p=&s["products"][0];let l=&s["sale_lines"][0];
 for(kind,record,changes)in [("product",p,json!({"item_code":"9","name":"New","model_description":"New"})),("entry",&e,json!({"item_code":"1","quantity":8,"unit_cost_baisa":99,"created_at":e["created_at"]})),("sale",l,json!({"item_code":"1","quantity":2,"unit_price_baisa":700,"cr_number":"66","created_at":l["created_at"]}))]{
  for password in [Value::Null,json!("wrong"),json!(1234)]{let mut request=correction(kind,record,changes.clone());request["password"]=password;assert!(store.execute("edit_record",request).unwrap_err().contains("password"));assert_eq!(snap(&store),s);}
 }
 assert!(store.execute("save_product",json!({"product":{"id":p["id"],"item_code":"9","name":"Bypass","model_description":"Blocked"}})).is_err());
}
#[test]fn purchase_correction_recalculates_later_fifo_sales_and_keeps_recovery_copy(){
 let(dir,mut store)=fixture();let e=entry(&mut store,"1",5,100);entry(&mut store,"1",3,200);sale(&mut store,"1",6);let before=snap(&store);
 let result=store.execute("edit_record",correction("entry",&e,json!({"item_code":"1","quantity":7,"unit_cost_baisa":150,"created_at":e["created_at"]}))).unwrap();
 let after=snap(&store);assert_eq!(after["summary"]["total_units"],4);assert_eq!(after["sale_lines"][0]["cost_baisa"],900);assert_eq!(after["sale_lines"][0]["profit_baisa"],2100);assert_eq!(after["summary"]["inventory_value_baisa"],750);
 let recovery=Store::open(Path::new(result["pre_edit_backup"].as_str().unwrap())).unwrap();assert_eq!(snap(&recovery),before);drop(recovery);
 validate_database(&store.conn).unwrap();assert!(store.conn.execute("UPDATE stock_entries SET quantity=9",[]).is_err());drop(store);assert_eq!(snap(&Store::open(&dir.path().join("inventory.db")).unwrap()),after);
}
#[test]fn invalid_stock_correction_rolls_back_records_and_schema_guards(){
 let(_dir,mut store)=fixture();let e=entry(&mut store,"1",5,100);sale(&mut store,"1",4);let before=snap(&store);
 let error=store.execute("edit_record",correction("entry",&e,json!({"item_code":"1","quantity":2,"unit_cost_baisa":200,"created_at":e["created_at"]}))).unwrap_err();assert!(error.contains("Insufficient"));assert_eq!(snap(&store),before);validate_database(&store.conn).unwrap();assert!(store.conn.execute("DELETE FROM fifo_allocations",[]).is_err());
}
#[test]fn sale_correction_updates_quantity_cr_date_prices_and_multiline_totals_atomically(){
 let(_dir,mut store)=fixture();entry(&mut store,"1",5,100);entry(&mut store,"2",5,200);
 store.execute("stock_out",json!({"cr_number":"55","lines":[{"item_code":"1","quantity":2,"unit_price_baisa":500},{"item_code":"2","quantity":1,"unit_price_baisa":600}]})).unwrap();
 let s=snap(&store);let line=s["sale_lines"].as_array().unwrap().iter().find(|v|v["item_code"]=="1").unwrap();
 store.execute("edit_record",correction("sale",line,json!({"item_code":"2","quantity":3,"unit_price_baisa":700,"cr_number":"66","created_at":"2026-09-03T12:30:00+04:00"}))).unwrap();
 let after=snap(&store);assert_eq!(after["summary"]["total_units"],6);for l in after["sale_lines"].as_array().unwrap(){assert_eq!(l["cr_number"],"66");assert!(l["created_at"].as_str().unwrap().starts_with("2026-09-03"));}
 assert_eq!(after["sales"][0]["revenue_baisa"],2700);assert_eq!(after["sales"][0]["cost_baisa"],800);assert_eq!(after["sales"][0]["profit_baisa"],1900);validate_database(&store.conn).unwrap();
}
#[test]fn imported_unknown_values_and_generated_codes_are_flagged_until_corrected(){
 let(_dir,mut store)=fixture();let e=entry(&mut store,"900001",4,100);let id=e["product_id"].as_str().unwrap();
 store.conn.execute("INSERT INTO legacy_archive(kind,record_id,record_json) VALUES('spreadsheet_opening_stock',?1,'{}')",[id]).unwrap();
 // Simulate the existing supported opening-stock representation.
 store.conn.execute_batch("DROP TRIGGER entry_no_update;").unwrap();store.conn.execute("UPDATE stock_entries SET legacy=1,unit_cost_baisa=NULL",[]).unwrap();let reference=Connection::open_in_memory().unwrap();reference.execute_batch(SCHEMA).unwrap();let sql:String=reference.query_row("SELECT sql FROM sqlite_master WHERE name='entry_no_update'",[],|r|r.get(0)).unwrap();store.conn.execute_batch(&sql).unwrap();
 let s=snap(&store);assert_eq!(s["data_quality"]["generated_item_codes"],1);assert_eq!(s["data_quality"]["stock_entries"],1);assert!(s["summary"]["inventory_value_baisa"].is_null());let p=&s["products"][0];assert_eq!(p["data_issues"].as_array().unwrap().len(),2);
 store.execute("edit_record",correction("product",p,json!({"item_code":"123","name":"Reviewed screen","model_description":"Known model"}))).unwrap();
 let mut changes=json!({"item_code":"123","quantity":4,"unit_cost_baisa":250,"created_at":e["created_at"]});changes["unit_cost_baisa"]=json!(250);
 store.execute("edit_record",correction("entry",&e,changes)).unwrap();let after=snap(&store);assert_eq!(after["data_quality"]["generated_item_codes"],0);assert_eq!(after["data_quality"]["stock_entries"],0);assert_eq!(after["summary"]["inventory_value_baisa"],1000);validate_database(&store.conn).unwrap();
}
#[test]fn rejected_duplicate_code_bad_date_and_injection_leave_database_unchanged(){
 let(_dir,mut store)=fixture();let e=entry(&mut store,"1",5,100);entry(&mut store,"2",2,200);let s=snap(&store);let p=s["products"].as_array().unwrap().iter().find(|p|p["item_code"]=="1").unwrap();
 assert!(store.execute("edit_record",correction("product",p,json!({"item_code":"2","name":"Duplicate","model_description":"Test"}))).is_err());
 for (code,date) in [("1","bad date"),("1'; DROP TABLE products;--","2026-10-20T12:00:00Z")]{assert!(store.execute("edit_record",correction("entry",&e,json!({"item_code":code,"quantity":3,"unit_cost_baisa":20,"created_at":date}))).is_err());}
 assert_eq!(snap(&store),s);validate_database(&store.conn).unwrap();
}
