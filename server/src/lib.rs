//! Single-writer online gateway for the same validated desktop domain engine.
#[path = "../../src-tauri/src/store/mod.rs"]
pub mod store;
use serde_json::{json, Value};
use std::{fs, path::{Path,PathBuf}};
use uuid::Uuid;
use fs2::FileExt;
use sha2::{Digest,Sha256};

pub const WRITES: &[&str] = &["save_product","stock_entry","stock_out","edit_record","delete_record","damaged_return","warranty"];
pub struct Service { store:store::Store, directory:PathBuf, _lock:fs::File }
impl Service {
 pub fn open(directory:&Path)->Result<Self,String>{
  fs::create_dir_all(directory).map_err(|e|e.to_string())?;
  let lock=fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(directory.join("server.lock")).map_err(|e|e.to_string())?;lock.try_lock_exclusive().map_err(|_|"Another inventory server already owns this database.")?;
  let mut store=store::Store::open(&directory.join("inventory.db"))?;
  store.online_initialize()?;
  Ok(Self{store,directory:directory.into(),_lock:lock})
 }
 pub fn state(&mut self)->Result<Value,String>{self.store.online_metadata()}
 pub fn snapshot(&mut self,payload:Value)->Result<Value,String>{
  let result=self.store.execute("snapshot",payload)?;
  Ok(json!({"result":result,"state":self.state()?}))
 }
 /// Build a private validated database with the business change AND receipt.
 /// Publish through SQLite's atomic backup transaction, while holding the sole
 /// service owner. Neither readers nor downloads can observe an intermediate DB.
 pub fn mutate(&mut self,request:&Value)->Result<Value,String>{
  let action=request["action"].as_str().ok_or("Missing action.")?;
  if !WRITES.contains(&action){return Err("Operation is not permitted.".into());}
  let id=request["operation_id"].as_str().ok_or("Missing operation identifier.")?;
  Uuid::parse_str(id).map_err(|_|"Invalid operation identifier.")?;
  let payload=&request["payload"];
  if !payload.is_object(){return Err("Invalid payload.".into());}
  let fingerprint=format!("{:x}",Sha256::digest(json!({"action":action,"payload":payload,"epoch":request["epoch"],"expected_revision":request["expected_revision"]}).to_string().as_bytes()));
  if let Some(receipt)=self.store.online_receipt(id)?{
   if receipt["request"]!=fingerprint {return Err("Operation identifier was already used for another request.".into());}
   return Ok(receipt["response"].clone());
  }
  let state=self.state()?;
  if request["epoch"]!=state["epoch"] {return Err("Database was replaced. Reload before saving.".into());}
  if request["expected_revision"]!=state["revision"]{return Err("Inventory changed on another device. Reload and review before saving.".into());}
  let revision=state["revision"].as_u64().ok_or("Invalid revision.")?.checked_add(1).ok_or("Revision overflow.")?;
  let staging=tempfile::tempdir_in(&self.directory).map_err(|e|e.to_string())?;
  let path=staging.path().join("working.db");self.store.backup(&path)?;
  let mut working=store::Store::open(&path)?;
  let mut result=working.execute(action,payload.clone())?;
  // Temporary staging paths must never appear as actionable client backups.
  if let Some(object)=result.as_object_mut(){object.remove("pre_edit_backup");}
  let response=json!({"result":result,"state":{"revision":revision,"epoch":state["epoch"]}});
  working.online_record(id,&fingerprint,&response,revision)?;
  let backup=self.directory.join("backups");fs::create_dir_all(&backup).map_err(|e|e.to_string())?;
  self.store.backup(&backup.join(format!("before-{revision}-{}.db",Uuid::new_v4())))?;
  self.store.online_publish(&path)?;
  if let Err(error)=self.checkpoint(){let mut response=response;response["result"]["backup_warning"]=json!(format!("Saved online, but the server snapshot failed: {error}"));return Ok(response);}
  Ok(response)
 }
 pub fn checkpoint(&mut self)->Result<(),String>{
  let state=self.state()?;let directory=self.directory.join("backups");fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
  let path=directory.join(format!("snapshot-{}-{}.db",state["epoch"].as_str().ok_or("Invalid epoch.")?,state["revision"]));
  if !path.exists(){self.store.backup(&path)?;}
  let mut files:Vec<_>=fs::read_dir(&directory).map_err(|e|e.to_string())?.filter_map(Result::ok).filter(|e|{let n=e.file_name();let n=n.to_string_lossy();(n.starts_with("before-")||n.starts_with("snapshot-"))&&n.ends_with(".db")}).filter_map(|e|e.metadata().ok().and_then(|m|m.modified().ok()).map(|t|(t,e.path()))).collect();
  files.sort_by_key(|(t,_)|std::cmp::Reverse(*t));let mut days=std::collections::HashSet::new();
  for (index,(modified,file))in files.into_iter().enumerate(){let age=std::time::SystemTime::now().duration_since(modified).unwrap_or_default().as_secs()/86400;let day=chrono::DateTime::<chrono::Utc>::from(modified).format("%Y-%m-%d").to_string();let daily=age<30&&days.insert(day);if index>=64&&!daily&&file!=path&&!file.file_name().unwrap().to_string_lossy().starts_with("before-restore-"){fs::remove_file(file).map_err(|e|e.to_string())?;}}
  Ok(())
 }
 pub fn backup(&mut self)->Result<Vec<u8>,String>{
  let temp=tempfile::tempdir_in(&self.directory).map_err(|e|e.to_string())?;
  let path=temp.path().join("snapshot.db");self.store.backup(&path)?;
  fs::read(path).map_err(|e|e.to_string())
 }
 pub fn restore(&mut self,bytes:&[u8],expected:&Value)->Result<Value,String>{
  let state=self.state()?;
  if expected["epoch"]!=state["epoch"]||expected["revision"]!=state["revision"] {return Err("Inventory changed. Reload before restoring.".into());}
  let temp=tempfile::tempdir_in(&self.directory).map_err(|e|e.to_string())?;
  let source=temp.path().join("incoming.db");fs::write(&source,bytes).map_err(|e|e.to_string())?;
  let path=temp.path().join("validated.db");let mut working=store::Store::open(&path)?;
  working.restore(&source)?;working.online_reset_epoch(state["revision"].as_u64().ok_or("Invalid revision.")?+1)?;
  let backup=self.directory.join("backups");fs::create_dir_all(&backup).map_err(|e|e.to_string())?;
  self.store.backup(&backup.join(format!("before-restore-{}.db",Uuid::new_v4())))?;
  self.store.online_publish(&path)?;
  Ok(json!({"result":{"restored":true},"state":self.state()?}))
 }
}

#[cfg(test)] mod online_tests {
 use super::*;
 fn request(s:&mut Service,action:&str,payload:Value)->Value {let state=s.state().unwrap();json!({"operation_id":Uuid::new_v4().to_string(),"expected_revision":state["revision"],"epoch":state["epoch"],"action":action,"payload":payload})}
 fn entry()->Value{json!({"item_code":"1","name":"شاشة","model_description":"Samsung screen","quantity":1,"unit_cost_baisa":2000})}
 #[test] fn replay_survives_restart_and_does_not_duplicate(){let d=tempfile::tempdir().unwrap();let mut s=Service::open(d.path()).unwrap();let r=request(&mut s,"stock_entry",entry());let first=s.mutate(&r).unwrap();drop(s);let mut s=Service::open(d.path()).unwrap();assert_eq!(s.mutate(&r).unwrap(),first);assert_eq!(s.snapshot(json!({})).unwrap()["result"]["summary"]["total_units"],1);let mut other=r;other["payload"]["quantity"]=json!(2);assert!(s.mutate(&other).is_err());}
 #[test] fn competing_requests_cannot_sell_last_unit_twice(){let d=tempfile::tempdir().unwrap();let mut s=Service::open(d.path()).unwrap();let r=request(&mut s,"stock_entry",entry());s.mutate(&r).unwrap();let a=request(&mut s,"stock_out",json!({"cr_number":"1","lines":[{"item_code":"1","quantity":1,"unit_price_baisa":5000}]}));let mut b=a.clone();b["operation_id"]=json!(Uuid::new_v4().to_string());s.mutate(&a).unwrap();assert!(s.mutate(&b).unwrap_err().contains("changed"));assert_eq!(s.snapshot(json!({})).unwrap()["result"]["summary"]["total_units"],0);}
 #[test] fn restore_changes_epoch_and_blocks_old_requests(){let d=tempfile::tempdir().unwrap();let mut s=Service::open(d.path()).unwrap();let old=request(&mut s,"stock_entry",entry());let bytes=s.backup().unwrap();let state=s.state().unwrap();s.restore(&bytes,&state).unwrap();assert!(s.mutate(&old).unwrap_err().contains("replaced"));let current=s.state().unwrap();assert!(s.restore(b"not a database",&current).is_err());}
 #[test] fn rejected_write_preserves_revision_and_data(){let d=tempfile::tempdir().unwrap();let mut s=Service::open(d.path()).unwrap();let before=s.state().unwrap();let r=request(&mut s,"stock_entry",json!({"quantity":-1}));assert!(s.mutate(&r).is_err());assert_eq!(s.state().unwrap(),before);assert_eq!(s.snapshot(json!({})).unwrap()["result"]["summary"]["total_products"],0);}
}
