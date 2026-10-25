//! Optional cloud copies of completed local backups. Never opens the live SQLite DB.
mod oauth;
mod drive;
use chrono::Utc;
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,path::{Path,PathBuf},sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,Ordering}},time::{Duration,Instant}};
use uuid::Uuid;
#[derive(Clone,Serialize,Deserialize)]
struct Pending {id:String,source:String,created_at:String,sha256:String,bytes:u64,remote_id:Option<String>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(default)]
struct Config {device:String,connected:bool,enabled:bool,pending:Option<Pending>,last_uploaded_source:String,last_success:Option<String>,folder_id:Option<String>,account:Option<String>}
impl Default for Config{fn default()->Self{Self{device:Uuid::new_v4().to_string(),connected:false,enabled:false,pending:None,last_uploaded_source:String::new(),last_success:None,folder_id:None,account:None}}}
struct Inner {config:Config,state:String,message:String,warning:String,signing_in:bool,force:bool,changed:Instant,queued_since:Instant,next_attempt:Instant,attempt:u32}
pub struct CloudBackup {directory:PathBuf,client:oauth::ClientConfig,inner:Mutex<Inner>,generation:AtomicU64,closed:AtomicBool}
#[derive(Debug)]
pub(super) struct Failure{message:String,auth:bool,offline:bool}
impl Failure{fn local(s:&str)->Self{Self{message:s.into(),auth:false,offline:false}}}
fn network(e:reqwest::Error)->Failure{Failure{message:if e.is_connect()||e.is_timeout(){"Waiting for internet. Google Drive backup will retry automatically."}else{"Google Drive connection failed. The backup remains queued."}.into(),auth:false,offline:e.is_connect()||e.is_timeout()}}
fn google_json(r:reqwest::blocking::Response)->Result<Value,Failure>{
 let code=r.status();let v:Value=r.json().unwrap_or(Value::Null);
 if code.is_success(){return if v.is_object(){Ok(v)}else{Err(Failure::local("Google returned an invalid response. The local backup is safe."))};}
 let reason=v["error"]["errors"][0]["reason"].as_str().unwrap_or("");
 let auth=code==reqwest::StatusCode::UNAUTHORIZED||v["error"]=="invalid_grant"||v["error"]=="invalid_client";
 let message=if auth{"Google sign-in expired or was revoked. Reconnect Google Drive; local backups are still active."}else if reason=="storageQuotaExceeded"{"Google Drive storage is full. Free space, then choose Retry backup."}else if code.as_u16()==429||code.is_server_error(){"Google Drive is temporarily busy. Backup will retry automatically."}else if code.as_u16()==403{"Google Drive rejected access. Check the app's Drive permission and reconnect if necessary."}else{"Google Drive backup failed. The local copy is safe; retry or reconnect."};
 Err(Failure{message:message.into(),auth,offline:false})
}

#[cfg(test)]mod tests{
 use super::*;
 fn configured(root:&Path)->CloudBackup{let mut c=CloudBackup::new(root);c.client=oauth::ClientConfig{client_id:"123-test.apps.googleusercontent.com".into(),client_secret:"test-public-client".into()};{let mut s=c.inner.lock().unwrap();s.config.connected=true;s.config.enabled=true;}c}
 fn snapshot(dir:&Path,name:&str)->PathBuf{let mut store=crate::store::Store::open(&dir.join("live.db")).unwrap();if store.execute("snapshot",json!({})).unwrap()["products"].as_array().unwrap().is_empty(){store.execute("stock_entry",json!({"item_code":"1","name":"Test screen","model_description":"Test model","quantity":7,"unit_cost_baisa":250})).unwrap();}let path=dir.join(format!("inventory-auto-{name}.db"));store.backup(&path).unwrap();path}
 #[test]fn snapshots_are_valid_sqlite_durable_and_coalesced_without_changing_live_data(){
  let dir=tempfile::tempdir().unwrap();let c=configured(dir.path());let first=snapshot(dir.path(),"first");let before=hash_file(&dir.path().join("live.db")).unwrap();c.queue(&first).unwrap();let p=c.inner.lock().unwrap().config.pending.clone().unwrap();let old=c.pending_path(&p);assert_eq!(hash_file(&old).unwrap(),(p.sha256.clone(),p.bytes));
  let read=rusqlite::Connection::open_with_flags(&old,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();assert_eq!(read.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");assert_eq!(read.query_row("SELECT SUM(quantity) FROM stock_entries",[],|r|r.get::<_,i64>(0)).unwrap(),7);drop(read);
  let second=snapshot(dir.path(),"second");c.queue(&second).unwrap();assert!(!old.exists());assert_eq!(hash_file(&dir.path().join("live.db")).unwrap(),before);let restarted=configured(dir.path());let restored=restarted.inner.lock().unwrap().config.pending.clone().unwrap();assert_eq!(restored.source,"inventory-auto-second.db");assert!(restarted.pending_path(&restored).is_file());assert_eq!(c.inner.lock().unwrap().config.device,restarted.inner.lock().unwrap().config.device);
 }
 #[test]fn recovery_pause_and_local_backup_failures_never_upload_or_drop_pending(){
  let dir=tempfile::tempdir().unwrap();let c=configured(dir.path());let source=snapshot(dir.path(),"safe");c.queue(&source).unwrap();let p=c.inner.lock().unwrap().config.pending.clone().unwrap();c.inner.lock().unwrap().force=true;
  c.tick(Some(source.clone()),true,true);assert_eq!(c.status()["state"],"recovery_required");assert!(c.pending_path(&p).exists());
  c.tick(Some(source.clone()),false,false);assert_eq!(c.status()["state"],"waiting_for_local_backup");c.set_enabled(false).unwrap();c.tick(Some(source),false,true);assert_eq!(c.status()["state"],"paused");assert!(c.pending_path(&p).exists());let config=fs::read_to_string(c.directory.join("settings.json")).unwrap();assert!(!config.contains("access_token"));assert!(!config.contains("refresh_token"));c.close();assert!(!c.valid(c.generation.load(Ordering::Acquire)));
 }
 #[test]fn source_queue_survives_offline_and_auth_failure_keeps_backup_for_reconnect(){
  let dir=tempfile::tempdir().unwrap();let c=configured(dir.path());let source=snapshot(dir.path(),"safe");c.queue(&source).unwrap();c.tick(Some(source),false,true);assert_eq!(c.status()["state"],"queued");c.failure(Failure{message:"Waiting for internet".into(),auth:false,offline:true});assert_eq!(c.status()["state"],"waiting_for_internet");assert!(c.status()["pending"].as_bool().unwrap());c.failure(Failure{message:"Reconnect Google Drive".into(),auth:true,offline:false});assert_eq!(c.status()["state"],"reconnect_required");assert_eq!(c.status()["enabled"],false);assert!(c.status()["pending"].as_bool().unwrap());assert!(c.retry().is_err());
 }
 #[test]fn corrupt_settings_fail_closed_and_missing_client_never_starts_sign_in(){
  let dir=tempfile::tempdir().unwrap();fs::create_dir_all(dir.path().join("google-drive-backup")).unwrap();fs::write(dir.path().join("google-drive-backup/settings.json"),b"not json").unwrap();let mut c=CloudBackup::new(dir.path());c.client=oauth::ClientConfig::default();let c=Arc::new(c);assert!(!c.status()["connected"].as_bool().unwrap());assert!(!c.status()["enabled"].as_bool().unwrap());assert_eq!(c.status()["state"],"not_configured");assert!(c.connect().is_err());assert!(!c.status().to_string().contains("client_secret"));
 }
 #[test]fn continuous_entry_cannot_delay_a_backup_indefinitely(){let dir=tempfile::tempdir().unwrap();let c=configured(dir.path());let source=snapshot(dir.path(),"continuous");c.queue(&source).unwrap();{let mut s=c.inner.lock().unwrap();s.queued_since=Instant::now()-Duration::from_secs(601);s.changed=Instant::now();}c.tick(Some(source),false,true);assert_eq!(c.status()["state"],"reconnect_required");assert!(c.status()["pending"].as_bool().unwrap());}
 #[test]fn secure_store_round_trip_uses_unique_test_identity(){let device=Uuid::new_v4().to_string();let token=oauth::Tokens{refresh_token:"fixture-not-a-google-token".into()};oauth::vault_set(&device,&token).unwrap();assert_eq!(oauth::vault_get(&device).unwrap().refresh_token,token.refresh_token);oauth::vault_delete(&device).unwrap();assert!(oauth::vault_get(&device).is_err());}
}
fn device_label()->&'static str{if cfg!(windows){"Windows"}else{"Mac"}}
fn hash_file(path:&Path)->Result<(String,u64),String>{let mut file=fs::File::open(path).map_err(|_|"Cannot read the completed local backup.")?;let mut hash=Sha256::new();let bytes=std::io::copy(&mut file,&mut drive::HashWriter(&mut hash)).map_err(|_|"Cannot verify the local backup.")?;Ok((format!("{:x}",hash.finalize()),bytes))}
impl CloudBackup {
 pub fn new(data:&Path)->Self{
  let directory=data.join("google-drive-backup");let raw=fs::read(directory.join("settings.json"));let mut warning=String::new();
  let mut config=match raw{Ok(b)=>serde_json::from_slice::<Config>(&b).unwrap_or_else(|_|{warning="Google Drive settings could not be read. Reconnect Drive. Local backups are unchanged.".into();Config::default()}),Err(_)=>Config::default()};
  if Uuid::parse_str(&config.device).is_err()||config.pending.as_ref().is_some_and(|p|Uuid::parse_str(&p.id).is_err()||p.sha256.len()!=64||!p.sha256.bytes().all(|b|b.is_ascii_hexdigit())||p.remote_id.as_deref().is_some_and(|s|!drive::valid_id(s))){config=Config::default();warning="Invalid Google Drive settings were ignored. Reconnect Drive; local backups are unchanged.".into();}
  Self{directory,client:oauth::ClientConfig::bundled(),inner:Mutex::new(Inner{config,state:"idle".into(),message:String::new(),warning,signing_in:false,force:false,changed:Instant::now(),queued_since:Instant::now(),next_attempt:Instant::now(),attempt:0}),generation:AtomicU64::new(0),closed:AtomicBool::new(false)}
 }
 fn save(&self,c:&Config)->Result<(),String>{
  fs::create_dir_all(&self.directory).map_err(|_|"Cannot create the local Drive backup queue.")?;
  #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;fs::set_permissions(&self.directory,fs::Permissions::from_mode(0o700)).map_err(|_|"Cannot protect the Drive backup queue.")?;}
  let temp=self.directory.join("settings.json.tmp");let bytes=serde_json::to_vec_pretty(c).map_err(|_|"Cannot save Drive backup settings.")?;fs::write(&temp,bytes).map_err(|_|"Cannot save Drive backup settings.")?;fs::OpenOptions::new().write(true).open(&temp).and_then(|f|f.sync_all()).map_err(|_|"Cannot save Drive backup settings.")?;fs::rename(temp,self.directory.join("settings.json")).map_err(|_|"Cannot save Drive backup settings.".into())
 }
 pub fn status(&self)->Value{
  let s=self.inner.lock().unwrap_or_else(|e|e.into_inner());let c=&s.config;
  let state=if !self.client.configured(){"not_configured"}else if s.signing_in{"signing_in"}else if s.state=="reconnect_required"{"reconnect_required"}else if s.state=="backup_failed"&&!s.message.is_empty(){"backup_failed"}else if !c.connected{"disconnected"}else if !c.enabled{"paused"}else{s.state.as_str()};
  let message=match state{"not_configured"=>"Google Drive is not configured in this build. Local automatic backups remain active.","disconnected"=>"Not connected. Your inventory and local backups continue to work offline.","paused"=>"Google Drive backups are paused. Local backups remain active.","signing_in"=>"Complete Google sign-in in your browser, then return here.",_=>s.message.as_str()};
  json!({"configured":self.client.configured(),"connected":c.connected,"enabled":c.enabled,"state":state,"message":message,"warning":s.warning,"last_success":c.last_success,"pending":c.pending.is_some(),"device":format!("{} - {}",device_label(),&c.device[..8]),"account":c.account,"folder_available":c.folder_id.as_deref().is_some_and(drive::valid_id)})
 }
 pub fn connect(self:&Arc<Self>)->Result<Value,String>{
  if !self.client.configured(){return Err("The developer must configure this build's Google Desktop OAuth client first.".into());}
  let generation=self.generation.fetch_add(1,Ordering::AcqRel)+1;
  {let mut s=self.inner.lock().map_err(|_|"Drive settings are unavailable.")?;s.config.enabled=false;self.save(&s.config)?;s.signing_in=true;s.message.clear();s.state="signing_in".into();}
  let this=Arc::clone(self);std::thread::spawn(move||{
   let result=oauth::sign_in(&this.client,||this.valid(generation));
   if !this.valid(generation){return;}
   let mut s=this.inner.lock().unwrap_or_else(|e|e.into_inner());
   if !this.valid(generation){return;}s.signing_in=false;
   let result=result.and_then(|tokens|{oauth::vault_set(&s.config.device,&tokens)?;let mut next=s.config.clone();next.connected=true;next.enabled=true;next.folder_id=None;next.account=None;next.last_success=None;if let Some(p)=next.pending.as_mut(){p.remote_id=None;}next.last_uploaded_source.clear();this.save(&next)?;s.config=next;Ok(())});
   match result{Ok(())=>{s.state="queued".into();s.message="Connected. A backup will upload shortly.".into();s.force=true;s.next_attempt=Instant::now();s.attempt=0;},Err(e)=>{s.state="backup_failed".into();s.message=e;}}
  });Ok(self.status())
 }
 pub fn set_enabled(&self,enabled:bool)->Result<Value,String>{
  {let mut s=self.inner.lock().map_err(|_|"Drive settings are unavailable.")?;if enabled&&(!s.config.connected||s.state=="reconnect_required"){return Err("Connect Google Drive first.".into());}let mut next=s.config.clone();next.enabled=enabled;self.save(&next)?;s.config=next;s.force=enabled;s.next_attempt=Instant::now();s.attempt=0;s.state=if enabled{"queued"}else{"paused"}.into();s.message=if enabled{"Backup queued."}else{"Google Drive backups are paused."}.into();if !enabled{self.generation.fetch_add(1,Ordering::AcqRel);s.signing_in=false;}}
  Ok(self.status())
 }
 pub fn disconnect(&self)->Result<Value,String>{
  self.generation.fetch_add(1,Ordering::AcqRel);
  {let mut s=self.inner.lock().map_err(|_|"Drive settings are unavailable.")?;s.signing_in=false;s.config.enabled=false;self.save(&s.config)?;oauth::vault_delete(&s.config.device)?;s.config.connected=false;s.config.folder_id=None;s.state="disconnected".into();s.message.clear();self.save(&s.config)?;}
  Ok(self.status())
 }
 pub fn retry(&self)->Result<Value,String>{
  {let mut s=self.inner.lock().map_err(|_|"Drive settings are unavailable.")?;if !s.config.connected||!s.config.enabled||s.state=="reconnect_required"{return Err("Connect and enable Google Drive backups first.".into());}s.force=true;s.next_attempt=Instant::now();s.attempt=0;s.state="queued".into();s.message="Backup queued for upload.".into();}
  Ok(self.status())
 }
 pub fn open_folder(&self)->Result<Value,String>{let url={let s=self.inner.lock().map_err(|_|"Drive settings are unavailable.")?;if !s.config.connected{return Err("Connect Google Drive first.".into());}match s.config.folder_id.as_deref().filter(|s|drive::valid_id(s)){Some(id)=>format!("https://drive.google.com/drive/folders/{id}"),None=>"https://drive.google.com/drive/my-drive".into()}};oauth::browser(&url)?;Ok(json!({"opened":true}))}
 fn valid(&self,generation:u64)->bool{!self.closed.load(Ordering::Acquire)&&self.generation.load(Ordering::Acquire)==generation}
 pub fn close(&self){self.closed.store(true,Ordering::Release);self.generation.fetch_add(1,Ordering::AcqRel);}
 fn pending_path(&self,p:&Pending)->PathBuf{self.directory.join(format!("{}.db",p.id))}
 fn queue(&self,source:&Path)->Result<(),String>{
  let name=source.file_name().and_then(|s|s.to_str()).filter(|s|s.starts_with("inventory-auto-")&&s.ends_with(".db")).ok_or("No completed automatic backup is available.")?;
  {let s=self.inner.lock().map_err(|_|"Drive settings unavailable.")?;if s.config.pending.as_ref().is_some_and(|p|p.source==name&&self.pending_path(p).is_file())||s.config.last_uploaded_source==name{return Ok(());}}
  fs::create_dir_all(&self.directory).map_err(|_|"Cannot create the Drive backup queue.")?;
  #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;fs::set_permissions(&self.directory,fs::Permissions::from_mode(0o700)).map_err(|_|"Cannot protect the local backup queue.")?;}
  let id=Uuid::new_v4().to_string();let path=self.directory.join(format!("{id}.db"));fs::copy(source,&path).map_err(|_|"Cannot queue the completed local backup. It will be retried.")?;
  fs::OpenOptions::new().write(true).open(&path).and_then(|f|f.sync_all()).map_err(|_|"Cannot save the queued backup.")?;
  let (hash,bytes)=hash_file(&path)?;let pending=Pending{id,source:name.into(),created_at:Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),sha256:hash,bytes,remote_id:None};
  let mut s=self.inner.lock().map_err(|_|"Drive settings unavailable.")?;
  let old=s.config.pending.clone();if old.is_none(){s.queued_since=Instant::now();}let mut next=s.config.clone();next.pending=Some(pending);self.save(&next)?;s.config=next;s.changed=Instant::now();
  if let Some(old)=old{let _=fs::remove_file(self.pending_path(&old));}Ok(())
 }
 // Called by one background worker, never by an inventory write or UI thread.
 pub fn tick(&self,source:Option<PathBuf>,recovery_pending:bool,local_ready:bool){
  {let s=self.inner.lock().unwrap_or_else(|e|e.into_inner());if !self.client.configured()||!s.config.connected||!s.config.enabled||s.signing_in{return;}}
  if recovery_pending{let mut s=self.inner.lock().unwrap_or_else(|e|e.into_inner());s.state="recovery_required".into();s.message="Drive backup is paused until you restore the missing local database. Existing cloud backups are preserved.".into();return;}
  if !local_ready{let mut s=self.inner.lock().unwrap_or_else(|e|e.into_inner());s.state="waiting_for_local_backup".into();s.message="No current local backup is confirmed. Check Automatic backups or choose Back up to Drive now. Inventory remains available.".into();return;}
  if let Some(source)=source{if let Err(e)=self.queue(&source){self.failure(Failure::local(&e));return;}}
  let (config,pending,generation)={
   let mut s=self.inner.lock().unwrap_or_else(|e|e.into_inner());if !s.config.enabled||!s.config.connected||s.signing_in||self.closed.load(Ordering::Acquire){return;}let Some(p)=s.config.pending.clone()else{if s.state!="reconnect_required"{s.state="up_to_date".into();s.message="Google Drive backup is up to date.".into();}return;};
   if Instant::now()<s.next_attempt||!s.force&&s.changed.elapsed()<Duration::from_secs(60)&&s.queued_since.elapsed()<Duration::from_secs(600){if s.state!="waiting_for_internet"&&s.state!="backup_failed"{s.state="queued".into();s.message="Backup queued. Upload starts after one quiet minute, or within ten minutes during continuous entry.".into();}return;}
   s.force=false;s.state="uploading".into();s.message="Uploading a verified database backup to your Google Drive…".into();(s.config.clone(),p,self.generation.load(Ordering::Acquire))
  };
  let result=self.upload(&config,pending.clone(),generation);
  if !self.valid(generation){return;}
  match result{
   Ok((folder,warning))=>{let mut s=self.inner.lock().unwrap_or_else(|e|e.into_inner());let mut next=s.config.clone();next.last_success=Some(Utc::now().to_rfc3339());next.last_uploaded_source=pending.source.clone();next.folder_id=Some(folder);if next.pending.as_ref().is_some_and(|p|p.id==pending.id){next.pending=None;}
    match self.save(&next){Ok(())=>{s.config=next;s.state="up_to_date".into();s.message="Backup uploaded and verified successfully.".into();s.warning=warning;s.attempt=0;let _=fs::remove_file(self.pending_path(&pending));},Err(e)=>{s.state="backup_failed".into();s.message=e;s.next_attempt=Instant::now()+Duration::from_secs(60);}}
   },Err(e)=>self.failure(e)
  }
 }
 fn upload(&self,config:&Config,mut pending:Pending,generation:u64)->Result<(String,String),Failure>{
  let tokens=oauth::vault_get(&config.device).map_err(|e|Failure{message:e,auth:true,offline:false})?;let c=oauth::client()?;let access=oauth::refresh(&c,&self.client,&tokens)?;
  let d=drive::Drive::new(&c,&access,Box::new(||self.valid(generation)));let account=d.account()?;{let mut s=self.inner.lock().map_err(|_|Failure::local("Drive settings unavailable."))?;if !self.valid(generation){return Err(Failure::local("Backup cancelled."));}s.config.account=account;self.save(&s.config).map_err(|e|Failure::local(&e))?;}let folder=d.folder(&config.device)?;
  if pending.remote_id.is_none(){let id=d.generate_id()?;let mut s=self.inner.lock().map_err(|_|Failure::local("Drive settings unavailable."))?;if !self.valid(generation){return Err(Failure::local("Backup cancelled."));}pending.remote_id=Some(id);let mut next=s.config.clone();if next.pending.as_ref().is_some_and(|p|p.id==pending.id){next.pending=Some(pending.clone());}self.save(&next).map_err(|e|Failure::local(&e))?;s.config=next;}
  let path=self.pending_path(&pending);let (hash,bytes)=hash_file(&path).map_err(|e|Failure::local(&e))?;
  if hash!=pending.sha256||bytes!=pending.bytes{return Err(Failure::local("The queued backup changed unexpectedly. Local backups are preserved."));}
  d.upload(&config.device,&folder,&pending,&path)?;
  let warning=d.retain_history(&config.device,&folder,pending.remote_id.as_deref().unwrap()).err().map(|_|"Backup succeeded, but older Drive history could not be pruned. Existing versions have been kept.".into()).unwrap_or_default();Ok((folder,warning))
 }
 fn failure(&self,e:Failure){let mut s=self.inner.lock().unwrap_or_else(|e|e.into_inner());s.state=if e.auth{"reconnect_required"}else if e.offline{"waiting_for_internet"}else{"backup_failed"}.into();s.message=e.message;s.attempt=s.attempt.saturating_add(1);s.next_attempt=Instant::now()+Duration::from_secs((30u64.saturating_mul(1u64<<s.attempt.min(5))).min(900));if e.auth{s.config.enabled=false;let _=self.save(&s.config);}}
}
