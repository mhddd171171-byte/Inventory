//! Durable coordination stored inside the existing validated metadata table.
use super::*;
impl Store {
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
