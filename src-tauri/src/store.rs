//! Single-user SQLite persistence with an immutable FIFO ledger and validated domain operations.
mod corrections;
mod deletions;
#[cfg(test)] mod correction_tests;
mod quality;
use chrono::{Local, SecondsFormat, Utc};
use rusqlite::{backup::Backup, params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}, time::Duration};
use uuid::Uuid;
const SCHEMA:&str=include_str!("../migrations/002_single_user.sql");
const LEGACY_SCHEMA:&str=include_str!("../migrations/001_initial.sql");
const APPLICATION_ID:i64=1_230_392_653;
const MAX_QUANTITY:i64=1_000_000;
const MAX_PRICE:i64=1_000_000_000;
const MAX_TOTAL:i64=9_007_199_254_740_991; // exact in SQLite and JavaScript
pub struct Store{conn:Connection,path:PathBuf}
impl Store {
 pub fn open(path:&Path)->Result<Self,String>{
  if let Some(parent)=path.parent().filter(|p|!p.as_os_str().is_empty()){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
  let mut conn=Connection::open(path).map_err(db_error)?;configure(&conn)?;
  match version(&conn)?{
   0=>{let objects:i64=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",[],|r|r.get(0)).map_err(db_error)?;if objects!=0{return Err("This file is not an Inventory Manager database.".into());}let tx=conn.transaction().map_err(db_error)?;tx.execute_batch(SCHEMA).map_err(db_error)?;tx.commit().map_err(db_error)?;},
   1=>{validate_legacy(&conn)?;let backup=path.with_file_name(format!("before-single-user-{}-{}.db",Utc::now().format("%Y%m%d-%H%M%S"),Uuid::new_v4().simple()));copy_database(&conn,&backup)?;migrate_legacy(&mut conn,&backup)?;},
   2=>{},v=>return Err(format!("Database version {v} is unsupported. Keep your data and use a compatible application version."))
  }
  validate_database(&conn)?;Ok(Self{conn,path:path.to_path_buf()})
 }
 pub fn execute(&mut self,action:&str,payload:Value)->Result<Value,String>{
  if !payload.is_object(){return Err("Request payload must be an object.".into());}
  match action{"status"=>Ok(json!({"ready":true,"schema_version":2})),"snapshot"=>self.snapshot(&payload),"save_product"=>{if payload["product"]["id"].as_str().is_some_and(|id|!id.is_empty()){let result=self.edit_record(&json!({"kind":"product","id":payload["product"]["id"],"password":payload["password"],"changes":payload["product"]}))?;let mut product=self.product(payload["product"]["id"].as_str().unwrap())?;product["pre_edit_backup"]=result["pre_edit_backup"].clone();Ok(product)}else{self.save_product(&payload)}},"edit_record"=>self.edit_record(&payload),"delete_record"=>self.delete_record(&payload),"stock_entry"=>self.stock_entry(&payload),"stock_out"=>self.stock_out(&payload),_=>Err("Unknown application action.".into())}
 }
 /// Read-only business snapshot; a read transaction prevents mixed report/dashboard states.
 pub fn snapshot(&self,payload:&Value)->Result<Value,String>{
  let month=reporting_month(payload)?;let tx=self.conn.unchecked_transaction().map_err(db_error)?;
  let mut products=query(&tx,PRODUCT_SELECT,product_row)?;let mut entries=query(&tx,ENTRY_SELECT,entry_row)?;let mut lines=query(&tx,LINE_SELECT,line_row)?;let data_quality=quality::annotate(&tx,&mut products,&mut entries,&mut lines)?;
  let sales=query(&tx,"SELECT s.id,s.cr_number,s.created_at,s.revenue_baisa,s.cost_baisa,s.profit_baisa,s.legacy,s.legacy_reference,COALESCE(SUM(l.quantity),0) FROM sales s LEFT JOIN sale_lines l ON l.sale_id=s.id GROUP BY s.id ORDER BY s.created_at DESC,s.rowid DESC",|r|Ok(json!({"id":r.get::<_,String>(0)?,"cr_number":r.get::<_,Option<String>>(1)?,"created_at":r.get::<_,String>(2)?,"revenue_baisa":r.get::<_,Option<i64>>(3)?,"cost_baisa":r.get::<_,Option<i64>>(4)?,"profit_baisa":r.get::<_,Option<i64>>(5)?,"legacy":r.get::<_,bool>(6)?,"legacy_reference":r.get::<_,String>(7)?,"total_quantity":r.get::<_,i64>(8)?})))?;
  let monthly:Vec<Value>=lines.iter().filter(|l|l["created_at"].as_str().is_some_and(|date|date.starts_with(&month))).cloned().collect();
  let unknown=sum_field(&products,"unknown_cost_units")?;let known=sum_field(&products,"known_inventory_value_baisa")?;
  let summary=json!({"total_products":products.len(),"total_units":sum_field(&products,"quantity")?,"inventory_value_baisa":if unknown>0{None}else{Some(known)},"known_inventory_value_baisa":known,"unknown_cost_units":unknown,"monthly_units_sold":sum_field(&monthly,"quantity")?,"monthly_sales_baisa":nullable_sum(&monthly,"revenue_baisa")?,"known_monthly_sales_baisa":sum_field(&monthly,"revenue_baisa")?,"monthly_cost_baisa":nullable_sum(&monthly,"cost_baisa")?,"monthly_profit_baisa":nullable_sum(&monthly,"profit_baisa")?,"unknown_sales_lines":monthly.iter().filter(|l|l["cost_baisa"].is_null()||l["revenue_baisa"].is_null()).count()});
  let low_stock:Vec<Value>=products.iter().filter(|p|p["quantity"].as_i64().unwrap_or(0)<=2).cloned().collect();
  let monthly_summary:Vec<Value>=products.iter().map(|p|{let sold:Vec<Value>=monthly.iter().filter(|l|l["product_id"]==p["id"]).cloned().collect();Ok(json!({"product_id":p["id"],"item_code":p["item_code"],"product_name":p["name"],"quantity_sold":sum_field(&sold,"quantity")?,"quantity_remaining":p["quantity"],"cost_baisa":nullable_sum(&sold,"cost_baisa")?,"revenue_baisa":nullable_sum(&sold,"revenue_baisa")?,"profit_baisa":nullable_sum(&sold,"profit_baisa")?}))}).collect::<Result<_,String>>()?;
  let migration:Option<String>=tx.query_row("SELECT value FROM migration_info WHERE key='summary'",[],|r|r.get(0)).optional().map_err(db_error)?;tx.commit().map_err(db_error)?;
  Ok(json!({"data_quality":data_quality,"month":month,"products":products,"entries":entries,"sales":sales,"sale_lines":lines,"monthly_sales_lines":monthly,"monthly_summary":monthly_summary,"summary":summary,"low_stock":low_stock,"migration":migration.and_then(|v|serde_json::from_str::<Value>(&v).ok()).unwrap_or(json!({"legacy_products":0,"code_changes":[],"message":""}))}))
 }
 fn product(&self,id:&str)->Result<Value,String>{self.conn.query_row(&format!("{PRODUCT_SELECT} WHERE p.id=?1"),[id],product_row).map_err(db_error)}
 fn save_product(&mut self,payload:&Value)->Result<Value,String>{
  let p=payload.get("product").filter(|p|p.is_object()).ok_or("Product details are required.")?;
  let code=numeric_reference(p,"item_code")?;let name=required_text(p,"name",200)?;let description=required_text(p,"model_description",2000)?;let existing=optional_text(p,"id",100)?;let id=if existing.is_empty(){Uuid::new_v4().to_string()}else{existing.clone()};let time=now();
  if existing.is_empty(){self.conn.execute("INSERT INTO products(id,item_code,name,model_description,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)",params![id,code,name,description,time]).map_err(db_error)?;}
  else{let affected=self.conn.execute("UPDATE products SET item_code=?2,name=?3,model_description=?4,updated_at=?5 WHERE id=?1",params![id,code,name,description,time]).map_err(db_error)?;if affected==0{return Err("Product not found.".into());}}
  self.product(&id)
 }
 fn stock_entry(&mut self,payload:&Value)->Result<Value,String>{
  let code=numeric_reference(payload,"item_code")?;let quantity=integer(payload,"quantity",1,MAX_QUANTITY)?;let cost=integer(payload,"unit_cost_baisa",0,MAX_PRICE)?;
  let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
  let id:Option<String>=tx.query_row("SELECT id FROM products WHERE item_code=?1",[&code],|r|r.get(0)).optional().map_err(db_error)?;
  let id=match id{Some(id)=>id,None=>{let name=required_text(payload,"name",200)?;let description=required_text(payload,"model_description",2000)?;let id=Uuid::new_v4().to_string();let time=now();tx.execute("INSERT INTO products(id,item_code,name,model_description,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)",params![id,code,name,description,time]).map_err(db_error)?;id}};
  let known:i64=tx.query_row("SELECT COALESCE(SUM(known_value_baisa),0) FROM product_balances",[],|r|r.get(0)).map_err(db_error)?;checked_add(known,quantity*cost)?;
  let entry=Uuid::new_v4().to_string();let time=now();tx.execute("INSERT INTO stock_entries(id,product_id,quantity,unit_cost_baisa,created_at) VALUES(?1,?2,?3,?4,?5)",params![entry,id,quantity,cost,time]).map_err(db_error)?;tx.execute("UPDATE products SET updated_at=?2 WHERE id=?1",params![id,time]).map_err(db_error)?;tx.commit().map_err(db_error)?;
  self.conn.query_row(&format!("{ENTRY_SELECT} WHERE e.id=?1"),[entry],entry_row).map_err(db_error)
 }
 fn stock_out(&mut self,payload:&Value)->Result<Value,String>{
  let cr=numeric_reference(payload,"cr_number")?;let lines=payload.get("lines").and_then(Value::as_array).filter(|l|!l.is_empty()&&l.len()<=100).ok_or("Add between 1 and 100 product lines.")?;
  let mut requested=Vec::new();let mut unique=HashSet::new();
  for line in lines{let code=numeric_reference(line,"item_code")?;if !unique.insert(code.clone()){return Err("Each product may appear once per Stock Out; combine its quantity into one line.".into());}requested.push((code,integer(line,"quantity",1,MAX_QUANTITY)?,integer(line,"unit_price_baisa",0,MAX_PRICE)?));}
  let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;let mut planned=Vec::new();let mut total_revenue=0;let mut total_cost=Some(0);
  // Calculate every line before appending any records. The immediate transaction
  // serializes concurrent writes, so both processes cannot spend the same stock.
  for (code,quantity,price) in requested{
   let product:Option<(String,String,String)>=tx.query_row("SELECT id,name,model_description FROM products WHERE item_code=?1",[&code],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
   let(id,name,description)=product.ok_or_else(||format!("No product exists with Item Code {code}."))?;let allocations=allocate_fifo(&tx,&id,quantity)?;let cost=allocation_cost(&allocations)?;let revenue=quantity*price;total_revenue=checked_add(total_revenue,revenue)?;total_cost=match(total_cost,cost){(Some(a),Some(b))=>Some(checked_add(a,b)?),_=>None};planned.push((id,code,name,description,quantity,price,revenue,cost,allocations));
  }
  let old_revenue:i64=tx.query_row("SELECT COALESCE(SUM(revenue_baisa),0) FROM sales",[],|r|r.get(0)).map_err(db_error)?;checked_add(old_revenue,total_revenue)?;
  let sale=Uuid::new_v4().to_string();let time=now();let profit=total_cost.map(|c|total_revenue-c);tx.execute("INSERT INTO sales(id,cr_number,created_at,revenue_baisa,cost_baisa,profit_baisa) VALUES(?1,?2,?3,?4,?5,?6)",params![sale,cr,time,total_revenue,total_cost,profit]).map_err(db_error)?;
  for(id,code,name,description,quantity,price,revenue,cost,allocations)in planned{
   let line=Uuid::new_v4().to_string();tx.execute("INSERT INTO sale_lines(id,sale_id,product_id,item_code,product_name,model_description,quantity,unit_price_baisa,revenue_baisa,cost_baisa,profit_baisa) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![line,sale,id,code,name,description,quantity,price,revenue,cost,cost.map(|c|revenue-c)]).map_err(db_error)?;
   for a in allocations{tx.execute("INSERT INTO fifo_allocations(line_id,entry_id,quantity) VALUES(?1,?2,?3)",params![line,a.0,a.1]).map_err(db_error)?;}tx.execute("UPDATE products SET updated_at=?2 WHERE id=?1",params![id,time]).map_err(db_error)?;
  }
  tx.commit().map_err(db_error)?;Ok(json!({"id":sale,"cr_number":cr,"created_at":time,"revenue_baisa":total_revenue,"cost_baisa":total_cost,"profit_baisa":profit}))
 }
 pub fn backup(&self,destination:&Path)->Result<Value,String>{reject_same_path(&self.path,destination)?;validate_database(&self.conn)?;copy_database(&self.conn,destination)?;Ok(json!({"path":destination.to_string_lossy()}))}
 pub fn restore(&mut self,source:&Path)->Result<Value,String>{
  reject_same_path(&self.path,source)?;let incoming=Connection::open_with_flags(source,OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(|e|format!("Cannot open the backup: {e}"))?;incoming.busy_timeout(Duration::from_secs(5)).map_err(db_error)?;incoming.pragma_update(None,"trusted_schema","OFF").map_err(db_error)?;incoming.execute_batch("BEGIN;").map_err(db_error)?;
  let staging=self.path.with_file_name(format!("restore-validated-{}.db",Uuid::new_v4().simple()));
  let result=(||{match version(&incoming)?{1=>validate_legacy(&incoming)?,2=>validate_database(&incoming)?,_=>return Err("The backup has an unsupported schema version.".into())}
   // Migrate a private copy before touching live data. A failed migration is harmless.
   copy_database(&incoming,&staging)?;let mut staged=Connection::open(&staging).map_err(db_error)?;staged.pragma_update(None,"foreign_keys","ON").map_err(db_error)?;if version(&staged)?==1{migrate_legacy(&mut staged,source)?;}validate_database(&staged)?;let mut imported_products=query(&staged,PRODUCT_SELECT,product_row)?;let mut imported_entries=query(&staged,ENTRY_SELECT,entry_row)?;let mut imported_lines=query(&staged,LINE_SELECT,line_row)?;let imported_quality=quality::annotate(&staged,&mut imported_products,&mut imported_entries,&mut imported_lines)?;
   let pre=self.path.with_file_name(format!("before-restore-{}-{}.db",Utc::now().format("%Y%m%d-%H%M%S"),Uuid::new_v4().simple()));self.backup(&pre)?;
   {let backup=Backup::new(&staged,&mut self.conn).map_err(db_error)?;backup.run_to_completion(128,Duration::from_millis(5),None).map_err(db_error)?;}configure(&self.conn)?;Ok(json!({"restored":true,"pre_restore_backup":pre.to_string_lossy(),"data_quality":imported_quality}))})();
  let _=incoming.execute_batch("ROLLBACK;");drop(incoming);let _=fs::remove_file(staging);result
 }
}
const PRODUCT_SELECT:&str="SELECT p.id,p.item_code,p.name,p.model_description,p.created_at,p.updated_at,b.quantity,b.unknown_cost_units,b.known_value_baisa FROM products p JOIN product_balances b ON b.product_id=p.id";
const ENTRY_SELECT:&str="SELECT e.id,e.product_id,p.item_code,p.name,e.quantity,e.remaining_quantity,e.unit_cost_baisa,e.created_at,e.legacy FROM entry_balances e JOIN products p ON p.id=e.product_id";
const LINE_SELECT:&str="SELECT l.id,l.sale_id,l.product_id,l.item_code,l.product_name,l.model_description,l.quantity,l.unit_price_baisa,l.cost_baisa,l.revenue_baisa,l.profit_baisa,s.cr_number,s.created_at,s.legacy,s.legacy_reference FROM sale_lines l JOIN sales s ON s.id=l.sale_id ORDER BY s.created_at DESC,s.rowid DESC,l.rowid";
fn product_row(r:&rusqlite::Row<'_>)->rusqlite::Result<Value>{let quantity:i64=r.get(6)?;let unknown:i64=r.get(7)?;let known:i64=r.get(8)?;Ok(json!({"id":r.get::<_,String>(0)?,"item_code":r.get::<_,String>(1)?,"name":r.get::<_,String>(2)?,"model_description":r.get::<_,String>(3)?,"created_at":r.get::<_,String>(4)?,"updated_at":r.get::<_,String>(5)?,"quantity":quantity,"unknown_cost_units":unknown,"known_inventory_value_baisa":known,"inventory_value_baisa":if unknown>0{None}else{Some(known)},"stock_status":match quantity{0=>"out",1=>"critical",2=>"low",_=>"normal"}}))}
fn entry_row(r:&rusqlite::Row<'_>)->rusqlite::Result<Value>{Ok(json!({"id":r.get::<_,String>(0)?,"product_id":r.get::<_,String>(1)?,"item_code":r.get::<_,String>(2)?,"product_name":r.get::<_,String>(3)?,"quantity":r.get::<_,i64>(4)?,"remaining_quantity":r.get::<_,i64>(5)?,"unit_cost_baisa":r.get::<_,Option<i64>>(6)?,"created_at":r.get::<_,String>(7)?,"legacy":r.get::<_,bool>(8)?}))}
fn line_row(r:&rusqlite::Row<'_>)->rusqlite::Result<Value>{Ok(json!({"id":r.get::<_,String>(0)?,"sale_id":r.get::<_,String>(1)?,"product_id":r.get::<_,String>(2)?,"item_code":r.get::<_,String>(3)?,"product_name":r.get::<_,String>(4)?,"model_description":r.get::<_,String>(5)?,"quantity":r.get::<_,i64>(6)?,"unit_price_baisa":r.get::<_,Option<i64>>(7)?,"cost_baisa":r.get::<_,Option<i64>>(8)?,"revenue_baisa":r.get::<_,Option<i64>>(9)?,"profit_baisa":r.get::<_,Option<i64>>(10)?,"cr_number":r.get::<_,Option<String>>(11)?,"created_at":r.get::<_,String>(12)?,"legacy":r.get::<_,bool>(13)?,"legacy_reference":r.get::<_,String>(14)?}))}
type Allocation=(String,i64,Option<i64>);
fn allocate_fifo(conn:&Connection,id:&str,quantity:i64)->Result<Vec<Allocation>,String>{let mut statement=conn.prepare("SELECT e.id,b.remaining_quantity,e.unit_cost_baisa FROM stock_entries e JOIN entry_balances b ON e.id=b.id WHERE e.product_id=?1 AND b.remaining_quantity>0 ORDER BY e.created_at,e.rowid").map_err(db_error)?;let mut rows=statement.query([id]).map_err(db_error)?;let mut remaining=quantity;let mut allocations=Vec::new();while let Some(row)=rows.next().map_err(db_error)?{let available:i64=row.get(1).map_err(db_error)?;let take=available.min(remaining);allocations.push((row.get(0).map_err(db_error)?,take,row.get(2).map_err(db_error)?));remaining-=take;if remaining==0{break;}}if remaining!=0{return Err(format!("Insufficient stock: requested {quantity}, available {}.",quantity-remaining));}Ok(allocations)}
fn allocation_cost(allocations:&[Allocation])->Result<Option<i64>,String>{let mut total=0;for(_,qty,cost)in allocations{match cost{Some(cost)=>total=checked_add(total,qty*cost)?,None=>return Ok(None)}}Ok(Some(total))}
fn query<F>(conn:&Connection,sql:&str,mapper:F)->Result<Vec<Value>,String>where F:FnMut(&rusqlite::Row<'_>)->rusqlite::Result<Value>{let mut s=conn.prepare(sql).map_err(db_error)?;let result=s.query_map([],mapper).map_err(db_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)?;Ok(result)}
fn checked_add(a:i64,b:i64)->Result<i64,String>{a.checked_add(b).filter(|n|n.unsigned_abs()<=MAX_TOTAL as u64).ok_or_else(||"The amount exceeds the supported exact monetary range.".into())}
fn sum_field(rows:&[Value],key:&str)->Result<i64,String>{rows.iter().try_fold(0,|sum,row|checked_add(sum,row[key].as_i64().unwrap_or(0)))}
fn nullable_sum(rows:&[Value],key:&str)->Result<Option<i64>,String>{if rows.iter().any(|r|r[key].is_null()){Ok(None)}else{sum_field(rows,key).map(Some)}}
fn reporting_month(payload:&Value)->Result<String,String>{let m=optional_text(payload,"month",7)?;if m.is_empty(){return Ok(Local::now().format("%Y-%m").to_string());}if m.len()!=7||!m.is_ascii()||m.as_bytes()[4]!=b'-'||!m[..4].bytes().all(|c|c.is_ascii_digit())||!m[5..].bytes().all(|c|c.is_ascii_digit())||!(1..=12).contains(&m[5..].parse::<u32>().unwrap_or(0))||&m[..4]=="0000"{return Err("Choose a valid reporting month (YYYY-MM).".into());}Ok(m)}
fn numeric_reference(payload:&Value,key:&str)->Result<String,String>{let v=required_text(payload,key,30)?;if !v.bytes().all(|c|c.is_ascii_digit())||v.trim_start_matches('0').is_empty(){return Err(format!("{} must be a nonzero numeric reference.",key.replace('_'," ")));}Ok(v.trim_start_matches('0').into())}
fn optional_text(payload:&Value,key:&str,max:usize)->Result<String,String>{let v=match payload.get(key){None|Some(Value::Null)=>"",Some(Value::String(v))=>v.trim(),_=>return Err(format!("{} must be text.",key.replace('_'," ")))};if v.chars().count()>max||v.contains('\0'){return Err(format!("{} must contain at most {max} characters and no null bytes.",key.replace('_'," ")));}Ok(v.into())}
fn required_text(payload:&Value,key:&str,max:usize)->Result<String,String>{let v=optional_text(payload,key,max)?;if v.is_empty(){Err(format!("{} is required.",key.replace('_'," ")))}else{Ok(v)}}
fn integer(payload:&Value,key:&str,min:i64,max:i64)->Result<i64,String>{let v=payload.get(key).and_then(Value::as_i64).ok_or_else(||format!("{} must be a whole number.",key.replace('_'," ")))?;if !(min..=max).contains(&v){return Err(format!("{} must be between {min} and {max}.",key.replace('_'," ")));}Ok(v)}
// Persist local date/offset: monthly reports follow the repair center's calendar.
fn now()->String{Local::now().to_rfc3339_opts(SecondsFormat::Millis,false)}
fn db_error(e:rusqlite::Error)->String{let s=e.to_string();if s.contains("UNIQUE constraint failed: products.item_code"){"This Item Code already belongs to another product.".into()}else{s}}
fn version(conn:&Connection)->Result<i64,String>{conn.pragma_query_value(None,"user_version",|r|r.get(0)).map_err(db_error)}
fn configure(conn:&Connection)->Result<(),String>{conn.busy_timeout(Duration::from_secs(5)).map_err(db_error)?;conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;").map_err(db_error)}

fn copy_database(conn:&Connection,destination:&Path)->Result<(),String>{
 let parent=destination.parent().ok_or("Choose a backup directory.")?;if !parent.is_dir(){return Err("The selected backup directory does not exist.".into());}
 let temporary=parent.join(format!(".inventory-backup-{}.tmp",Uuid::new_v4().simple()));
 let result=(||{let mut target=Connection::open(&temporary).map_err(db_error)?;target.busy_timeout(Duration::from_secs(5)).map_err(db_error)?;{let backup=Backup::new(conn,&mut target).map_err(db_error)?;backup.run_to_completion(128,Duration::from_millis(5),None).map_err(db_error)?;}target.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;").map_err(db_error)?;drop(target);fs::rename(&temporary,destination).map_err(|e|format!("Cannot save the backup: {e}"))?;Ok(())})();
 if result.is_err(){let _=fs::remove_file(&temporary);}result
}
fn reject_same_path(current:&Path,selected:&Path)->Result<(),String>{
 use std::path::{Component,Prefix};
 let network=selected.components().any(|c|matches!(c,Component::Prefix(p)if matches!(p.kind(),Prefix::UNC(..)|Prefix::VerbatimUNC(..)|Prefix::DeviceNS(..))));
 if !selected.is_absolute()||network||selected.to_string_lossy().starts_with("//"){return Err("Choose a file on this computer. Network and device paths are not supported.".into());}
 let live=fs::canonicalize(current).map_err(|e|format!("Cannot locate the live database: {e}"))?;
 let target=if selected.exists(){fs::canonicalize(selected).map_err(|e|e.to_string())?}else{fs::canonicalize(selected.parent().ok_or("Choose a valid backup directory.")?).map_err(|e|e.to_string())?.join(selected.file_name().ok_or("Choose a valid backup filename.")?)};
 let live=live.to_string_lossy().to_lowercase();let target=target.to_string_lossy().to_lowercase();
 if target==live||["-wal","-shm","-journal"].iter().any(|suffix|target==format!("{live}{suffix}")){return Err("Choose a separate backup file, not the live database or one of its working files.".into());}Ok(())
}
// SQLite preserves source line endings. Normalize only CRLF; retain every
// other byte, object, constraint and trigger check.
fn schema_objects(conn:&Connection)->Result<Vec<(String,String,String)>,String>{let mut s=conn.prepare("SELECT type,name,COALESCE(sql,'') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name").map_err(db_error)?;let objects=s.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get::<_,String>(2)?.replace("\r\n","\n")))).map_err(db_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)?;Ok(objects)}
fn validate_structure(conn:&Connection,schema:&str,expected:i64)->Result<(),String>{
 let app:i64=conn.pragma_query_value(None,"application_id",|r|r.get(0)).map_err(db_error)?;
 if version(conn)?!=expected||app!=APPLICATION_ID{return Err("This is not a compatible Inventory Manager database.".into());}
 let reference=Connection::open_in_memory().map_err(db_error)?;reference.execute_batch(schema).map_err(db_error)?;
 if schema_objects(conn)?!=schema_objects(&reference)?{return Err("The database schema is damaged or incompatible. The original data has not been replaced.".into());}
 let mut s=conn.prepare("PRAGMA integrity_check").map_err(db_error)?;let checks=s.query_map([],|r|r.get::<_,String>(0)).map_err(db_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)?;
 if checks!=["ok"]{return Err("The database failed its integrity check.".into());}
 if conn.prepare("PRAGMA foreign_key_check").map_err(db_error)?.query([]).map_err(db_error)?.next().map_err(db_error)?.is_some(){return Err("The database contains broken record relationships.".into());}Ok(())
}
fn invalid(conn:&Connection,sql:&str)->Result<bool,String>{conn.query_row(sql,[],|r|r.get(0)).map_err(db_error)}
fn validate_database(conn:&Connection)->Result<(),String>{
 validate_structure(conn,SCHEMA,2)?;
 if invalid(conn,"SELECT EXISTS(SELECT 1 FROM product_balances WHERE quantity<0 OR quantity>1000000)")?{return Err("The database contains invalid stock balances.".into());}
 if invalid(conn,"SELECT EXISTS(SELECT 1 FROM entry_balances WHERE remaining_quantity<0 OR remaining_quantity>quantity)")?{return Err("The database contains invalid FIFO balances.".into());}
 if invalid(conn,"SELECT EXISTS(SELECT 1 FROM fifo_allocations a JOIN sale_lines l ON l.id=a.line_id JOIN stock_entries e ON e.id=a.entry_id WHERE e.product_id!=l.product_id)")?{return Err("The database contains inconsistent FIFO product references.".into());}
 if invalid(conn,"SELECT EXISTS(SELECT 1 FROM sale_lines l WHERE l.quantity!=COALESCE((SELECT SUM(a.quantity) FROM fifo_allocations a WHERE a.line_id=l.id),0) OR l.cost_baisa IS NOT (SELECT CASE WHEN SUM(CASE WHEN e.unit_cost_baisa IS NULL THEN 1 ELSE 0 END)>0 THEN NULL ELSE SUM(a.quantity*e.unit_cost_baisa) END FROM fifo_allocations a JOIN stock_entries e ON e.id=a.entry_id WHERE a.line_id=l.id))")?{return Err("The database contains inconsistent sale quantities or FIFO costs.".into());}
 if invalid(conn,"SELECT EXISTS(SELECT 1 FROM sale_lines l JOIN sales s ON s.id=l.sale_id WHERE s.legacy=0 AND l.unit_price_baisa IS NULL)")?{return Err("The database contains a sale without its actual selling price.".into());}
 if invalid(conn,"SELECT EXISTS(SELECT 1 FROM sales s WHERE NOT EXISTS(SELECT 1 FROM sale_lines l WHERE l.sale_id=s.id) OR s.revenue_baisa IS NOT (SELECT CASE WHEN SUM(CASE WHEN l.revenue_baisa IS NULL THEN 1 ELSE 0 END)>0 THEN NULL ELSE SUM(l.revenue_baisa) END FROM sale_lines l WHERE l.sale_id=s.id) OR s.cost_baisa IS NOT (SELECT CASE WHEN SUM(CASE WHEN l.cost_baisa IS NULL THEN 1 ELSE 0 END)>0 THEN NULL ELSE SUM(l.cost_baisa) END FROM sale_lines l WHERE l.sale_id=s.id))")?{return Err("The database contains inconsistent sale totals.".into());}
 // A tampered backup must not allocate a newer batch while an earlier batch was
 // available at that sale. Rowid records append order even for equal timestamps.
 if invalid(conn,"SELECT EXISTS(SELECT 1 FROM fifo_allocations a JOIN stock_entries e ON e.id=a.entry_id JOIN sale_lines l ON l.id=a.line_id JOIN sales s ON s.id=l.sale_id WHERE EXISTS(SELECT 1 FROM stock_entries older WHERE older.product_id=e.product_id AND (older.created_at<e.created_at OR (older.created_at=e.created_at AND older.rowid<e.rowid)) AND older.quantity>COALESCE((SELECT SUM(prior.quantity) FROM fifo_allocations prior JOIN sale_lines pl ON pl.id=prior.line_id JOIN sales ps ON ps.id=pl.sale_id WHERE prior.entry_id=older.id AND (ps.rowid<s.rowid OR (ps.rowid=s.rowid AND pl.rowid<=l.rowid))),0)))")?{return Err("The database contains inconsistent FIFO history.".into());}
 Ok(())
}
fn validate_legacy(conn:&Connection)->Result<(),String>{
 validate_structure(conn,LEGACY_SCHEMA,1)?;
 if invalid(conn,"WITH balances AS (SELECT SUM(CASE kind WHEN 'IN' THEN quantity ELSE -quantity END) OVER (PARTITION BY item_id ORDER BY rowid ROWS UNBOUNDED PRECEDING) AS balance FROM stock_movements) SELECT EXISTS(SELECT 1 FROM balances WHERE balance<0 OR balance>1000000)")?{return Err("The database contains inconsistent historical stock balances.".into());}Ok(())
}
fn raw_records(conn:&Connection,sql:&str)->Result<Vec<Value>,String>{
 let mut statement=conn.prepare(sql).map_err(db_error)?;let columns:Vec<String>=statement.column_names().iter().map(|s|s.to_string()).collect();
 let rows=statement.query_map([],|row|{let mut object=serde_json::Map::new();for(i,name)in columns.iter().enumerate(){let value=match row.get_ref(i)?{rusqlite::types::ValueRef::Null=>Value::Null,rusqlite::types::ValueRef::Integer(n)=>json!(n),rusqlite::types::ValueRef::Real(n)=>json!(n),rusqlite::types::ValueRef::Text(s)=>json!(String::from_utf8_lossy(s)),rusqlite::types::ValueRef::Blob(_)=>Value::Null};object.insert(name.clone(),value);}Ok(Value::Object(object))}).map_err(db_error)?;
 rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)
}
fn migrated_time(value:&Value)->String{let original=value.as_str().unwrap_or_default();chrono::DateTime::parse_from_rfc3339(original).map(|date|date.with_timezone(&Local).to_rfc3339_opts(SecondsFormat::Millis,false)).unwrap_or_else(|_|original.into())}
fn migrate_legacy(conn:&mut Connection,backup:&Path)->Result<(),String>{
 let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
 let items=raw_records(&tx,"SELECT * FROM items ORDER BY rowid")?;
 let movements=raw_records(&tx,"SELECT m.*,u.display_name AS recorded_by FROM stock_movements m LEFT JOIN users u ON u.id=m.user_id ORDER BY m.rowid")?;
 let orders=raw_records(&tx,"SELECT o.*,u.display_name AS recorded_by FROM repair_orders o LEFT JOIN users u ON u.id=o.user_id ORDER BY o.rowid")?;
 tx.execute_batch(SCHEMA).map_err(db_error)?;
 for(kind,records)in [("item",&items),("stock_movement",&movements),("repair_reference",&orders)]{for record in records{tx.execute("INSERT INTO legacy_archive(kind,record_id,record_json) VALUES(?1,?2,?3)",params![kind,record["id"].as_str(),record.to_string()]).map_err(db_error)?;}}
 let mut reserved=HashSet::new();for item in &items{if let Ok(code)=numeric_reference(&json!({"item_code":item["sku"]}),"item_code"){reserved.insert(code);}}
 let mut used=HashSet::new();let mut next_code=1_000_000_000u64;let mut changes=Vec::new();let mut products=HashMap::new();
 for item in &items{
  let old=item["sku"].as_str().unwrap_or_default();let candidate=numeric_reference(&json!({"item_code":old}),"item_code").ok();
  let code=if let Some(code)=candidate.filter(|code|!used.contains(code)){code}else{while reserved.contains(&next_code.to_string())||used.contains(&next_code.to_string()){next_code+=1;}let code=next_code.to_string();next_code+=1;code};used.insert(code.clone());
  let mut description=[item["compatible_device"].as_str().unwrap_or_default(),item["description"].as_str().unwrap_or_default()].into_iter().filter(|v|!v.is_empty()).collect::<Vec<_>>().join(" — ");if description.is_empty(){description=format!("Imported product (previous code: {old})");}description=description.chars().take(2000).collect();
  if code!=old{changes.push(json!({"old_code":old,"item_code":code,"product_name":item["name"]}));}
  let id=item["id"].as_str().ok_or("Invalid legacy product identifier.")?.to_string();let name=item["name"].as_str().unwrap_or_default().to_string();
  tx.execute("INSERT INTO products(id,item_code,name,model_description,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,code,name,description,migrated_time(&item["created_at"]),migrated_time(&item["updated_at"])]).map_err(db_error)?;
  products.insert(id,(code,name,description));
 }
 for movement in &movements{
  let id=movement["id"].as_str().ok_or("Invalid legacy movement identifier.")?;let product=movement["item_id"].as_str().ok_or("Invalid legacy product reference.")?;let quantity=movement["quantity"].as_i64().ok_or("Invalid legacy quantity.")?;let date=migrated_time(&movement["created_at"]);
  if movement["kind"]=="IN"{tx.execute("INSERT INTO stock_entries(id,product_id,quantity,unit_cost_baisa,created_at,legacy) VALUES(?1,?2,?3,NULL,?4,1)",params![id,product,quantity,date]).map_err(db_error)?;}
  else{
   let(code,name,description)=products.get(product).ok_or("Missing legacy product.")?;let reference=movement["order_id"].as_str().unwrap_or_default();let cr=numeric_reference(&json!({"cr_number":reference}),"cr_number").ok();let allocations=allocate_fifo(&tx,product,quantity)?;
   tx.execute("INSERT INTO sales(id,cr_number,created_at,revenue_baisa,cost_baisa,profit_baisa,legacy,legacy_reference) VALUES(?1,?2,?3,NULL,NULL,NULL,1,?4)",params![id,cr,date,reference]).map_err(db_error)?;
   tx.execute("INSERT INTO sale_lines(id,sale_id,product_id,item_code,product_name,model_description,quantity,unit_price_baisa,cost_baisa,revenue_baisa,profit_baisa) VALUES(?1,?1,?2,?3,?4,?5,?6,NULL,NULL,NULL,NULL)",params![id,product,code,name,description,quantity]).map_err(db_error)?;
   for a in allocations{tx.execute("INSERT INTO fifo_allocations(line_id,entry_id,quantity) VALUES(?1,?2,?3)",params![id,a.0,a.1]).map_err(db_error)?;}
  }
 }
 let metadata=json!({"legacy_products":items.len(),"code_changes":changes,"backup_path":backup.to_string_lossy(),"message":if items.is_empty(){""}else{"Existing products and stock history were preserved. Previous nonnumeric codes were mapped to numeric Item Codes. Historical purchase costs and actual selling prices were not recorded by the previous version and remain unknown; they are never estimated from current prices."}});
 tx.execute("INSERT INTO migration_info(key,value) VALUES('summary',?1)",[metadata.to_string()]).map_err(db_error)?;
 // Full v1 backup was secured before migration. Archive business data above;
 // deliberately remove obsolete credentials and account tables from active v2.
 tx.execute_batch("DROP TRIGGER movement_validate_balance; DROP TRIGGER movement_no_update; DROP TRIGGER movement_no_delete; DROP TRIGGER movement_touch_item; DROP TRIGGER user_preserve_last_admin; DROP TRIGGER user_no_delete; DROP TRIGGER item_no_delete; DROP TRIGGER order_no_delete; DROP VIEW stock_balances; DROP TABLE stock_movements; DROP TABLE repair_orders; DROP TABLE items; DROP TABLE users;").map_err(db_error)?;
 validate_database(&tx)?;tx.commit().map_err(db_error)?;Ok(())
}

#[cfg(test)]
mod tests{
 use super::*;
 struct Fixture{directory:tempfile::TempDir,store:Store}
 impl Fixture{
  fn new()->Self{let directory=tempfile::tempdir().unwrap();let store=Store::open(&directory.path().join("inventory.db")).unwrap();Self{directory,store}}
  fn call(&mut self,action:&str,payload:Value)->Result<Value,String>{self.store.execute(action,payload)}
  fn entry(&mut self,code:&str,quantity:i64,cost:i64)->Value{self.call("stock_entry",json!({"item_code":code,"name":format!("Screen {code} شاشة"),"model_description":"Samsung S3 LCD","quantity":quantity,"unit_cost_baisa":cost})).unwrap()}
  fn sale(&mut self,cr:&str,code:&str,quantity:i64,price:i64)->Result<Value,String>{self.call("stock_out",json!({"cr_number":cr,"lines":[{"item_code":code,"quantity":quantity,"unit_price_baisa":price}]}))}
  fn snapshot(&self)->Value{self.store.snapshot(&json!({})).unwrap()}
 }
 #[test]fn single_user_product_codes_are_numeric_unique_and_manual(){
  let mut f=Fixture::new();assert_eq!(f.call("status",json!({})).unwrap(),json!({"ready":true,"schema_version":2}));
  for action in ["login","setup","create_user","create_order","record_movement"]{assert!(f.call(action,json!({})).is_err());}
  for code in ["","0","000","-1","1.0","A12","12 OR 1=1","١٢"]{assert!(f.call("save_product",json!({"product":{"item_code":code,"name":"Display","model_description":"Phone"}})).is_err());}
  let p=f.call("save_product",json!({"product":{"item_code":"0011234","name":"iPhone display","model_description":"17 Pro Max"}})).unwrap();assert_eq!(p["item_code"],"11234");assert_eq!(p["quantity"],0);
  assert!(f.call("save_product",json!({"product":{"item_code":"11234","name":"Duplicate","model_description":"x"}})).unwrap_err().contains("already"));
  let edited=f.call("save_product",json!({"password":"1234","product":{"id":p["id"],"item_code":"11235","name":"Edited","model_description":"Updated model","quantity":999}})).unwrap();assert_eq!(edited["quantity"],0);assert_eq!(edited["item_code"],"11235");
 }
 #[test]fn fifo_costs_are_immutable_and_sale_prices_are_actual(){
  let mut f=Fixture::new();let first=f.entry("11234",5,10000);f.entry("11234",3,12000);
  let sold=f.sale("99876","11234",6,50000).unwrap();assert_eq!(sold["revenue_baisa"],300000);assert_eq!(sold["cost_baisa"],62000);assert_eq!(sold["profit_baisa"],238000);
  let s=f.snapshot();assert_eq!(s["summary"]["total_products"],1);assert_eq!(s["summary"]["total_units"],2);assert_eq!(s["summary"]["inventory_value_baisa"],24000);assert_eq!(s["summary"]["monthly_profit_baisa"],238000);
  assert_eq!(s["products"][0]["stock_status"],"low");assert!(f.store.conn.execute("UPDATE stock_entries SET unit_cost_baisa=1 WHERE id=?1",[first["id"].as_str()]).is_err());assert!(f.store.conn.execute("DELETE FROM sales",[]).is_err());validate_database(&f.store.conn).unwrap();
 }
 #[test]fn multiline_cr_transaction_is_atomic_and_repeated_cr_is_allowed(){
  let mut f=Fixture::new();f.entry("1",5,5000);f.entry("2",2,2000);
  let mut sale=json!({"cr_number":"0099876","lines":[{"item_code":"1","quantity":1,"unit_price_baisa":15000},{"item_code":"2","quantity":3,"unit_price_baisa":4000}]});
  assert!(f.call("stock_out",sale.clone()).unwrap_err().contains("Insufficient"));assert_eq!(f.snapshot()["summary"]["total_units"],7);assert_eq!(f.snapshot()["sales"].as_array().unwrap().len(),0);
  sale["lines"][1]["quantity"]=json!(2);let result=f.call("stock_out",sale).unwrap();assert_eq!(result["cr_number"],"99876");assert_eq!(result["revenue_baisa"],23000);assert_eq!(result["cost_baisa"],9000);assert_eq!(result["profit_baisa"],14000);
  f.sale("99876","1",1,0).unwrap();let s=f.snapshot();assert_eq!(s["sale_lines"].as_array().unwrap().len(),3);assert_eq!(s["sales"].as_array().unwrap().len(),2);assert_eq!(s["summary"]["monthly_profit_baisa"],9000);assert_eq!(s["summary"]["total_units"],3);
 }
 #[test]fn validation_requires_cr_price_and_positive_integer_quantities(){
  let mut f=Fixture::new();f.entry("1",3,1);
  for cr in [json!(null),json!(""),json!("0"),json!("-7"),json!("CR-12"),json!(123)]{assert!(f.call("stock_out",json!({"cr_number":cr,"lines":[{"item_code":"1","quantity":1,"unit_price_baisa":2}]})).is_err());}
  for quantity in [json!(0),json!(-1),json!(1.2),json!("1"),json!(1000001)]{assert!(f.call("stock_entry",json!({"item_code":"1","quantity":quantity,"unit_cost_baisa":1})).is_err());assert!(f.call("stock_out",json!({"cr_number":"1","lines":[{"item_code":"1","quantity":quantity,"unit_price_baisa":2}]})).is_err());}
  for price in [json!(null),json!(-1),json!(1.1),json!("2"),json!(1000000001)]{assert!(f.call("stock_out",json!({"cr_number":"1","lines":[{"item_code":"1","quantity":1,"unit_price_baisa":price}]})).is_err());}
  assert!(f.call("stock_out",json!({"cr_number":"1","lines":[{"item_code":"1","quantity":1,"unit_price_baisa":2},{"item_code":"01","quantity":1,"unit_price_baisa":2}]})).is_err());assert_eq!(f.snapshot()["summary"]["total_units"],3);
  assert!(f.call("stock_entry",json!({"item_code":"999","name":"New","quantity":1,"unit_cost_baisa":1})).is_err());assert_eq!(f.snapshot()["products"].as_array().unwrap().len(),1);
 }
 #[test]fn low_stock_thresholds_and_precise_baisa_are_correct(){
  let mut f=Fixture::new();f.entry("1",3,333);f.sale("1","1",1,1001).unwrap();assert_eq!(f.snapshot()["products"][0]["stock_status"],"low");f.sale("1","1",1,1001).unwrap();assert_eq!(f.snapshot()["products"][0]["stock_status"],"critical");f.sale("1","1",1,1001).unwrap();let s=f.snapshot();assert_eq!(s["products"][0]["stock_status"],"out");assert_eq!(s["summary"]["monthly_sales_baisa"],3003);assert_eq!(s["summary"]["monthly_cost_baisa"],999);assert_eq!(s["summary"]["monthly_profit_baisa"],2004);
 }
 #[test]fn month_reports_are_selected_without_changing_current_stock(){
  let mut f=Fixture::new();f.entry("1",4,100);f.sale("2","1",1,200).unwrap();
  let s=f.store.snapshot(&json!({"month":"2001-02"})).unwrap();assert_eq!(s["monthly_sales_lines"],json!([]));assert_eq!(s["summary"]["monthly_sales_baisa"],0);assert_eq!(s["summary"]["total_units"],3);assert_eq!(s["monthly_summary"][0]["quantity_sold"],0);
  for month in ["2026-13","2026-00","2026-1","0000-01","2026-01' OR 1=1","éééé-01"]{assert!(f.store.snapshot(&json!({"month":month})).is_err());}
 }
 #[test]fn edited_products_preserve_historical_sale_names_codes_and_prices(){
  let mut f=Fixture::new();f.entry("1",2,111);f.sale("22","1",1,333).unwrap();let p=f.snapshot()["products"][0].clone();f.call("save_product",json!({"password":"1234","product":{"id":p["id"],"item_code":"99","name":"Renamed","model_description":"New"}})).unwrap();let s=f.snapshot();assert_eq!(s["sale_lines"][0]["item_code"],"1");assert_eq!(s["sale_lines"][0]["product_name"],"Screen 1 شاشة");assert_eq!(s["monthly_summary"][0]["item_code"],"99");assert_eq!(s["monthly_summary"][0]["quantity_sold"],1);
 }
 #[test]fn concurrent_connections_cannot_spend_the_same_stock(){
  let mut f=Fixture::new();f.entry("1",5,100);let gate=std::sync::Arc::new(std::sync::Barrier::new(2));let handles:Vec<_>=(0..2).map(|_|{let gate=gate.clone();let path=f.store.path.clone();std::thread::spawn(move||{let mut store=Store::open(&path).unwrap();gate.wait();store.execute("stock_out",json!({"cr_number":"1","lines":[{"item_code":"1","quantity":4,"unit_price_baisa":200}]}))})}).collect();let results:Vec<_>=handles.into_iter().map(|h|h.join().unwrap()).collect();assert_eq!(results.iter().filter(|r|r.is_ok()).count(),1);assert_eq!(f.snapshot()["summary"]["total_units"],1);validate_database(&f.store.conn).unwrap();
 }
 #[test]fn backup_restore_keeps_wal_data_and_recoverable_pre_restore_copy(){
  let mut f=Fixture::new();f.entry("1",5,100);let backup=f.directory.path().join("backup.db");f.store.backup(&backup).unwrap();f.sale("1","1",2,200).unwrap();let restored=f.store.restore(&backup).unwrap();assert_eq!(f.snapshot()["summary"]["total_units"],5);let previous=Store::open(Path::new(restored["pre_restore_backup"].as_str().unwrap())).unwrap();assert_eq!(previous.snapshot(&json!({})).unwrap()["summary"]["total_units"],3);drop(previous);let reopened=Store::open(&f.store.path).unwrap();assert_eq!(reopened.snapshot(&json!({})).unwrap()["summary"]["inventory_value_baisa"],500);
 }
 #[test]fn restore_rejects_bad_schema_cost_tampering_and_live_sidecars(){
  let mut f=Fixture::new();f.entry("1",3,100);f.sale("1","1",1,200).unwrap();let bad=f.directory.path().join("bad.db");f.store.backup(&bad).unwrap();let conn=Connection::open(&bad).unwrap();let trigger:String=conn.query_row("SELECT sql FROM sqlite_master WHERE name='line_no_update'",[],|r|r.get(0)).unwrap();conn.execute_batch("DROP TRIGGER line_no_update;UPDATE sale_lines SET cost_baisa=50,profit_baisa=150;").unwrap();conn.execute_batch(&trigger).unwrap();drop(conn);assert!(f.store.restore(&bad).unwrap_err().contains("FIFO costs"));assert_eq!(f.snapshot()["summary"]["total_units"],2);
  f.store.backup(&bad).unwrap();let conn=Connection::open(&bad).unwrap();conn.execute_batch("DROP TRIGGER entry_no_update").unwrap();drop(conn);assert!(f.store.restore(&bad).unwrap_err().contains("schema"));
  for suffix in ["","-wal","-shm","-journal"]{let path=PathBuf::from(format!("{}{suffix}",f.store.path.display()));assert!(f.store.backup(&path).is_err());assert!(f.store.restore(&path).is_err());}
  assert!(f.store.backup(Path::new(r"\\server\share\backup.db")).is_err());
 }
 fn legacy_fixture(path:&Path){
  let conn=Connection::open(path).unwrap();conn.execute_batch(LEGACY_SCHEMA).unwrap();conn.execute("INSERT INTO users(id,username,display_name,password_hash,role,created_at) VALUES('u','olduser','Old operator','old-password-hash','admin','2025-01-01T00:00:00Z')",[]).unwrap();
  for(id,code)in [("p1","ABC-S3"),("p2","000123"),("p3","123")]{conn.execute("INSERT INTO items(id,sku,name,description,compatible_device,purchase_price_baisa,selling_price_baisa,created_at,updated_at) VALUES(?1,?2,'شاشة سامسونج S3','Replacement LCD','Samsung S3',999999,888888,'2025-01-01T00:00:00Z','2025-01-02T00:00:00Z')",params![id,code]).unwrap();}
  conn.execute("INSERT INTO repair_orders(id,device,notes,created_at,user_id) VALUES('OLD-CR','Samsung S3','Preserve repair reference','2025-01-01T00:00:00Z','u')",[]).unwrap();
  for(id,kind,qty,order)in [("e1","IN",5,None),("o1","OUT",2,Some("OLD-CR")),("e2","IN",3,None)]{conn.execute("INSERT INTO stock_movements(id,item_id,kind,quantity,created_at,order_id,user_id,notes) VALUES(?1,'p1',?2,?3,'2025-01-02T00:00:00Z',?4,'u','Preserved notes')",params![id,kind,qty,order]).unwrap();}
 }
 #[test]fn legacy_migration_preserves_business_records_without_inventing_costs(){
  let directory=tempfile::tempdir().unwrap();let path=directory.path().join("v1.db");legacy_fixture(&path);let mut store=Store::open(&path).unwrap();let s=store.snapshot(&json!({"month":"2025-01"})).unwrap();assert_eq!(s["summary"]["total_products"],3);assert_eq!(s["summary"]["total_units"],6);assert_eq!(s["summary"]["unknown_cost_units"],6);assert!(s["summary"]["inventory_value_baisa"].is_null());assert!(s["summary"]["monthly_sales_baisa"].is_null());assert!(s["sale_lines"][0]["unit_price_baisa"].is_null());assert_eq!(s["sale_lines"][0]["legacy_reference"],"OLD-CR");assert!(s["sale_lines"][0]["cr_number"].is_null());
  assert_eq!(store.conn.query_row("SELECT COUNT(*) FROM legacy_archive",[],|r|r.get::<_,i64>(0)).unwrap(),7);assert_eq!(store.conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name IN ('users','items','repair_orders','stock_movements')",[],|r|r.get::<_,i64>(0)).unwrap(),0);
  let archive:String=store.conn.query_row("SELECT group_concat(record_json) FROM legacy_archive",[],|r|r.get(0)).unwrap();assert!(!archive.contains("old-password-hash"));assert!(archive.contains("Old operator"));let original=Path::new(s["migration"]["backup_path"].as_str().unwrap());let old=Connection::open(original).unwrap();assert_eq!(version(&old).unwrap(),1);assert_eq!(old.query_row("SELECT COUNT(*) FROM items",[],|r|r.get::<_,i64>(0)).unwrap(),3);
  let code=store.product("p1").unwrap()["item_code"].as_str().unwrap().to_string();let sale=store.execute("stock_out",json!({"cr_number":"99876","lines":[{"item_code":code,"quantity":1,"unit_price_baisa":10000}]})).unwrap();assert!(sale["cost_baisa"].is_null());assert!(sale["profit_baisa"].is_null());assert_eq!(sale["revenue_baisa"],10000);validate_database(&store.conn).unwrap();
 }
 #[test]fn restoring_v1_migrates_private_copy_and_leaves_selected_backup_unchanged(){
  let mut f=Fixture::new();f.entry("1",2,50);let old=f.directory.path().join("legacy.db");legacy_fixture(&old);let source_bytes=fs::read(&old).unwrap();f.store.restore(&old).unwrap();assert_eq!(fs::read(&old).unwrap(),source_bytes);assert_eq!(f.snapshot()["summary"]["total_units"],6);assert_eq!(version(&f.store.conn).unwrap(),2);
 }

 #[test]fn schema_line_endings_are_portable_without_weakening_validation(){
  let directory=tempfile::tempdir().unwrap();
  for (index,schema) in [SCHEMA,LEGACY_SCHEMA].iter().enumerate(){
   for (suffix,sql) in [("lf",schema.replace("\r\n","\n")),("crlf",schema.replace("\r\n","\n").replace("\n","\r\n"))]{
    let path=directory.path().join(format!("schema-{index}-{suffix}.db"));
    let conn=Connection::open(&path).unwrap();conn.execute_batch(&sql).unwrap();drop(conn);
    let mut store=Store::open(&path).expect("LF/CRLF databases must open, including v1 migration");
    store.execute("stock_entry",json!({"item_code":"123","name":"Portable screen","model_description":"Opening test","quantity":7,"unit_cost_baisa":1500})).unwrap();
    let backup=directory.path().join(format!("backup-{index}-{suffix}.db"));store.backup(&backup).unwrap();
    let restored_path=directory.path().join(format!("restore-{index}-{suffix}.db"));let mut restored=Store::open(&restored_path).unwrap();
    restored.restore(&backup).expect("Restore must accept line-ending differences");
    assert_eq!(restored.snapshot(&json!({})).unwrap()["summary"]["total_units"],7);
    drop(restored);let conn=Connection::open(&restored_path).unwrap();
    conn.execute_batch("DROP TRIGGER entry_no_delete;").unwrap();drop(conn);
    assert!(Store::open(&restored_path).is_err(),"Real schema changes must still be rejected");
   }
  }
 }
}
