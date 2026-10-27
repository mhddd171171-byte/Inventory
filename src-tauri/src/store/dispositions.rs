//! Explicit non-sale stock movements and append-only customer return settlements.
use super::*;

pub(super) fn migrate_v2(conn:&mut Connection)->Result<(),String>{
 // Preserve SQLite rowids: FIFO replay uses the original append order.
 let tables=["products","stock_entries","sales","sale_lines","fifo_allocations","legacy_archive","migration_info"];
 let mut data=Vec::new();
 for table in tables{data.push(raw_records(conn,&format!("SELECT rowid AS _original_rowid,* FROM {table} ORDER BY rowid"))?);}
 let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
 let objects=query(&tx,"SELECT type,name FROM sqlite_master WHERE type IN ('trigger','view') AND name NOT LIKE 'sqlite_%'",|r|Ok(json!({"type":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?})))?;
 for o in objects{tx.execute_batch(&format!("DROP {} \"{}\";",o["type"].as_str().unwrap(),o["name"].as_str().unwrap().replace('"',"\"\""))).map_err(db_error)?;}
 for table in tables.into_iter().rev(){tx.execute_batch(&format!("DROP TABLE {table};")).map_err(db_error)?;}
 tx.execute_batch(SCHEMA).map_err(db_error)?;
 // Defer validation triggers until every original batch and line is restored.
 let guards=query(&tx,"SELECT name,sql FROM sqlite_master WHERE type='trigger'",|r|Ok(json!({"name":r.get::<_,String>(0)?,"sql":r.get::<_,String>(1)?})))?;
 for g in &guards{tx.execute_batch(&format!("DROP TRIGGER {};",g["name"].as_str().unwrap())).map_err(db_error)?;}
 for (table,rows) in tables.into_iter().zip(data){for mut row in rows{let map=row.as_object_mut().unwrap();let rowid=map.remove("_original_rowid").unwrap();let mut columns=vec!["rowid".to_string()];let mut values=vec![rusqlite::types::Value::Integer(rowid.as_i64().ok_or("Invalid original rowid.")?)];for (key,v) in map{columns.push(key.clone());values.push(match v{Value::Null=>rusqlite::types::Value::Null,Value::String(s)=>rusqlite::types::Value::Text(s.clone()),Value::Number(n)=>rusqlite::types::Value::Integer(n.as_i64().ok_or("Invalid ledger amount.")?),_=>return Err("Invalid database value.".into())});}let marks=vec!["?";columns.len()].join(",");tx.execute(&format!("INSERT INTO {table}({}) VALUES({marks})",columns.join(",")),rusqlite::params_from_iter(values)).map_err(db_error)?;}}
 for g in guards{tx.execute_batch(g["sql"].as_str().unwrap()).map_err(db_error)?;}
 validate_database(&tx)?;tx.commit().map_err(db_error)
}

fn purpose(line:&Value)->Result<String,String>{
 let p=optional_text(line,"purpose",20)?;let p=if p.is_empty(){"sale"}else{&p};
 if !["sale","personal","damage"].contains(&p){return Err("Choose sale, personal use or damage for this line.".into());}Ok(p.into())
}

// The caller owns the immediate transaction. A replacement and its return
// settlement therefore either commit together or leave no stock movement.
fn append(conn:&Connection,cr:Option<&str>,requested:&[(String,i64,i64,String)],time:&str)->Result<Value,String>{
 let sale=Uuid::new_v4().to_string();let mut revenue=0;let mut total_cost=Some(0);let mut ids=Vec::new();
 // Header values are finalized after all lines. No reader sees this transaction.
 conn.execute("INSERT INTO sales(id,cr_number,created_at,revenue_baisa,cost_baisa,profit_baisa) VALUES(?1,?2,?3,0,0,0)",params![sale,cr,time]).map_err(db_error)?;
 for (code,qty,price,purpose) in requested{
  let product:(String,String,String)=conn.query_row("SELECT id,name,model_description FROM products WHERE item_code=?1",[code],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|format!("No product exists with Item Code {code}."))?;
  let allocations=allocate_fifo(conn,&product.0,*qty)?;let cost=allocation_cost(&allocations)?;let amount=qty*price;revenue=checked_add(revenue,amount)?;total_cost=match(total_cost,cost){(Some(a),Some(b))=>Some(checked_add(a,b)?),_=>None};
  let id=Uuid::new_v4().to_string();conn.execute("INSERT INTO sale_lines(id,sale_id,product_id,item_code,product_name,model_description,quantity,unit_price_baisa,revenue_baisa,cost_baisa,profit_baisa) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![id,sale,product.0,code,product.1,product.2,qty,price,amount,cost,cost.map(|c|amount-c)]).map_err(db_error)?;
  conn.execute("INSERT INTO stock_out_details(line_id,purpose) VALUES(?1,?2)",params![id,purpose]).map_err(db_error)?;
  for(entry,quantity,_)in allocations{conn.execute("INSERT INTO fifo_allocations(line_id,entry_id,quantity) VALUES(?1,?2,?3)",params![id,entry,quantity]).map_err(db_error)?;}
  conn.execute("UPDATE products SET updated_at=?2 WHERE id=?1",params![product.0,time]).map_err(db_error)?;ids.push(id);
 }
 // Immutable guards are restored before committing, including on error rollback.
 conn.execute_batch("DROP TRIGGER sale_no_update;").map_err(db_error)?;
 conn.execute("UPDATE sales SET revenue_baisa=?2,cost_baisa=?3,profit_baisa=?4 WHERE id=?1",params![sale,revenue,total_cost,total_cost.map(|c|revenue-c)]).map_err(db_error)?;
 conn.execute_batch("CREATE TRIGGER sale_no_update BEFORE UPDATE ON sales BEGIN SELECT RAISE(ABORT,'Sales are permanent.'); END;").map_err(db_error)?;
 let old:i64=conn.query_row("SELECT COALESCE(SUM(revenue_baisa),0) FROM sales",[],|r|r.get(0)).map_err(db_error)?;checked_add(old,0)?;
 Ok(json!({"id":sale,"line_ids":ids,"cr_number":cr,"created_at":time,"revenue_baisa":revenue,"cost_baisa":total_cost,"profit_baisa":total_cost.map(|c|revenue-c)}))
}
impl Store{
 pub(super) fn stock_out(&mut self,payload:&Value)->Result<Value,String>{
  let lines=payload["lines"].as_array().filter(|l|!l.is_empty()&&l.len()<=100).ok_or("Add between 1 and 100 product lines.")?;
  let mut requested=Vec::new();let mut unique=HashSet::new();let mut has_sale=false;
  for l in lines{let code=numeric_reference(l,"item_code")?;if !unique.insert(code.clone()){return Err("Each product may appear once per Stock Out; combine its quantity into one line.".into());}let p=purpose(l)?;let price=if p=="sale"{has_sale=true;integer(l,"unit_price_baisa",0,MAX_PRICE)?}else{if l.get("unit_price_baisa").is_some_and(|v|!v.is_null()&&v.as_i64()!=Some(0)){return Err("Personal use and damage cannot have a selling price.".into());}0};requested.push((code,integer(l,"quantity",1,MAX_QUANTITY)?,price,p));}
  let cr=if has_sale{Some(numeric_reference(payload,"cr_number")?)}else{if !optional_text(payload,"cr_number",30)?.is_empty(){return Err("A non-sale Stock Out must not have a customer CR.".into());}None};
  let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;let result=append(&tx,cr.as_deref(),&requested,&now())?;validate_database(&tx)?;tx.commit().map_err(db_error)?;Ok(result)
 }
 pub(super) fn damaged_return(&mut self,payload:&Value)->Result<Value,String>{
  corrections::authorize(payload)?;
  let line=required_text(payload,"line_id",100)?;let resolution=required_text(payload,"resolution",20)?;
  if !["refund","replacement","pending"].contains(&resolution.as_str()){return Err("Choose refund, replacement or pending.".into());}
  let qty=integer(payload,"quantity",1,MAX_QUANTITY)?;let note=required_text(payload,"note",2000)?;
  let refund=integer(payload,"refund_baisa",0,MAX_TOTAL)?;let extra=integer(payload,"extra_baisa",0,MAX_TOTAL)?;
  if resolution=="pending"&&(refund!=0||extra!=0)||resolution=="refund"&&extra!=0{return Err("This resolution cannot include that payment.".into());}
  if refund>0&&extra>0{return Err("Enter either an additional payment or a partial refund, not both.".into());}
  let return_id=optional_text(payload,"return_id",100)?;
  let recovery=self.path.with_file_name(format!("before-damaged-return-{}.db",Uuid::new_v4().simple()));self.backup(&recovery)?;
  let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
  let original:(i64,Option<i64>,String)=tx.query_row("SELECT l.quantity,l.unit_price_baisa,s.cr_number FROM sale_lines l JOIN sales s ON s.id=l.sale_id LEFT JOIN stock_out_details d ON d.line_id=l.id WHERE l.id=?1 AND COALESCE(d.purpose,'sale') IN ('sale','replacement') AND s.cr_number IS NOT NULL",[&line],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|"Choose a customer sale with its original CR.".to_string())?;
  // A replacement return has zero recorded sale price. Settle any money on the
  // original paid sale instead; never invent an unrecorded refund entitlement.
  let already:i64=tx.query_row("SELECT COALESCE(SUM(quantity),0) FROM damaged_returns WHERE line_id=?1 AND id!=?2",params![line,return_id],|r|r.get(0)).map_err(db_error)?;
  if qty+already>original.0{return Err("Returned quantity exceeds the units still eligible for return.".into());}
  let paid=original.1.ok_or("The original selling price is unknown. Correct it before recording a refund.")?;
  if refund>qty*paid{return Err("Refund exceeds the original amount paid for the returned units.".into());}
  if !return_id.is_empty(){let existing:(String,i64,String)=tx.query_row("SELECT line_id,quantity,resolution FROM damaged_returns WHERE id=?1",[&return_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|"Pending return not found.".to_string())?;if existing!=(line.clone(),qty,"pending".into())||resolution=="pending"{return Err("Only an unresolved pending return can be settled, with its original quantity.".into());}}
  let time=now();let replacement=if resolution=="replacement"{let code=numeric_reference(payload,"replacement_item_code")?;let result=append(&tx,Some(&original.2),&[(code,qty,0,"replacement".into())],&time)?;Some(result["line_ids"][0].as_str().unwrap().to_string())}else{None};
  let resolved=if resolution=="pending"{None}else{Some(time.clone())};let id=if return_id.is_empty(){Uuid::new_v4().to_string()}else{return_id.clone()};
  if return_id.is_empty(){tx.execute("INSERT INTO damaged_returns(id,line_id,quantity,resolution,refund_baisa,extra_baisa,replacement_line_id,note,created_at,resolved_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![id,line,qty,resolution,refund,extra,replacement,note,time,resolved]).map_err(db_error)?;}else{tx.execute("UPDATE damaged_returns SET resolution=?2,refund_baisa=?3,extra_baisa=?4,replacement_line_id=?5,note=?6,resolved_at=?7 WHERE id=?1",params![id,resolution,refund,extra,replacement,note,resolved]).map_err(db_error)?;}
  let amounts=query(&tx,"SELECT refund_baisa,extra_baisa FROM damaged_returns",|r|Ok(json!({"refund":r.get::<_,i64>(0)?,"extra":r.get::<_,i64>(1)?})))?;sum_field(&amounts,"refund")?;sum_field(&amounts,"extra")?;
  tx.execute("INSERT INTO legacy_archive(kind,record_id,record_json) VALUES('damaged_return_event',?1,?2)",params![Uuid::new_v4().to_string(),json!({"return_id":id,"line_id":line,"resolution":resolution,"quantity":qty,"note":note,"refund_baisa":refund,"extra_baisa":extra,"created_at":time,"pre_edit_backup":recovery.to_string_lossy()}).to_string()]).map_err(db_error)?;
  validate_database(&tx)?;tx.commit().map_err(db_error)?;Ok(json!({"saved":true,"id":id,"pre_edit_backup":recovery.to_string_lossy()}))
 }
}

pub(super) fn validate(conn:&Connection)->Result<(),String>{
 let checks=[
 "SELECT EXISTS(SELECT 1 FROM sale_lines l JOIN sales s ON s.id=l.sale_id LEFT JOIN stock_out_details d ON d.line_id=l.id WHERE (COALESCE(d.purpose,'sale') IN ('sale','replacement') AND s.legacy=0 AND s.cr_number IS NULL) OR (d.purpose IN ('personal','damage','replacement') AND l.unit_price_baisa IS NOT 0))",
 "SELECT EXISTS(SELECT 1 FROM sales s WHERE s.cr_number IS NOT NULL AND NOT EXISTS(SELECT 1 FROM sale_lines l LEFT JOIN stock_out_details d ON d.line_id=l.id WHERE l.sale_id=s.id AND COALESCE(d.purpose,'sale') IN ('sale','replacement')))",
 "SELECT EXISTS(SELECT 1 FROM damaged_returns r JOIN sale_lines l ON l.id=r.line_id LEFT JOIN stock_out_details d ON d.line_id=l.id WHERE COALESCE(d.purpose,'sale') NOT IN ('sale','replacement') OR l.unit_price_baisa IS NULL OR r.refund_baisa>r.quantity*l.unit_price_baisa OR (SELECT SUM(quantity) FROM damaged_returns WHERE line_id=l.id)>l.quantity OR (r.refund_baisa>0 AND r.extra_baisa>0))",
 "SELECT EXISTS(SELECT 1 FROM damaged_returns r JOIN sale_lines o ON o.id=r.line_id JOIN sales os ON os.id=o.sale_id LEFT JOIN sale_lines l ON l.id=r.replacement_line_id LEFT JOIN sales s ON s.id=l.sale_id LEFT JOIN stock_out_details d ON d.line_id=l.id WHERE os.cr_number IS NULL OR (r.resolution='replacement' AND (l.id IS NULL OR l.quantity!=r.quantity OR s.cr_number IS NOT os.cr_number OR d.purpose IS NOT 'replacement')))",
 "SELECT EXISTS(SELECT 1 FROM stock_out_details d WHERE d.purpose='replacement' AND NOT EXISTS(SELECT 1 FROM damaged_returns r WHERE r.replacement_line_id=d.line_id))"
 ];
 for sql in checks{if invalid(conn,sql)?{return Err("The database contains inconsistent stock purposes or damaged returns.".into());}}
 let returns=query(conn,"SELECT refund_baisa,extra_baisa FROM damaged_returns",|r|Ok(json!({"refund":r.get::<_,i64>(0)?,"extra":r.get::<_,i64>(1)?})))?;
 let headers=query(conn,"SELECT revenue_baisa,cost_baisa FROM sales",|r|Ok(json!({"revenue":r.get::<_,Option<i64>>(0)?,"cost":r.get::<_,Option<i64>>(1)?})))?;
 checked_add(sum_field(&headers,"revenue")?,sum_field(&returns,"extra")?)?;sum_field(&headers,"cost")?;sum_field(&returns,"refund")?;
 let mut lines=query(conn,LINE_SELECT,line_row)?;annotate(conn,&mut lines)?;
 let mut months:HashMap<String,Vec<Value>>=HashMap::new();
 for line in report_lines(&lines).0{let stamp=line["created_at"].as_str().unwrap_or("");if let Some(month)=stamp.get(..7){months.entry(month.into()).or_default().push(line);}}
 for rows in months.values(){sum_field(rows,"revenue_baisa")?;sum_field(rows,"cost_baisa")?;sum_field(rows,"profit_baisa")?;}
 Ok(())
}

pub(super) fn annotate(conn:&Connection,lines:&mut [Value])->Result<(),String>{
 let details=query(conn,"SELECT line_id,purpose FROM stock_out_details",|r|Ok(json!({"id":r.get::<_,String>(0)?,"purpose":r.get::<_,String>(1)?})))?;
 let returns=query(conn,"SELECT id,line_id,quantity,resolution,refund_baisa,extra_baisa,replacement_line_id,note,created_at,resolved_at FROM damaged_returns ORDER BY rowid",|r|Ok(json!({"id":r.get::<_,String>(0)?,"line_id":r.get::<_,String>(1)?,"quantity":r.get::<_,i64>(2)?,"resolution":r.get::<_,String>(3)?,"refund_baisa":r.get::<_,i64>(4)?,"extra_baisa":r.get::<_,i64>(5)?,"replacement_line_id":r.get::<_,Option<String>>(6)?,"note":r.get::<_,String>(7)?,"created_at":r.get::<_,String>(8)?,"resolved_at":r.get::<_,Option<String>>(9)?})))?;
 for line in lines{line["purpose"]=details.iter().find(|d|d["id"]==line["id"]).map(|d|d["purpose"].clone()).unwrap_or(json!("sale"));if line["purpose"]=="personal"||line["purpose"]=="damage"{line["cr_number"]=Value::Null;}let rs:Vec<Value>=returns.iter().filter(|r|r["line_id"]==line["id"]).cloned().collect();line["returned_quantity"]=json!(sum_field(&rs,"quantity")?);line["returns"]=json!(rs);}
 Ok(())
}

pub(super) fn report_lines(lines:&[Value])->(Vec<Value>,Vec<Value>){
 let mut sales=Vec::new();let mut other=Vec::new();
 for l in lines{if l["purpose"]=="personal"||l["purpose"]=="damage"{other.push(l.clone());}else{sales.push(l.clone());}
  if let Some(rs)=l["returns"].as_array(){for r in rs{if r["resolved_at"].is_null(){continue;}let mut event=l.clone();let net=r["extra_baisa"].as_i64().unwrap()-r["refund_baisa"].as_i64().unwrap();event["id"]=json!(format!("return:{}",r["id"].as_str().unwrap()));event["purpose"]=json!("return_settlement");event["created_at"]=r["resolved_at"].clone();event["quantity"]=json!(0);event["unit_price_baisa"]=Value::Null;event["revenue_baisa"]=json!(net);event["cost_baisa"]=json!(0);event["profit_baisa"]=json!(net);event["returns"]=json!([]);event["product_name"]=json!(format!("{} — {} settlement ({} returned)",l["product_name"].as_str().unwrap_or("Item"),r["resolution"].as_str().unwrap(),r["quantity"]));sales.push(event);}}
 }(sales,other)
}
