//! Online business connection. No local writable inventory is opened.
use reqwest::{blocking::Client,redirect::Policy,Url};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,io::Write,path::{Path,PathBuf},time::Duration};
use uuid::Uuid;
const WRITES:&[&str]=&["save_product","stock_entry","stock_out","edit_record","delete_record","damaged_return","warranty"];
#[derive(Clone,Serialize,Deserialize)]
struct Configuration{url:String,credential_id:String}
pub struct Gateway{client:Client,config:Option<Configuration>,state:Option<Value>,directory:PathBuf,backups:PathBuf,backup_state:Option<Value>,warning:String,cached_snapshot:Option<(Value,Value,Value)>,last_verified:Option<std::time::Instant>,read_source:String,last_commit:Option<std::time::Instant>,backup_attempt:Option<std::time::Instant>}
impl Gateway{
 pub fn open(directory:&Path,backups:&Path)->Result<Self,String>{
  let path=directory.join("online-connection.json");
  let config=if path.exists(){Some(serde_json::from_slice(&fs::read(&path).map_err(|e|e.to_string())?).map_err(|_|"Online connection settings are damaged; existing data has not been changed.")?)}else{None};
  Ok(Self{client:Client::builder().timeout(Duration::from_secs(20)).connect_timeout(Duration::from_secs(5)).redirect(Policy::none()).build().map_err(|e|e.to_string())?,config,state:None,directory:directory.into(),backups:backups.into(),backup_state:None,warning:String::new(),cached_snapshot:None,last_verified:None,read_source:String::new(),last_commit:None,backup_attempt:None})
 }
 fn turso(&self)->Result<Option<crate::turso::Turso>,String>{match self.config.as_ref(){Some(c)if crate::turso::accepts(&c.url)=>Ok(Some(crate::turso::Turso::with_client(&c.url,Self::credential(c)?,&self.directory,self.client.clone())?.recovery_directory(&self.backups))),_=>Ok(None)}}
 fn credential(c:&Configuration)->Result<String,String>{keyring::Entry::new("com.repaircenter.inventorymanager.online",&c.credential_id).map_err(|_|"Secure credential storage unavailable.")?.get_password().map_err(|_|"Online connection credential unavailable. Configure this installation again.".into())}
 fn request(&self,method:reqwest::Method,path:&str)->Result<reqwest::blocking::RequestBuilder,String>{let c=self.config.as_ref().ok_or("Online database is not configured. Open Online connection; no local database will be used.")?;Ok(self.client.request(method,format!("{}{path}",c.url)).bearer_auth(Self::credential(c)?))}
 fn decode(response:reqwest::blocking::Response)->Result<Value,String>{let code=response.status();let value:Value=response.json().map_err(|_|"The database service returned an invalid response.")?;if !code.is_success(){return Err(value["error"].as_str().unwrap_or("The database service rejected this request.").into());}Ok(value)}
 pub fn configure(&mut self,url:&str,token:&str)->Result<Value,String>{
  if self.directory.join("pending-online-operation.json").exists(){return Err("An earlier save is awaiting confirmation. Reconnect to the same database before changing the connection.".into());}
  if token.len()<32||token.len()>4096{return Err("Enter the private installation key supplied with this deployment.".into());}
  let (url,value)=if crate::turso::accepts(url.trim()){
   let direct=crate::turso::Turso::with_client(url.trim(),token.to_string(),&self.directory,self.client.clone())?;let state=direct.initialize()?;(url.trim().to_string(),json!({"state":state}))
  }else{
  let parsed=Url::parse(url.trim()).map_err(|_|"Enter a valid HTTPS service address.")?;
  if !(parsed.scheme()=="https" || cfg!(debug_assertions)&&parsed.scheme()=="http"&&matches!(parsed.host_str(),Some("127.0.0.1"|"localhost")))||!parsed.username().is_empty()||parsed.password().is_some()||parsed.query().is_some()||parsed.fragment().is_some()||parsed.path()!="/"{return Err("Use an HTTPS server origin without a path, password or query.".into());}
  if token.len()<32||token.len()>4096{return Err("Enter the private installation key supplied with this deployment.".into());}
  let url=parsed.as_str().trim_end_matches('/').to_string();
  let value=Self::decode(self.client.get(format!("{url}/v1/state")).bearer_auth(token).send().map_err(|_|"Cannot connect to the online database. No settings were changed.")?)?;
   (url,value)
  };
  let config=Configuration{url,credential_id:Uuid::new_v4().to_string()};
  let entry=keyring::Entry::new("com.repaircenter.inventorymanager.online",&config.credential_id).map_err(|_|"Secure credential storage unavailable.")?;entry.set_password(token).map_err(|_|"Could not save the connection key securely.")?;
  if let Err(e)=atomic_write(&self.directory.join("online-connection.json"),&serde_json::to_vec(&config).map_err(|e|e.to_string())?){let _=entry.delete_credential();return Err(e);}
  self.config=Some(config);self.state=Some(value["state"].clone());self.backup_state=None;self.cached_snapshot=None;self.last_verified=Some(std::time::Instant::now());Ok(self.status())
 }
 pub fn status(&self)->Value{json!({"configured":self.config.is_some(),"connected":self.last_verified.is_some_and(|t|t.elapsed()<Duration::from_secs(10)),"url":self.config.as_ref().map(|c|&c.url),"state":self.state,"backup_state":self.backup_state,"read_source":self.read_source,"backup_directory":self.backups,"warning":self.warning})}
 pub fn execute(&mut self,action:&str,mut payload:Value)->Result<Value,String>{
  if action=="status"{return Ok(self.status());}
  if action=="snapshot"{
   let prefer=payload.as_object_mut().and_then(|p|p.remove("prefer_confirmed_commit"))==Some(Value::Bool(true));
   if prefer&&self.last_commit.is_some_and(|t|t.elapsed()<Duration::from_secs(2))&&!self.directory.join("pending-online-operation.json").exists(){if let Some((month,cached,_))=&self.cached_snapshot{if *month==payload{return Ok(cached.clone());}}}
   self.recover_pending()?;
   let direct=self.turso()?;let using_turso=direct.is_some();
   let value=if let Some(t)=direct {
    let (remote,time)=t.state_and_clock().map_err(|e|{self.last_verified=None;e.message})?;self.last_verified=Some(std::time::Instant::now());if payload["month"].is_null(){payload["month"]=json!(&time[..7]);}
    if let Some((month,cached,state))=&self.cached_snapshot{if *month==payload&&*state==remote{self.read_source="memory".into();let result=cached.clone();if self.backup_state.as_ref()!=Some(&remote)&&self.backup_attempt.is_none_or(|t|t.elapsed()>Duration::from_secs(60)){self.capture_if_changed();}return Ok(result);}}
    if let Some(snapshot)=self.cached_backup_snapshot(&payload,&remote){self.read_source="verified_backup".into();self.backup_state=Some(remote.clone());json!({"result":snapshot,"state":remote})}else{self.read_source="cloud".into();let capture=payload.clone();t.snapshot_observe(payload.clone(),|store,_|self.cache_from_store(store,&capture,false)).map_err(|e|e.message)?}
   }else{Self::decode(self.request(reqwest::Method::POST,"/v1/action")?.json(&json!({"action":"snapshot","payload":payload})).send().map_err(|_|"Online database disconnected. Changes are unavailable until reconnected.")?)?};
   self.last_verified=Some(std::time::Instant::now());self.state=Some(value["state"].clone());let mut result=value["result"].clone();result["online_state"]=value["state"].clone();
   for field in ["products","entries","sale_lines","low_stock"]{if let Some(rows)=result[field].as_array_mut(){for row in rows{row["_online_state"]=value["state"].clone();}}}
   self.cached_snapshot=Some((payload,result.clone(),value["state"].clone()));if !using_turso{self.capture_if_changed();}return Ok(result);
  }
  if !WRITES.contains(&action){return Err("Unknown online operation.".into());}
  let expected=payload.as_object_mut().and_then(|p|p.remove("_online_state")).filter(|v|v.is_object()).or_else(||self.state.clone()).ok_or("Load current inventory before saving.")?;
  let pending_path=self.directory.join("pending-online-operation.json");
  if pending_path.exists(){let old:Value=serde_json::from_slice(&fs::read(&pending_path).map_err(|e|e.to_string())?).map_err(|_|"Pending save record is damaged. Keep this file for recovery.")?;let result=self.send_mutation(&old,None)?;if old["action"]==action&&old["payload"]==payload{return Ok(result);}return Err("The earlier save was confirmed. Reload and review before making another change.".into());}
  let mut seed=None;
  let current=if let Some(t)=self.turso()?{let (state,timestamp)=t.state_and_clock().map_err(|_|"Online database disconnected. This change was not queued or saved.")?;if let Some(path)=self.verified_backup_path(&state){seed=Some(crate::turso::VerifiedSeed{path,state:state.clone(),timestamp});}json!({"state":state})}else{Self::decode(self.request(reqwest::Method::GET,"/v1/state")?.send().map_err(|_|"Online database disconnected. This change was not queued or saved.")?)?};
  if current["state"]!=expected{return Err("Inventory changed on another device. Refresh and review before saving.".into());}
  let request=json!({"action":action,"payload":payload,"operation_id":Uuid::new_v4().to_string(),"expected_revision":expected["revision"],"epoch":expected["epoch"]});
  atomic_write(&pending_path,&serde_json::to_vec(&request).map_err(|e|e.to_string())?)?;
  self.send_mutation(&request,seed)
 }
 fn send_mutation(&mut self,request:&Value,seed:Option<crate::turso::VerifiedSeed>)->Result<Value,String>{
  if let Some(t)=self.turso()?{
   self.last_commit=None;
   let result=t.mutate_observe(request,seed,|store,time|self.cache_from_store(store,&json!({"month":&time[..7]}),true));
   if result.is_ok()||result.as_ref().is_err_and(|e|e.definitive){fs::remove_file(self.directory.join("pending-online-operation.json")).map_err(|_|"Could not clear the pending confirmation. Refresh before continuing.")?;}
   let value=result.map_err(|e|e.message)?;if self.last_commit.is_none(){self.state=Some(value["state"].clone());self.cached_snapshot=None;}let mut result=value["result"].clone();if !self.warning.is_empty(){result["backup_warning"]=json!(self.warning);}return Ok(result);
  }

  let response=self.request(reqwest::Method::POST,"/v1/action")?.json(request).send().map_err(|_|"Connection interrupted: save confirmation is pending. Reconnect and refresh before trying a different operation.")?;
  let rejected=matches!(response.status().as_u16(),400|409|413);let decoded=Self::decode(response);
  if rejected||decoded.is_ok(){fs::remove_file(self.directory.join("pending-online-operation.json")).map_err(|_|"Save response received, but pending confirmation could not be cleared. Refresh before continuing.")?;}
  let value=decoded?;self.state=Some(value["state"].clone());self.capture_if_changed();Ok(value["result"].clone())
 }
 fn recover_pending(&mut self)->Result<(),String>{let path=self.directory.join("pending-online-operation.json");if path.exists(){let request=serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?).map_err(|_|"Pending save record is damaged. Keep it for recovery.")?;self.send_mutation(&request,None)?;}Ok(())}
 fn capture_if_changed(&mut self){if self.state==self.backup_state{return;}self.backup_attempt=Some(std::time::Instant::now());let destination=self.backups.join(format!("inventory-online-auto-{}.db",Uuid::new_v4()));match self.backup(&destination){Ok(_)=>{self.warning=prune_automatic_backups(&self.backups).err().unwrap_or_default();},Err(e)=>self.warning=format!("Online data is available, but the local backup failed: {e}")}}
 pub fn backup(&mut self,destination:&Path)->Result<Value,String>{
  if let Some(t)=self.turso()?{let parent=destination.parent().ok_or("Invalid backup destination.")?;fs::create_dir_all(parent).map_err(|e|e.to_string())?;let value=t.backup(destination).map_err(|e|e.message)?;self.backup_state=Some(value["state"].clone());self.save_backup_index(destination,&value["state"])?;return Ok(value);}

  let response=self.request(reqwest::Method::GET,"/v1/backup")?.send().map_err(|_|"Online backup download failed.")?;
  if !response.status().is_success(){return Err("Online backup is unavailable.".into());}
  let expected=response.headers().get("X-Content-SHA256").and_then(|v|v.to_str().ok()).ok_or("Backup checksum missing.")?.to_string();
  let state=response.headers().get("X-Inventory-State").and_then(|v|v.to_str().ok()).and_then(|v|serde_json::from_str::<Value>(v).ok()).ok_or("Backup revision missing.")?;
  if response.content_length().is_some_and(|n|n>32*1024*1024){return Err("Backup exceeds the current download limit.".into());}
  use std::io::Read;let mut bytes=Vec::new();response.take(32*1024*1024+1).read_to_end(&mut bytes).map_err(|_|"Backup download interrupted.")?;
  if bytes.len()>32*1024*1024||format!("{:x}",Sha256::digest(&bytes))!=expected{return Err("Backup checksum failed; previous backups were preserved.".into());}
  let parent=destination.parent().ok_or("Backup destination has no directory.")?;fs::create_dir_all(parent).map_err(|e|e.to_string())?;
  let temp=parent.join(format!(".inventory-download-{}.db",Uuid::new_v4()));atomic_write(&temp,&bytes)?;
  let validation=(||{crate::store::Store::validate_backup(&temp)?;fs::rename(&temp,destination).map_err(|e|e.to_string())})();if validation.is_err(){let _=fs::remove_file(temp);}validation?;
  self.backup_state=Some(state);Ok(json!({"path":destination,"verified":true}))
 }
 fn save_backup_index(&self,path:&Path,state:&Value)->Result<(),String>{
  if path.parent()!=Some(self.backups.as_path()){return Ok(());}
  let file=path.file_name().and_then(|s|s.to_str()).ok_or("Invalid backup filename.")?;let hash=format!("{:x}",Sha256::digest(fs::read(path).map_err(|e|e.to_string())?));
  let index=json!({"url":self.config.as_ref().map(|c|&c.url),"file":file,"sha256":hash,"state":state});atomic_write(&self.directory.join("online-backup-index.json"),&serde_json::to_vec(&index).map_err(|e|e.to_string())?)
 }
 fn verified_backup_path(&self,remote:&Value)->Option<PathBuf>{
  let index:Value=serde_json::from_slice(&fs::read(self.directory.join("online-backup-index.json")).ok()?).ok()?;
  if index["state"]!=*remote||index["url"].as_str()!=self.config.as_ref().map(|c|c.url.as_str()){return None;}
  let file=index["file"].as_str()?;if file.contains(['/', '\\', ':'])||!file.ends_with(".db")||Path::new(file).components().count()!=1{return None;}
  let path=self.backups.join(file);if fs::metadata(&path).ok()?.len()>32*1024*1024{return None;}
  let hash=format!("{:x}",Sha256::digest(fs::read(&path).ok()?));if index["sha256"].as_str()!=Some(hash.as_str()){return None;}
  Some(path)
 }
 fn cached_backup_snapshot(&self,payload:&Value,remote:&Value)->Option<Value>{let path=self.verified_backup_path(remote)?;crate::store::Store::readonly_online_snapshot(&path,payload,remote).ok()}
 fn cache_from_store(&mut self,store:&crate::store::Store,payload:&Value,committed:bool){
  let capture=(||->Result<(),String>{let state=store.online_metadata()?;let mut result=store.snapshot(payload)?;result["online_state"]=state.clone();for field in ["products","entries","sale_lines","low_stock"]{if let Some(rows)=result[field].as_array_mut(){for row in rows{row["_online_state"]=state.clone();}}}
   self.state=Some(state.clone());self.cached_snapshot=Some((payload.clone(),result,state.clone()));self.last_verified=Some(std::time::Instant::now());if committed{self.last_commit=Some(std::time::Instant::now());}
   if self.backup_state!=Some(state.clone()){self.backup_attempt=Some(std::time::Instant::now());fs::create_dir_all(&self.backups).map_err(|e|e.to_string())?;let path=self.backups.join(format!("inventory-online-auto-{}.db",Uuid::new_v4()));store.backup(&path)?;self.save_backup_index(&path,&state)?;self.backup_state=Some(state);prune_automatic_backups(&self.backups)?;}Ok(())})();
  self.warning=capture.err().map(|e|format!("Online operation confirmed, but local backup/cache needs attention: {e}")).unwrap_or_default();
 }
 pub fn restore(&mut self,source:&Path)->Result<Value,String>{
  if self.directory.join("pending-online-operation.json").exists(){return Err("Confirm the pending save before restoring.".into());}
  let metadata=fs::metadata(source).map_err(|e|e.to_string())?;if metadata.len()>32*1024*1024{return Err("Database exceeds the current upload limit.".into());}
  let state=self.state.clone().ok_or("Load inventory before restoring.")?;
  if let Some(t)=self.turso()?{fs::create_dir_all(&self.backups).map_err(|e|e.to_string())?;let recovery=self.backups.join(format!("before-online-restore-{}.db",Uuid::new_v4()));let value=t.restore(source,&state,&recovery).map_err(|e|e.message)?;self.state=Some(value["state"].clone());self.cached_snapshot=None;self.capture_if_changed();return Ok(value["result"].clone());}
  let bytes=fs::read(source).map_err(|e|e.to_string())?;
  let value=Self::decode(self.request(reqwest::Method::POST,"/v1/restore")?.header("X-Inventory-State",state.to_string()).body(bytes).send().map_err(|_|"Restore confirmation was interrupted. Refresh to inspect the shared database before attempting restore again.")?)?;
  self.state=Some(value["state"].clone());self.capture_if_changed();Ok(value["result"].clone())
 }
}
fn atomic_write(path:&Path,bytes:&[u8])->Result<(),String>{let temporary=path.with_extension(format!("{}.tmp",Uuid::new_v4()));let result=(||{let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|e|e.to_string())?;file.write_all(bytes).map_err(|e|e.to_string())?;file.sync_all().map_err(|e|e.to_string())?;drop(file);fs::rename(&temporary,path).map_err(|e|e.to_string())})();if result.is_err(){let _=fs::remove_file(temporary);}result}

// Rotate only our UUID-named automatic snapshots. Manual files and recovery
// copies are never candidates for deletion.
fn prune_automatic_backups(directory:&Path)->Result<(),String>{
 let mut files=fs::read_dir(directory).map_err(|e|e.to_string())?.filter_map(Result::ok).filter(|e|{let n=e.file_name();let n=n.to_string_lossy();n.strip_prefix("inventory-online-auto-").and_then(|s|s.strip_suffix(".db")).is_some_and(|s|Uuid::parse_str(s).is_ok())}).filter_map(|e|e.metadata().ok().and_then(|m|m.modified().ok()).map(|time|(time,e.path()))).collect::<Vec<_>>();
 files.sort_by_key(|(time,_)|std::cmp::Reverse(*time));let mut days=std::collections::HashSet::new();for (index,(time,path))in files.into_iter().enumerate(){let age=std::time::SystemTime::now().duration_since(time).unwrap_or_default().as_secs()/86400;let day=chrono::DateTime::<chrono::Utc>::from(time).format("%Y-%m-%d").to_string();let daily=age<30&&days.insert(day);if index>=64&&!daily{fs::remove_file(path).map_err(|_|"Local snapshot saved, but old automatic backups could not be rotated.".to_string())?;}}Ok(())
}
#[cfg(test)] mod backup_retention_tests {
 use super::*;
 #[test] fn retention_keeps_manual_and_recovery_files(){let d=tempfile::tempdir().unwrap();let manual=d.path().join("inventory-online-manual.db");let recovery=d.path().join("before-online-restore.db");fs::write(&manual,b"manual").unwrap();fs::write(&recovery,b"recovery").unwrap();for _ in 0..70{fs::write(d.path().join(format!("inventory-online-auto-{}.db",Uuid::new_v4())),b"auto").unwrap();}prune_automatic_backups(d.path()).unwrap();assert!(manual.exists()&&recovery.exists());assert_eq!(fs::read_dir(d.path()).unwrap().count(),66);}
}

#[cfg(all(test,target_os="macos"))]
mod mac_credential_tests {
 #[test] #[ignore="Native Mac packaging explicitly verifies its temporary keychain"]
 fn mac_keychain_roundtrip(){let id=uuid::Uuid::new_v4().to_string();let entry=keyring::Entry::new("com.repaircenter.inventorymanager.test",&id).unwrap();entry.set_password("non-secret-local-keychain-probe").unwrap();assert_eq!(entry.get_password().unwrap(),"non-secret-local-keychain-probe");entry.delete_credential().unwrap();}
}
