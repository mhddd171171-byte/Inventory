//! Password-authorized deletion with recovery and FIFO consistency.
use super::*;
impl Store {
 pub(super) fn delete_record(&mut self,payload:&Value)->Result<Value,String>{
  corrections::authorize(payload)?;
  if payload.get("confirmed").and_then(Value::as_bool)!=Some(true){return Err("Confirm deletion before continuing.".into());}
  let kind=required_text(payload,"kind",20)?;let id=required_text(payload,"id",100)?;
  let before=match kind.as_str(){
   "product"=>self.product(&id)?,
   "entry"=>self.conn.query_row(&format!("{ENTRY_SELECT} WHERE e.id=?1"),[&id],entry_row).map_err(|_|"Stock entry not found.".to_string())?,
   "sale"=>{let sql=LINE_SELECT.replace(" ORDER BY s.created_at DESC,s.rowid DESC,l.rowid"," WHERE l.id=?1");self.conn.query_row(&sql,[&id],line_row).map_err(|_|"Stock Out record not found.".to_string())?},
   _=>return Err("Choose a product, stock entry or Stock Out record.".into())
  };
  if kind=="sale"{
   let blocked:i64=self.conn.query_row("SELECT (SELECT COUNT(*) FROM damaged_returns r JOIN sale_lines l ON l.id=r.line_id WHERE l.sale_id=?1 OR r.replacement_line_id=?2)+(SELECT COUNT(*) FROM stock_out_details WHERE line_id=?2 AND purpose='replacement')",params![before["sale_id"].as_str(),id],|r|r.get(0)).map_err(db_error)?;
   if blocked>0{return Err("This Stock Out has a non-sale purpose or return history. Its original record is protected; use the damaged return controls to resolve a pending return.".into());}
  }
  let recovery=self.path.with_file_name(format!("before-delete-{}-{}.db",Utc::now().format("%Y%m%d-%H%M%S"),Uuid::new_v4().simple()));
  self.backup(&recovery)?;
  let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
  if kind=="product"{
   let history:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM stock_entries WHERE product_id=?1)+(SELECT COUNT(*) FROM sale_lines WHERE product_id=?1)",[&id],|r|r.get(0)).map_err(db_error)?;
   if history>0{return Err("This product has stock or sales history. Delete its Stock Out and stock entry records first; other records will not be deleted automatically.".into());}
  }
  let guards=query(&tx,"SELECT name,sql FROM sqlite_master WHERE type='trigger' AND name IN ('product_no_delete','entry_no_delete','sale_no_delete','line_no_delete','sale_no_update','line_no_update','allocation_no_delete')",|r|Ok(json!({"name":r.get::<_,String>(0)?,"sql":r.get::<_,String>(1)?})))?;
  for guard in &guards{tx.execute_batch(&format!("DROP TRIGGER {};",guard["name"].as_str().unwrap())).map_err(db_error)?;}
  if kind!="product"{tx.execute("DELETE FROM fifo_allocations",[]).map_err(db_error)?;}
  match kind.as_str(){
   "product"=>{tx.execute("DELETE FROM products WHERE id=?1",[&id]).map_err(db_error)?;},
   "entry"=>{tx.execute("DELETE FROM stock_entries WHERE id=?1",[&id]).map_err(db_error)?;},
   "sale"=>{
    tx.execute("DELETE FROM stock_out_details WHERE line_id=?1",[&id]).map_err(db_error)?;
    tx.execute("DELETE FROM sale_lines WHERE id=?1",[&id]).map_err(db_error)?;
    tx.execute("DELETE FROM sales WHERE id=?1 AND NOT EXISTS(SELECT 1 FROM sale_lines WHERE sale_id=?1)",[before["sale_id"].as_str().unwrap()]).map_err(db_error)?;
   },_=>unreachable!()
  }
  if kind!="product"{corrections::recalculate(&tx)?;}
  for guard in &guards{tx.execute_batch(guard["sql"].as_str().unwrap()).map_err(db_error)?;}
  validate_database(&tx)?;
  let balances=query(&tx,"SELECT known_value_baisa FROM product_balances",|r|Ok(json!({"value":r.get::<_,i64>(0)?})))?;sum_field(&balances,"value")?;
  let amounts=query(&tx,"SELECT revenue_baisa,cost_baisa FROM sales",|r|Ok(json!({"revenue":r.get::<_,Option<i64>>(0)?,"cost":r.get::<_,Option<i64>>(1)?})))?;sum_field(&amounts,"revenue")?;sum_field(&amounts,"cost")?;
  let audit=json!({"kind":kind,"record_id":id,"created_at":now(),"before":before,"recovery_backup":recovery.to_string_lossy()});
  tx.execute("INSERT INTO legacy_archive(kind,record_id,record_json) VALUES('record_delete',?1,?2)",params![Uuid::new_v4().to_string(),audit.to_string()]).map_err(db_error)?;
  tx.commit().map_err(db_error)?;
  Ok(json!({"deleted":true,"pre_delete_backup":recovery.to_string_lossy()}))
 }
}
