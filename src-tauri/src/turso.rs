//! libSQL cloud adapter. Cloud rows are authoritative. Private SQLite staging
//! runs the existing domain validator; only its row delta is committed remotely.
//! No staging file is used after disconnection or retained as writable inventory.
use crate::store::{Store,ONLINE_TABLES};
use reqwest::{blocking::Client,redirect::Policy,Url};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::BTreeMap,io::{Read,Write},path::{Path,PathBuf},time::Duration};
use uuid::Uuid;

pub struct VerifiedSeed {pub path:PathBuf,pub state:Value,pub timestamp:String}
pub struct Turso {client:Client,url:String,token:String,directory:PathBuf,recovery:PathBuf}
pub struct Failure {pub message:String,pub definitive:bool}
type Result<T>=std::result::Result<T,Failure>;
fn unavailable(message:&str)->Failure{Failure{message:message.into(),definitive:false}}
fn rejected(message:String)->Failure{Failure{message,definitive:true}}
pub fn accepts(url:&str)->bool {url.starts_with("libsql://")||Url::parse(url).ok().and_then(|u|u.host_str().map(|h|h.ends_with(".turso.io"))).unwrap_or(false)}
fn endpoint(url:&str)->std::result::Result<String,String>{
 let converted=url.replacen("libsql://","https://",1);let u=Url::parse(&converted).map_err(|_|"Invalid Turso database URL.")?;
 if u.scheme()!="https"||!u.host_str().is_some_and(|h|h.ends_with(".turso.io"))||u.path()!="/"||u.query().is_some()||u.fragment().is_some()||!u.username().is_empty()||u.password().is_some(){return Err("Use the libsql:// database address from Turso Overview.".into());}
 Ok(u.as_str().trim_end_matches('/').into())
}
impl Turso {
 pub fn new(url:&str,token:String,directory:&Path)->std::result::Result<Self,String>{
  let client=Client::builder().connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(25)).redirect(Policy::none()).build().map_err(|e|e.to_string())?;
  Self::with_client(url,token,directory,client)
 }
 pub fn with_client(url:&str,token:String,directory:&Path,client:Client)->std::result::Result<Self,String>{Ok(Self{client,url:endpoint(url)?,token,directory:directory.into(),recovery:directory.join("recovery-backups")})}
 #[cfg(test)] pub fn test_connection(url:String,directory:&Path)->Self{Self{client:Client::builder().timeout(Duration::from_secs(10)).build().unwrap(),url,token:"test-only".into(),directory:directory.into(),recovery:directory.join("recovery-backups")}}
 pub fn recovery_directory(mut self,path:&Path)->Self{self.recovery=path.into();self}
 #[cfg(test)] pub fn explain_revision_guard(&self)->Result<usize>{let mut session=self.session();let results=session.batch(vec![stmt(&format!("EXPLAIN {REVISION_GUARD}"),vec![text("explain-only"),text("0")])],true)?;Ok(results[0]["rows"].as_array().ok_or_else(||unavailable("Invalid query plan."))?.len())}
 fn session(&self)->Session<'_>{Session{owner:self,url:self.url.clone(),baton:None,timeout:None}}
 fn temporary(&self)->Result<tempfile::TempDir>{tempfile::tempdir_in(&self.directory).map_err(|_|unavailable("Cannot create private inventory staging storage."))}
 pub fn initialize(&self)->std::result::Result<Value,String>{
  let temp=self.temporary().map_err(|e|e.message)?;let reference=Store::open(&temp.path().join("reference.db"))?;
  let mut session=self.session();let result=session.batch(vec![stmt("SELECT type,name,COALESCE(sql,'') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",vec![])],true).map_err(|e|e.message)?;
  if schema(&result[0]).map_err(|e|e.message)?!=reference.online_schema()?{return Err("The Turso database schema does not match Inventory Manager. No business records were changed.".into());}
  let epoch=Uuid::new_v4().to_string();let mut session=self.session();session.batch(vec![stmt("BEGIN IMMEDIATE",vec![]),stmt("INSERT OR IGNORE INTO migration_info(key,value) VALUES('online_epoch',?)",vec![text(&epoch)]),stmt("INSERT OR IGNORE INTO migration_info(key,value) VALUES('online_revision','0')",vec![]),stmt("INSERT OR IGNORE INTO migration_info(key,value) VALUES('online_schema_version','3')",vec![]),stmt("COMMIT",vec![])],true).map_err(|e|e.message)?;
  self.state().map_err(|e|e.message)
 }
 pub fn state(&self)->Result<Value>{self.state_and_clock().map(|(state,_)|state)}
 pub fn state_and_clock(&self)->Result<(Value,String)>{let mut session=self.session();let result=session.batch(vec![state_stmt()],true)?;let stamp=result[0]["rows"][0][2]["value"].as_str().ok_or_else(||unavailable("Database clock unavailable."))?.to_string();Ok((state(&result[0])?,stamp))}
 fn seeded_mirror(&self,seed:Option<VerifiedSeed>)->Result<(tempfile::TempDir,Store,Value,String)>{
  if let Some(seed)=seed {let temp=self.temporary()?;let path=temp.path().join("working.db");let candidate:std::result::Result<(Store,Value),String>=(||{let mut working=Store::open(&path)?;working.online_publish(&seed.path)?;if working.online_metadata()?!=seed.state{return Err("Stale local cache".into());}let dump=working.online_dump()?;Ok((working,dump))})();if let Ok((working,dump))=candidate{return Ok((temp,working,dump,seed.timestamp));}}
  self.mirror()
 }
 fn mirror(&self)->Result<(tempfile::TempDir,Store,Value,String)>{
  let temp=self.temporary()?;let mut store=Store::open(&temp.path().join("working.db")).map_err(rejected)?;
  let mut statements=vec![stmt("BEGIN",vec![]),stmt("SELECT type,name,COALESCE(sql,'') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",vec![])];
  for name in ONLINE_TABLES{statements.push(stmt(&format!("SELECT rowid,* FROM {name} ORDER BY rowid"),vec![]));}statements.push(stmt("SELECT strftime('%Y-%m-%dT%H:%M:%f+04:00','now','+4 hours') AS server_time",vec![]));statements.push(stmt("COMMIT",vec![]));
  let mut session=self.session();let results=session.batch(statements,true)?;
  if schema(&results[1])?!=store.online_schema().map_err(rejected)?{return Err(rejected("The online schema changed unexpectedly. Keep existing backups and contact support.".into()));}
  let mut tables=Vec::new();for (i,name)in ONLINE_TABLES.iter().enumerate(){let r=&results[i+2];let columns=r["cols"].as_array().ok_or_else(||unavailable("Invalid cloud columns."))?.iter().map(|c|c["name"].clone()).collect::<Vec<_>>();tables.push(json!({"name":name,"columns":columns,"rows":r["rows"]}));}
  let dump=json!(tables);store.online_import(&dump).map_err(rejected)?;let time=results[11]["rows"][0][0]["value"].as_str().ok_or_else(||unavailable("Database clock unavailable."))?.to_string();Ok((temp,store,dump,time))
 }
 pub fn snapshot(&self,payload:Value)->Result<Value>{self.snapshot_observe(payload,|_,_|{})}
 pub fn snapshot_observe(&self,mut payload:Value,mut confirmed:impl FnMut(&Store,&str))->Result<Value>{let (_temp,mut store,_,timestamp)=self.mirror()?;if payload["month"].is_null(){payload["month"]=json!(&timestamp[..7]);}let result=store.execute("snapshot",payload).map_err(rejected)?;confirmed(&store,&timestamp);Ok(json!({"result":result,"state":store.online_metadata().map_err(rejected)?}))}
 pub fn mutate(&self,request:&Value)->Result<Value>{self.mutate_observe(request,None,|_,_|{})}
 pub fn mutate_observe(&self,request:&Value,seed:Option<VerifiedSeed>,mut confirmed:impl FnMut(&Store,&str))->Result<Value>{
  let action=request["action"].as_str().ok_or_else(||rejected("Missing operation.".into()))?;
  if !["save_product","stock_entry","stock_out","edit_record","delete_record","damaged_return","warranty"].contains(&action){return Err(rejected("Operation is not permitted.".into()));}
  let id=request["operation_id"].as_str().ok_or_else(||rejected("Missing operation identifier.".into()))?;Uuid::parse_str(id).map_err(|_|rejected("Invalid operation identifier.".into()))?;
  let fingerprint=format!("{:x}",Sha256::digest(json!({"action":action,"payload":request["payload"],"epoch":request["epoch"],"expected_revision":request["expected_revision"]}).to_string().as_bytes()));
  let (_temp,mut working,before,timestamp)=self.seeded_mirror(seed)?;
  if let Some(receipt)=working.online_receipt(id).map_err(rejected)?{let response=receipt_result(&receipt,&fingerprint)?;confirmed(&working,&timestamp);return Ok(response);}
  let expected=working.online_metadata().map_err(rejected)?;
  if request["epoch"]!=expected["epoch"]||request["expected_revision"]!=expected["revision"]{return Err(rejected("Inventory changed on another device. Refresh and review before saving.".into()));}
  let revision=expected["revision"].as_u64().and_then(|n|n.checked_add(1)).filter(|n|*n<=9_007_199_254_740_991).ok_or_else(||rejected("Invalid database revision.".into()))?;
  let mut result=working.execute_online_at(action,request["payload"].clone(),&timestamp).map_err(rejected)?;
  let recovery=result["pre_edit_backup"].as_str().or_else(||result["pre_delete_backup"].as_str()).map(str::to_string);
  if let Some(source)=recovery{
   let source=PathBuf::from(source);if source.parent()!=Some(_temp.path()){return Err(rejected("Unexpected recovery snapshot path.".into()));}
   Store::validate_backup(&source).map_err(rejected)?;std::fs::create_dir_all(&self.recovery).map_err(|_|rejected("Cannot preserve a recovery backup. No online records were changed.".into()))?;
   let path=self.recovery.join(format!("before-{action}-{}.db",Uuid::new_v4()));let mut target=std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|_|rejected("Cannot create a recovery backup. No online records were changed.".into()))?;
   let bytes=std::fs::read(source).map_err(|_|rejected("Cannot read the recovery snapshot.".into()))?;target.write_all(&bytes).and_then(|_|target.sync_all()).map_err(|_|rejected("Recovery backup could not be completed. No online records were changed.".into()))?;result["recovery_backup_saved"]=json!(true);
  }
  if let Some(o)=result.as_object_mut(){o.remove("pre_edit_backup");o.remove("pre_delete_backup");}
  let response=json!({"result":result,"state":{"epoch":expected["epoch"],"revision":revision}});working.online_record(id,&fingerprint,&response,revision).map_err(rejected)?;
  let after=working.online_dump().map_err(rejected)?;
  match self.publish(&before,&after,&expected){
   Ok(())=>{confirmed(&working,&timestamp);Ok(response)},
   Err(error)if error.definitive=>{let (_temp,check,_,time)=self.mirror()?;if let Some(receipt)=check.online_receipt(id).map_err(rejected)?{let value=receipt_result(&receipt,&fingerprint)?;confirmed(&check,&time);return Ok(value);}Err(error)},
   Err(error)=>Err(error)
  }
 }
 fn publish(&self,before:&Value,after:&Value,expected:&Value)->Result<()>{
  let mut session=self.session();
  session.timeout=Some(Duration::from_secs(120));
  let mut statements=vec![stmt("PRAGMA foreign_keys=ON",vec![]),stmt("BEGIN IMMEDIATE",vec![]),stmt(REVISION_GUARD,vec![text(expected["epoch"].as_str().ok_or_else(||rejected("Invalid epoch.".into()))?),text(&expected["revision"].to_string())]),stmt("PRAGMA defer_foreign_keys=ON",vec![])];
  let guards=reference_guards().map_err(rejected)?;
  for (name,_) in &guards{statements.push(stmt(&format!("DROP TRIGGER \"{name}\""),vec![]));}
  let mut inserts=Vec::new();
  for (old,new)in before.as_array().unwrap().iter().zip(after.as_array().unwrap()){
   let name=old["name"].as_str().unwrap();if old["name"]!=new["name"]||old["columns"]!=new["columns"]{return Err(rejected("Inventory staging columns changed unexpectedly.".into()));}
   let old_rows=rows_by_id(old)?;let new_rows=rows_by_id(new)?;
   let removed=old_rows.iter().filter(|(id,row)|new_rows.get(*id)!=Some(*row)).map(|(id,_)|json!({"type":"integer","value":id})).collect::<Vec<_>>();
   for ids in removed.chunks(256){statements.push(stmt(&format!("DELETE FROM {name} WHERE rowid IN ({})",vec!["?";ids.len()].join(",")),ids.to_vec()));}
   let columns=new["columns"].as_array().unwrap().iter().map(|c|format!("\"{}\"",c.as_str().unwrap().replace('"',"\"\""))).collect::<Vec<_>>();
   let changed=new_rows.iter().filter(|(id,row)|old_rows.get(*id)!=Some(*row)).map(|(_,row)|row.as_array().unwrap()).collect::<Vec<_>>();
   // Bound variables below the portable 999-variable ceiling. One SQL INSERT
   // carries several rows, avoiding thousands of network/proxy executions.
   let group=(900/columns.len()).clamp(1,64);let values=format!("({})",vec!["?";columns.len()].join(","));
   for rows in changed.chunks(group){let sql=format!("INSERT INTO {name}({}) VALUES{}",columns.join(","),vec![values.as_str();rows.len()].join(","));inserts.push(stmt(&sql,rows.iter().flat_map(|row|row.iter().cloned()).collect()));}
  }
  statements.extend(inserts);for (_,sql)in guards{statements.push(stmt(&sql,vec![]));}
  // Chunks retain the same transaction. No COMMIT is sent after a failed chunk.
  statements.push(stmt("COMMIT",vec![]));
  // Keep typical restores in one atomic HTTP batch, avoiding idle expiry
  // between requests. Larger imports retain the same guarded transaction.
  let mut chunks=Vec::new();let mut chunk=Vec::new();let mut bytes=0;
  for statement in statements{let size=statement.to_string().len();if size>8*1024*1024{return Err(rejected("An inventory row exceeds the supported upload size.".into()));}if !chunk.is_empty()&&(chunk.len()>=128||bytes+size>8*1024*1024){chunks.push(std::mem::take(&mut chunk));bytes=0;}chunk.push(statement);bytes+=size;}
  if !chunk.is_empty(){chunks.push(chunk);}let count=chunks.len();for (index,chunk)in chunks.into_iter().enumerate(){session.batch(chunk,index+1==count)?;}Ok(())
 }
 pub fn backup(&self,destination:&Path)->Result<Value>{let (_temp,store,_,_)=self.mirror()?;let state=store.online_metadata().map_err(rejected)?;store.backup(destination).map_err(rejected)?;Ok(json!({"path":destination,"verified":true,"state":state}))}
 pub fn restore(&self,source:&Path,expected:&Value,recovery:&Path)->Result<Value>{
  let (_temp,mut working,before,_)=self.mirror()?;let current=working.online_metadata().map_err(rejected)?;
  if current!=*expected{return Err(rejected("Inventory changed. Reload before restoring.".into()));}
  working.backup(recovery).map_err(rejected)?;working.restore(source).map_err(rejected)?;let revision=current["revision"].as_u64().and_then(|n|n.checked_add(1)).ok_or_else(||rejected("Invalid revision.".into()))?;working.online_reset_epoch(revision).map_err(rejected)?;
  let after=working.online_dump().map_err(rejected)?;self.publish(&before,&after,&current)?;Ok(json!({"result":{"restored":true,"pre_restore_backup":recovery},"state":working.online_metadata().map_err(rejected)?}))
 }
}
const REVISION_GUARD:&str="INSERT INTO migration_info(key,value) SELECT 'online_revision',NULL WHERE (SELECT value FROM migration_info WHERE key='online_epoch') IS NOT ?1 OR (SELECT value FROM migration_info WHERE key='online_revision') IS NOT ?2";
fn text(s:&str)->Value{json!({"type":"text","value":s})}
fn stmt(sql:&str,args:Vec<Value>)->Value{json!({"sql":sql,"args":args,"want_rows":true})}
fn state_stmt()->Value{stmt("SELECT key,value,strftime('%Y-%m-%dT%H:%M:%f+04:00','now','+4 hours') AS server_time FROM migration_info WHERE key IN ('online_epoch','online_revision') ORDER BY key",vec![])}
fn state(result:&Value)->Result<Value>{let mut epoch=None;let mut revision=None;for row in result["rows"].as_array().ok_or_else(||unavailable("Invalid cloud state."))?{let key=row[0]["value"].as_str().unwrap_or("");let value=row[1]["value"].as_str().unwrap_or("");if key=="online_epoch"{Uuid::parse_str(value).map_err(|_|rejected("Invalid database epoch.".into()))?;epoch=Some(value.to_string());}if key=="online_revision"{revision=value.parse::<u64>().ok();}}Ok(json!({"epoch":epoch.ok_or_else(||rejected("Online database has not been initialized.".into()))?,"revision":revision.ok_or_else(||rejected("Invalid database revision.".into()))?}))}
fn schema(result:&Value)->Result<Value>{let rows=result["rows"].as_array().ok_or_else(||unavailable("Invalid schema response."))?;Ok(json!(rows.iter().map(|r|r.as_array().unwrap().iter().map(|v|v["value"].as_str().unwrap_or("").replace("\r\n","\n")).collect::<Vec<_>>()).collect::<Vec<_>>()))}
fn rows_by_id(table:&Value)->Result<BTreeMap<String,Value>>{let mut result=BTreeMap::new();for row in table["rows"].as_array().ok_or_else(||unavailable("Invalid cloud rows."))?{let id=row[0]["value"].as_str().ok_or_else(||unavailable("Invalid cloud row identifier."))?;id.parse::<i64>().map_err(|_|unavailable("Invalid cloud row identifier."))?;if result.insert(id.to_string(),row.clone()).is_some(){return Err(unavailable("Duplicate cloud row identifier."));}}Ok(result)}
fn receipt_result(receipt:&Value,fingerprint:&str)->Result<Value>{if receipt["request"]!=fingerprint{return Err(rejected("Operation identifier was already used for another request.".into()));}Ok(receipt["response"].clone())}
fn reference_guards()->std::result::Result<Vec<(String,String)>,String>{let conn=rusqlite::Connection::open_in_memory().map_err(|e|e.to_string())?;conn.execute_batch(include_str!("../migrations/003_dispositions.sql")).map_err(|e|e.to_string())?;let mut query=conn.prepare("SELECT name,sql FROM sqlite_master WHERE type='trigger' ORDER BY name").map_err(|e|e.to_string())?;let rows=query.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?.collect::<std::result::Result<Vec<_>,_>>().map_err(|e|e.to_string())?;Ok(rows)}
struct Session<'a>{owner:&'a Turso,url:String,baton:Option<String>,timeout:Option<Duration>}
impl Session<'_>{
 fn batch(&mut self,statements:Vec<Value>,close:bool)->Result<Vec<Value>>{
  let guarded=statements.get(2).is_some_and(|s|s["sql"]==REVISION_GUARD);
  let steps=statements.into_iter().enumerate().map(|(i,s)|{let mut step=json!({"stmt":s});if i>0{step["condition"]=json!({"type":"ok","step":i-1});}step}).collect::<Vec<_>>();let mut requests=vec![json!({"type":"batch","batch":{"steps":steps}})];if close{requests.push(json!({"type":"close"}));}
  let request=json!({"baton":self.baton,"requests":requests});
  if request.to_string().len()>16*1024*1024{return Err(rejected("Inventory transaction exceeds the supported request size.".into()));}
  let mut sending=self.owner.client.post(format!("{}/v2/pipeline",self.url)).bearer_auth(&self.owner.token).json(&request);if let Some(timeout)=self.timeout{sending=sending.timeout(timeout);}
  let response=sending.send().map_err(|e|unavailable(if e.is_timeout(){"Turso request timed out. Refresh to confirm the operation before retrying."}else{"Turso connection interrupted. Refresh to confirm the operation before retrying."}))?;
  if !response.status().is_success(){return Err(unavailable(&format!("Turso connection rejected (HTTP {}). Check the connection key and database availability.",response.status().as_u16())));}
  let mut bytes=Vec::new();response.take(32*1024*1024+1).read_to_end(&mut bytes).map_err(|_|unavailable("Turso response was interrupted."))?;if bytes.len()>32*1024*1024{return Err(unavailable("Turso response exceeds the supported size."));}
  let value:Value=serde_json::from_slice(&bytes).map_err(|_|unavailable("Invalid Turso response."))?;
  self.baton=value["baton"].as_str().map(str::to_string);
  if let Some(url)=value["base_url"].as_str(){self.url=endpoint(url).map_err(rejected)?;}
  let first=&value["results"][0];if first["type"]!="ok"{return Err(unavailable("Turso could not execute the database request."));}
  let result=&first["response"]["result"];if let Some(errors)=result["step_errors"].as_array(){if errors.iter().any(|e|!e.is_null()){if guarded&&errors.get(2).is_some_and(|e|!e.is_null()){return Err(rejected("Inventory changed on another device. Refresh before saving.".into()));}return Err(unavailable("Turso could not complete the transaction. Refresh to confirm its status before changing the request."));}}
  let values=result["step_results"].as_array().ok_or_else(||unavailable("Invalid Turso batch response."))?;if values.iter().any(Value::is_null){return Err(unavailable("Turso did not complete the database transaction."));}Ok(values.clone())
 }
}
impl Drop for Session<'_>{fn drop(&mut self){if let Some(baton)=self.baton.take(){let _=self.owner.client.post(format!("{}/v2/pipeline",self.url)).bearer_auth(&self.owner.token).timeout(Duration::from_secs(3)).json(&json!({"baton":baton,"requests":[{"type":"close"}]})).send();}}}
