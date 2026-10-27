//! Explicit, password-authorized corrections. The original schema remains intact.
use super::*;

pub(super) fn authorize(payload:&Value)->Result<(),String>{
 if payload.get("password").and_then(Value::as_str)!=Some("1234"){return Err("Incorrect edit password. No records were changed.".into());}Ok(())
}
fn date(payload:&Value)->Result<String,String>{
 let value=required_text(payload,"created_at",64)?;
 let parsed=chrono::DateTime::parse_from_rfc3339(&value).map_err(|_|"Enter a valid date and time.".to_string())?;
 if !(1..=9999).contains(&chrono::Datelike::year(&parsed)){return Err("Enter a valid year.".into());}
 Ok(parsed.with_timezone(&Local).to_rfc3339_opts(SecondsFormat::Millis,false))
}
impl Store {
 pub(super) fn edit_record(&mut self,payload:&Value)->Result<Value,String>{
  authorize(payload)?;
  let kind=required_text(payload,"kind",20)?;let id=required_text(payload,"id",100)?;
  let changes=payload.get("changes").filter(|v|v.is_object()).ok_or("Correction details are required.")?;
  let before=match kind.as_str(){
   "product"=>self.product(&id)?,
   "entry"=>self.conn.query_row(&format!("{ENTRY_SELECT} WHERE e.id=?1"),[&id],entry_row).map_err(db_error)?,
   "sale"=>Value::Null,
   _=>return Err("Choose a product, stock entry or Stock Out record.".into())
  };
  // Validate all user input before creating a recovery copy or changing anything.
  let code=numeric_reference(changes,"item_code")?;
  let name=if kind=="product"{Some(required_text(changes,"name",200)?)}else{None};
  let description=if kind=="product"{Some(required_text(changes,"model_description",2000)?)}else{None};
  let quantity=if kind!="product"{Some(integer(changes,"quantity",1,MAX_QUANTITY)?)}else{None};
  let cost=if kind=="entry"{Some(integer(changes,"unit_cost_baisa",0,MAX_PRICE)?)}else{None};
  let non_sale=kind=="sale"&&self.conn.query_row("SELECT purpose IN ('personal','damage') FROM stock_out_details WHERE line_id=?1",[&id],|r|r.get::<_,bool>(0)).optional().map_err(db_error)?.unwrap_or(false);
  let price=if kind=="sale"{Some(if non_sale{if changes["unit_price_baisa"].as_i64()!=Some(0){return Err("Non-sale lines cannot have a selling price.".into());}0}else{integer(changes,"unit_price_baisa",0,MAX_PRICE)?})}else{None};
  let cr=if kind=="sale"&&!non_sale{Some(numeric_reference(changes,"cr_number")?)}else{None};
  let timestamp=if kind!="product"{Some(date(changes)?)}else{None};
  let before=if kind=="sale"{
   let sql=LINE_SELECT.replace(" ORDER BY s.created_at DESC,s.rowid DESC,l.rowid"," WHERE l.id=?1");
   self.conn.query_row(&sql,[&id],line_row).map_err(|_|"Stock Out record not found.".to_string())?
  }else{before};
  if kind=="sale"{
   let blocked:i64=self.conn.query_row("SELECT (SELECT COUNT(*) FROM damaged_returns r JOIN sale_lines l ON l.id=r.line_id WHERE l.sale_id=?1 OR r.replacement_line_id=?2)+(SELECT COUNT(*) FROM stock_out_details WHERE line_id=?2 AND purpose='replacement')",params![before["sale_id"].as_str(),id],|r|r.get(0)).map_err(db_error)?;
   if blocked>0{return Err("This Stock Out has a non-sale purpose or return history. Its original record is protected; use the damaged return controls to resolve a pending return.".into());}
  }
  let recovery=self.path.with_file_name(format!("before-edit-{}-{}.db",Utc::now().format("%Y%m%d-%H%M%S"),Uuid::new_v4().simple()));
  self.backup(&recovery)?;
  let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
  // These guards are removed only inside this transaction and recreated verbatim
  // before validation/commit. Any error rolls back both records and guards.
  let guards=query(&tx,"SELECT name,sql FROM sqlite_master WHERE type='trigger' AND name IN ('entry_no_update','sale_no_update','line_no_update','allocation_no_delete')",|r|Ok(json!({"name":r.get::<_,String>(0)?,"sql":r.get::<_,String>(1)?})))?;
  for guard in &guards{let name=guard["name"].as_str().unwrap();tx.execute_batch(&format!("DROP TRIGGER {name};")).map_err(db_error)?;}
  match kind.as_str(){
   "product"=>{
    tx.execute("UPDATE products SET item_code=?2,name=?3,model_description=?4,updated_at=?5 WHERE id=?1",params![id,code,name,description,now()]).map_err(db_error)?;
    tx.execute("INSERT OR REPLACE INTO migration_info(key,value) VALUES(?1,'true')",[format!("reviewed-item-code:{id}")]).map_err(db_error)?;
   },
   "entry"=>{
    let product:String=tx.query_row("SELECT id FROM products WHERE item_code=?1",[&code],|r|r.get(0)).map_err(|_|"No product exists with that Item Code.".to_string())?;
    tx.execute("UPDATE stock_entries SET product_id=?2,quantity=?3,unit_cost_baisa=?4,created_at=?5 WHERE id=?1",params![id,product,quantity,cost,timestamp]).map_err(db_error)?;
   },
   "sale"=>{
    let product:(String,String,String)=tx.query_row("SELECT id,name,model_description FROM products WHERE item_code=?1",[&code],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|"No product exists with that Item Code.".to_string())?;
    let qty=quantity.unwrap();let revenue=qty*price.unwrap();
    tx.execute("UPDATE sale_lines SET product_id=?2,item_code=?3,product_name=?4,model_description=?5,quantity=?6,unit_price_baisa=?7,revenue_baisa=?8,profit_baisa=?8-cost_baisa WHERE id=?1",params![id,product.0,code,product.1,product.2,qty,price,revenue]).map_err(db_error)?;
    if non_sale{tx.execute("UPDATE sales SET created_at=?2 WHERE id=?1",params![before["sale_id"].as_str(),timestamp]).map_err(db_error)?;}else{tx.execute("UPDATE sales SET cr_number=?2,created_at=?3 WHERE id=?1",params![before["sale_id"].as_str(),cr,timestamp]).map_err(db_error)?;}
   },_=>unreachable!()
  }
  // Purchase corrections can affect later sales. Rebuild allocations in the
  // ledger's original recording order, preserving unknown legacy amounts.
  if kind!="product"{recalculate(&tx)?;}
  for guard in &guards{tx.execute_batch(guard["sql"].as_str().unwrap()).map_err(db_error)?;}
  validate_database(&tx)?;
  let balances=query(&tx,"SELECT known_value_baisa FROM product_balances",|r|Ok(json!({"value":r.get::<_,i64>(0)?})))?;sum_field(&balances,"value")?;
  let amounts=query(&tx,"SELECT revenue_baisa,cost_baisa FROM sales",|r|Ok(json!({"revenue":r.get::<_,Option<i64>>(0)?,"cost":r.get::<_,Option<i64>>(1)?})))?;sum_field(&amounts,"revenue")?;sum_field(&amounts,"cost")?;
  let audit=json!({"kind":kind,"record_id":id,"created_at":now(),"before":before,"changes":changes,"recovery_backup":recovery.to_string_lossy()});
  tx.execute("INSERT INTO legacy_archive(kind,record_id,record_json) VALUES('record_edit',?1,?2)",params![Uuid::new_v4().to_string(),audit.to_string()]).map_err(db_error)?;
  tx.commit().map_err(db_error)?;
  Ok(json!({"saved":true,"pre_edit_backup":recovery.to_string_lossy(),"message":"Correction saved. Stock balances, FIFO costs and report totals were recalculated."}))
 }
}
pub(super) fn recalculate(conn:&Connection)->Result<(),String>{
 conn.execute("DELETE FROM fifo_allocations",[]).map_err(db_error)?;
 let lines=query(conn,"SELECT l.id,l.product_id,l.quantity,l.unit_price_baisa FROM sale_lines l JOIN sales s ON s.id=l.sale_id ORDER BY s.rowid,l.rowid",|r|Ok(json!({"id":r.get::<_,String>(0)?,"product":r.get::<_,String>(1)?,"quantity":r.get::<_,i64>(2)?,"price":r.get::<_,Option<i64>>(3)?})))?;
 for line in lines{
  let allocations=allocate_fifo(conn,line["product"].as_str().unwrap(),line["quantity"].as_i64().unwrap())?;
  let cost=allocation_cost(&allocations)?;let revenue=line["price"].as_i64().map(|p|p*line["quantity"].as_i64().unwrap());
  let profit=revenue.zip(cost).map(|(r,c)|r-c);
  conn.execute("UPDATE sale_lines SET cost_baisa=?2,revenue_baisa=?3,profit_baisa=?4 WHERE id=?1",params![line["id"].as_str(),cost,revenue,profit]).map_err(db_error)?;
  for(entry,quantity,_)in allocations{conn.execute("INSERT INTO fifo_allocations(line_id,entry_id,quantity) VALUES(?1,?2,?3)",params![line["id"].as_str(),entry,quantity]).map_err(db_error)?;}
 }
 conn.execute_batch("UPDATE sales SET revenue_baisa=(SELECT CASE WHEN COUNT(*)!=COUNT(revenue_baisa) THEN NULL ELSE SUM(revenue_baisa) END FROM sale_lines WHERE sale_id=sales.id),cost_baisa=(SELECT CASE WHEN COUNT(*)!=COUNT(cost_baisa) THEN NULL ELSE SUM(cost_baisa) END FROM sale_lines WHERE sale_id=sales.id),profit_baisa=(SELECT CASE WHEN COUNT(*)!=COUNT(profit_baisa) THEN NULL ELSE SUM(profit_baisa) END FROM sale_lines WHERE sale_id=sales.id);").map_err(db_error)?;
 Ok(())
}
