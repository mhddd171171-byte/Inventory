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
pub struct Gateway{client:Client,config:Option<Configuration>,state:Option<Value>,directory:PathBuf,backups:PathBuf,backup_state:Option<Value>,warning:String}
impl Gateway{
 pub fn open(directory:&Path,backups:&Path)->Result<Self,String>{
  let path=directory.join("online-connection.json");
  let config=if path.exists(){Some(serde_json::from_slice(&fs::read(&path).map_err(|e|e.to_string())?).map_err(|_|"Online connection settings are damaged; existing data has not been changed.")?)}else{None};
  Ok(Self{client:Client::builder().timeout(Duration::from_secs(20)).connect_timeout(Duration::from_secs(5)).redirect(Policy::none()).build().map_err(|e|e.to_string())?,config,state:None,directory:directory.into(),backups:backups.into(),backup_state:None,warning:String::new()})
 }
 fn credential(c:&Configuration)->Result<String,String>{keyring::Entry::new("com.repaircenter.inventorymanager.online",&c.credential_id).map_err(|_|"Secure credential storage unavailable.")?.get_password().map_err(|_|"Online connection credential unavailable. Configure this installation again.".into())}
 fn request(&self,method:reqwest::Method,path:&str)->Result<reqwest::blocking::RequestBuilder,String>{let c=self.config.as_ref().ok_or("Online database is not configured. Open Online connection; no local database will be used.")?;Ok(self.client.request(method,format!("{}{path}",c.url)).bearer_auth(Self::credential(c)?))}
 fn decode(response:reqwest::blocking::Response)->Result<Value,String>{let code=response.status();let value:Value=response.json().map_err(|_|"The database service returned an invalid response.")?;if !code.is_success(){return Err(value["error"].as_str().unwrap_or("The database service rejected this request.").into());}Ok(value)}
 pub fn configure(&mut self,url:&str,token:&str)->Result<Value,String>{
  if self.directory.join("pending-online-operation.json").exists(){return Err("An earlier save is awaiting confirmation. Reconnect to the same database before changing the connection.".into());}
  let parsed=Url::parse(url.trim()).map_err(|_|"Enter a valid HTTPS service address.")?;
  if !(parsed.scheme()=="https" || cfg!(debug_assertions)&&parsed.scheme()=="http"&&matches!(parsed.host_str(),Some("127.0.0.1"|"localhost")))||!parsed.username().is_empty()||parsed.password().is_some()||parsed.query().is_some()||parsed.fragment().is_some()||parsed.path()!="/"{return Err("Use an HTTPS server origin without a path, password or query.".into());}
  if token.len()<32||token.len()>4096{return Err("Enter the private installation key supplied with this deployment.".into());}
  let url=parsed.as_str().trim_end_matches('/').to_string();
  let value=Self::decode(self.client.get(format!("{url}/v1/state")).bearer_auth(token).send().map_err(|_|"Cannot connect to the online database. No settings were changed.")?)?;
  let config=Configuration{url,credential_id:Uuid::new_v4().to_string()};
  let entry=keyring::Entry::new("com.repaircenter.inventorymanager.online",&config.credential_id).map_err(|_|"Secure credential storage unavailable.")?;entry.set_password(token).map_err(|_|"Could not save the connection key securely.")?;
  if let Err(e)=atomic_write(&self.directory.join("online-connection.json"),&serde_json::to_vec(&config).map_err(|e|e.to_string())?){let _=entry.delete_credential();return Err(e);}
  self.config=Some(config);self.state=Some(value["state"].clone());self.backup_state=None;Ok(self.status())
 }
 pub fn status(&self)->Value{json!({"configured":self.config.is_some(),"url":self.config.as_ref().map(|c|&c.url),"state":self.state,"backup_state":self.backup_state,"backup_directory":self.backups,"warning":self.warning})}
 pub fn execute(&mut self,action:&str,mut payload:Value)->Result<Value,String>{
  if action=="status"{return Ok(self.status());}
  if action=="snapshot"{
   self.recover_pending()?;
   let value=Self::decode(self.request(reqwest::Method::POST,"/v1/action")?.json(&json!({"action":"snapshot","payload":payload})).send().map_err(|_|"Online database disconnected. Changes are unavailable until reconnected.")?)?;
   self.state=Some(value["state"].clone());let mut result=value["result"].clone();result["online_state"]=value["state"].clone();
   for field in ["products","entries","sale_lines"]{if let Some(rows)=result[field].as_array_mut(){for row in rows{row["_online_state"]=value["state"].clone();}}}
   self.capture_if_changed();return Ok(result);
  }
  if !WRITES.contains(&action){return Err("Unknown online operation.".into());}
  let expected=payload.as_object_mut().and_then(|p|p.remove("_online_state")).filter(|v|v.is_object()).or_else(||self.state.clone()).ok_or("Load current inventory before saving.")?;
  let pending_path=self.directory.join("pending-online-operation.json");
  if pending_path.exists(){let old:Value=serde_json::from_slice(&fs::read(&pending_path).map_err(|e|e.to_string())?).map_err(|_|"Pending save record is damaged. Keep this file for recovery.")?;let result=self.send_mutation(&old)?;if old["action"]==action&&old["payload"]==payload{return Ok(result);}return Err("The earlier save was confirmed. Reload and review before making another change.".into());}
  let current=Self::decode(self.request(reqwest::Method::GET,"/v1/state")?.send().map_err(|_|"Online database disconnected. This change was not queued or saved.")?)?;
  if current["state"]!=expected{return Err("Inventory changed on another device. Refresh and review before saving.".into());}
  let request=json!({"action":action,"payload":payload,"operation_id":Uuid::new_v4().to_string(),"expected_revision":expected["revision"],"epoch":expected["epoch"]});
  atomic_write(&pending_path,&serde_json::to_vec(&request).map_err(|e|e.to_string())?)?;
  self.send_mutation(&request)
 }
 fn send_mutation(&mut self,request:&Value)->Result<Value,String>{
  let response=self.request(reqwest::Method::POST,"/v1/action")?.json(request).send().map_err(|_|"Connection interrupted: save confirmation is pending. Reconnect and refresh before trying a different operation.")?;
  let rejected=matches!(response.status().as_u16(),400|409|413);let decoded=Self::decode(response);
  if rejected||decoded.is_ok(){fs::remove_file(self.directory.join("pending-online-operation.json")).map_err(|_|"Save response received, but pending confirmation could not be cleared. Refresh before continuing.")?;}
  let value=decoded?;self.state=Some(value["state"].clone());self.capture_if_changed();Ok(value["result"].clone())
 }
 fn recover_pending(&mut self)->Result<(),String>{let path=self.directory.join("pending-online-operation.json");if path.exists(){let request=serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?).map_err(|_|"Pending save record is damaged. Keep it for recovery.")?;self.send_mutation(&request)?;}Ok(())}
 fn capture_if_changed(&mut self){if self.state==self.backup_state{return;}let destination=self.backups.join(format!("inventory-online-{}.db",Uuid::new_v4()));match self.backup(&destination){Ok(_)=>{self.warning.clear();},Err(e)=>self.warning=format!("Online data is available, but the local backup failed: {e}")}}
 pub fn backup(&mut self,destination:&Path)->Result<Value,String>{
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
 pub fn restore(&mut self,source:&Path)->Result<Value,String>{
  if self.directory.join("pending-online-operation.json").exists(){return Err("Confirm the pending save before restoring.".into());}
  let metadata=fs::metadata(source).map_err(|e|e.to_string())?;if metadata.len()>32*1024*1024{return Err("Database exceeds the current upload limit.".into());}
  let bytes=fs::read(source).map_err(|e|e.to_string())?;let state=self.state.clone().ok_or("Load inventory before restoring.")?;
  let value=Self::decode(self.request(reqwest::Method::POST,"/v1/restore")?.header("X-Inventory-State",state.to_string()).body(bytes).send().map_err(|_|"Restore confirmation was interrupted. Refresh to inspect the shared database before attempting restore again.")?)?;
  self.state=Some(value["state"].clone());self.capture_if_changed();Ok(value["result"].clone())
 }
}
fn atomic_write(path:&Path,bytes:&[u8])->Result<(),String>{let temporary=path.with_extension(format!("{}.tmp",Uuid::new_v4()));let result=(||{let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|e|e.to_string())?;file.write_all(bytes).map_err(|e|e.to_string())?;file.sync_all().map_err(|e|e.to_string())?;drop(file);fs::rename(&temporary,path).map_err(|e|e.to_string())})();if result.is_err(){let _=fs::remove_file(temporary);}result}
