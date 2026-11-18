//! Durable coordination stored inside the existing validated metadata table.
use super::*;
thread_local!{static ONLINE_CLOCK:std::cell::RefCell<Option<String>>=const{std::cell::RefCell::new(None)};}
pub(super) fn clock()->Option<String>{ONLINE_CLOCK.with(|c|c.borrow().clone())}
impl Store {
 pub fn readonly_online_snapshot(path:&Path,payload:&Value,expected:&Value)->Result<Value,String>{
  let conn=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(db_error)?;
  conn.busy_timeout(Duration::from_secs(5)).map_err(db_error)?;conn.pragma_update(None,"trusted_schema","OFF").map_err(db_error)?;
  validate_database(&conn)?;let store=Self{conn,path:path.into()};if store.online_metadata()?!=*expected{return Err("Local backup does not match the current online revision.".into());}store.snapshot(payload)
 }
 pub fn execute_online_at(&mut self,action:&str,payload:Value,timestamp:&str)->Result<Value,String>{
  let parsed=chrono::DateTime::parse_from_rfc3339(timestamp).map_err(|_|"Invalid online database clock.")?;
  let timestamp=parsed.with_timezone(&chrono::FixedOffset::east_opt(4*3600).unwrap()).to_rfc3339_opts(SecondsFormat::Millis,false);
  struct Reset(Option<String>);impl Drop for Reset{fn drop(&mut self){ONLINE_CLOCK.with(|c|*c.borrow_mut()=self.0.take());}}
  let _reset=Reset(ONLINE_CLOCK.with(|c|c.replace(Some(timestamp))));self.execute(action,payload)
 }
 pub fn online_schema(&self)->Result<Value,String>{Ok(json!(schema_objects(&self.conn)?))}
 pub fn online_dump(&self)->Result<Value,String>{
  let mut tables=Vec::new();
  for name in ONLINE_TABLES {
   let mut statement=self.conn.prepare(&format!("SELECT rowid,* FROM {name} ORDER BY rowid")).map_err(db_error)?;
   let columns:Vec<String>=statement.column_names().iter().map(|s|s.to_string()).collect();
   let rows=statement.query_map([],|r|{let mut values=Vec::new();for i in 0..columns.len(){let value:rusqlite::types::Value=r.get(i)?;values.push(match value{rusqlite::types::Value::Null=>json!({"type":"null"}),rusqlite::types::Value::Integer(n)=>json!({"type":"integer","value":n.to_string()}),rusqlite::types::Value::Text(s)=>json!({"type":"text","value":s}),_=>return Err(rusqlite::Error::InvalidQuery)});}Ok(values)}).map_err(db_error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)?;
   tables.push(json!({"name":name,"columns":columns,"rows":rows}));
  }
  Ok(json!(tables))
 }
 /// Load a consistent cloud read into private temporary staging, never a local
 /// inventory fallback. The caller has already verified the remote schema.
 pub fn online_import(&mut self,dump:&Value)->Result<(),String>{
  let expected=self.online_dump()?;let input=dump.as_array().ok_or("Invalid inventory snapshot.")?;
  if input.len()!=ONLINE_TABLES.len(){return Err("Incomplete inventory snapshot.".into());}
  let tx=self.conn.transaction().map_err(db_error)?;
  let guards=query(&tx,"SELECT name,sql FROM sqlite_master WHERE type='trigger' ORDER BY name",|r|Ok(json!({"name":r.get::<_,String>(0)?,"sql":r.get::<_,String>(1)?})))?;
  for guard in &guards{tx.execute_batch(&format!("DROP TRIGGER {};",guard["name"].as_str().unwrap())).map_err(db_error)?;}
  for (index,table) in input.iter().enumerate(){
   if table["name"]!=ONLINE_TABLES[index]||table["columns"]!=expected[index]["columns"]{return Err("Inventory columns do not match the supported schema.".into());}
   let columns=table["columns"].as_array().ok_or("Invalid inventory columns.")?;
   let names=columns.iter().map(|c|format!("\"{}\"",c.as_str().unwrap().replace('"',"\"\""))).collect::<Vec<_>>().join(",");
   let sql=format!("INSERT INTO {}({names}) VALUES({})",ONLINE_TABLES[index],vec!["?";columns.len()].join(","));
   for row in table["rows"].as_array().ok_or("Missing inventory rows.")?{let values=row.as_array().ok_or("Invalid inventory row.")?;if values.len()!=columns.len(){return Err("Incomplete inventory row.".into());}let values=values.iter().map(protocol_value).collect::<Result<Vec<_>,_>>()?;tx.execute(&sql,rusqlite::params_from_iter(values)).map_err(db_error)?;}
  }
  for guard in guards{tx.execute_batch(guard["sql"].as_str().unwrap()).map_err(db_error)?;}
  validate_database(&tx)?;tx.commit().map_err(db_error)
 }
 pub fn online_initialize(&mut self)->Result<(),String>{
  let tx=self.conn.transaction().map_err(db_error)?;
  tx.execute("INSERT OR IGNORE INTO migration_info(key,value) VALUES('online_epoch',?1)",[Uuid::new_v4().to_string()]).map_err(db_error)?;
  tx.execute("INSERT OR IGNORE INTO migration_info(key,value) VALUES('online_revision','0')",[]).map_err(db_error)?;
  tx.commit().map_err(db_error)
 }
 pub fn online_metadata(&self)->Result<Value,String>{let get=|key:&str|self.conn.query_row("SELECT value FROM migration_info WHERE key=?1",[key],|r|r.get::<_,String>(0)).map_err(db_error);Ok(json!({"epoch":get("online_epoch")?,"revision":get("online_revision")?.parse::<u64>().map_err(|_|"Invalid revision.")?}))}
 pub fn online_receipt(&self,id:&str)->Result<Option<Value>,String>{let row:Option<String>=self.conn.query_row("SELECT value FROM migration_info WHERE key=?1",[format!("online_receipt:{id}")],|r|r.get(0)).optional().map_err(db_error)?;row.map(|v|serde_json::from_str(&v).map_err(|e|e.to_string())).transpose()}
 pub fn online_record(&mut self,id:&str,request:&str,response:&Value,revision:u64)->Result<(),String>{let tx=self.conn.transaction().map_err(db_error)?;tx.execute("INSERT INTO migration_info(key,value) VALUES(?1,?2)",params![format!("online_receipt:{id}"),json!({"request":request,"response":response}).to_string()]).map_err(db_error)?;tx.execute("UPDATE migration_info SET value=?1 WHERE key='online_revision'",[revision.to_string()]).map_err(db_error)?;tx.commit().map_err(db_error)}
 pub fn online_reset_epoch(&mut self,revision:u64)->Result<(),String>{let tx=self.conn.transaction().map_err(db_error)?;tx.execute("DELETE FROM migration_info WHERE key LIKE 'online_receipt:%' OR key IN ('online_epoch','online_revision')",[]).map_err(db_error)?;tx.execute("INSERT INTO migration_info(key,value) VALUES('online_epoch',?1)",[Uuid::new_v4().to_string()]).map_err(db_error)?;tx.execute("INSERT INTO migration_info(key,value) VALUES('online_revision',?1)",[revision.to_string()]).map_err(db_error)?;tx.commit().map_err(db_error)}
 pub fn online_publish(&mut self,path:&Path)->Result<(),String>{let source=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(db_error)?;validate_database(&source)?;let backup=Backup::new(&source,&mut self.conn).map_err(db_error)?;backup.run_to_completion(128,Duration::from_millis(1),None).map_err(db_error)}
}
pub const ONLINE_TABLES:[&str;9]=["products","stock_entries","sales","sale_lines","fifo_allocations","legacy_archive","migration_info","stock_out_details","damaged_returns"];
fn protocol_value(value:&Value)->Result<rusqlite::types::Value,String>{match value["type"].as_str(){Some("null")=>Ok(rusqlite::types::Value::Null),Some("text")=>Ok(rusqlite::types::Value::Text(value["value"].as_str().ok_or("Invalid text value.")?.into())),Some("integer")=>Ok(rusqlite::types::Value::Integer(value["value"].as_str().ok_or("Invalid integer value.")?.parse().map_err(|_|"Invalid integer value.")?)),_=>Err("Unsupported inventory value type.".into())}}
