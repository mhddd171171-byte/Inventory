use super::*;
use std::io::Read;
const FILES:&str="https://www.googleapis.com/drive/v3/files";
const UPLOAD:&str="https://www.googleapis.com/upload/drive/v3/files";
const MARKER:&str="inventory-manager-backup-v1";
pub fn valid_id(id:&str)->bool{!id.is_empty()&&id.len()<=200&&id.bytes().all(|b|b.is_ascii_alphanumeric()||b"_-".contains(&b))}
pub struct Drive<'a>{pub client:&'a reqwest::blocking::Client,pub token:&'a str,pub valid:Box<dyn Fn()->bool+'a>,files:String,upload:String}
impl<'a> Drive<'a>{
 pub fn new(client:&'a reqwest::blocking::Client,token:&'a str,valid:Box<dyn Fn()->bool+'a>)->Self{Self{client,token,valid,files:FILES.into(),upload:UPLOAD.into()}}
 fn session_allowed(&self,url:&reqwest::Url)->bool{
  if !url.username().is_empty()||url.password().is_some(){return false;}
  #[cfg(test)] if let Ok(base)=reqwest::Url::parse(&self.upload){if base.scheme()=="http"&&base.host_str()==Some("127.0.0.1")&&url.origin()==base.origin()&&url.path().starts_with("/upload/drive/"){return true;}}
  url.scheme()=="https"&&url.host_str()==Some("www.googleapis.com")&&url.port_or_known_default()==Some(443)&&url.path().starts_with("/upload/drive/")
 }
 fn check(&self)->Result<(),Failure>{if (self.valid)(){Ok(())}else{Err(Failure::local("Backup operation cancelled."))}}
 fn get(&self,url:&str)->Result<Value,Failure>{self.check()?;google_json(self.client.get(url).bearer_auth(self.token).send().map_err(network)?)}
 fn list(&self,q:&str)->Result<Vec<Value>,Failure>{
  let mut result=Vec::new();let mut page=String::new();
  for _ in 0..100{
   self.check()?;let mut u=reqwest::Url::parse(&self.files).unwrap();u.query_pairs_mut().extend_pairs([("q",q),("pageSize","1000"),("fields","nextPageToken,files(id,name,mimeType,parents,appProperties,size,sha256Checksum,createdTime,trashed)"),("orderBy","createdTime desc")]);if !page.is_empty(){u.query_pairs_mut().append_pair("pageToken",&page);}
   let v=self.get(u.as_str())?;result.extend(v["files"].as_array().ok_or_else(||Failure::local("Google returned an invalid backup list."))?.iter().cloned());
   match v["nextPageToken"].as_str(){Some(p) if !p.is_empty()=>page=p.into(),_=>return Ok(result)}
  }Err(Failure::local("Too many Drive backup records. No older files were removed."))
 }
 fn create_folder(&self,name:&str,parent:Option<&str>,device:Option<&str>)->Result<String,Failure>{
  self.check()?;let mut meta=json!({"name":name,"mimeType":"application/vnd.google-apps.folder","appProperties":{"app":MARKER,"role":if device.is_some(){"device"}else{"root"}}});
  if let Some(d)=device{meta["appProperties"]["device"]=json!(d);}if let Some(p)=parent{meta["parents"]=json!([p]);}
  let v=google_json(self.client.post(&self.files).query(&[("fields","id")]).bearer_auth(self.token).json(&meta).send().map_err(network)?)?;
  v["id"].as_str().filter(|s|valid_id(s)).map(str::to_owned).ok_or_else(||Failure::local("Google returned an invalid folder identifier."))
 }
 pub fn folder(&self,device:&str)->Result<String,Failure>{
  if Uuid::parse_str(device).is_err(){return Err(Failure::local("Invalid backup device identifier."));}
  let root_q=format!("trashed=false and mimeType='application/vnd.google-apps.folder' and appProperties has {{key='app' and value='{MARKER}'}} and appProperties has {{key='role' and value='root'}}");
  let roots=self.list(&root_q)?;let root=match roots.first().and_then(|v|v["id"].as_str()).filter(|s|valid_id(s)){Some(id)=>id.into(),None=>self.create_folder("Inventory Manager Backups",None,None)?};
  let q=format!("trashed=false and '{root}' in parents and mimeType='application/vnd.google-apps.folder' and appProperties has {{key='app' and value='{MARKER}'}} and appProperties has {{key='role' and value='device'}} and appProperties has {{key='device' and value='{device}'}}");
  let folders=self.list(&q)?;match folders.first().and_then(|v|v["id"].as_str()).filter(|s|valid_id(s)){Some(id)=>Ok(id.into()),None=>self.create_folder(&format!("{} - {}",device_label(),&device[..8]),Some(&root),Some(device))}
 }
 pub fn account(&self)->Result<Option<String>,Failure>{let v=self.get(&format!("{}/about?fields=user(emailAddress)",self.files.trim_end_matches("/files")))?;Ok(v["user"]["emailAddress"].as_str().map(str::to_owned))}
 pub fn generate_id(&self)->Result<String,Failure>{let v=self.get(&format!("{}/generateIds?count=1&space=drive&type=files",self.files))?;v["ids"][0].as_str().filter(|s|valid_id(s)).map(str::to_owned).ok_or_else(||Failure::local("Google returned an invalid backup identifier."))}
 pub fn upload(&self,device:&str,folder:&str,pending:&Pending,path:&Path)->Result<(),Failure>{
  let id=pending.remote_id.as_deref().filter(|s|valid_id(s)).ok_or_else(||Failure::local("Missing remote backup identifier."))?;
  if !valid_id(folder){return Err(Failure::local("Invalid backup folder."));}
  // Pre-generated, persisted Drive IDs make uncertain upload responses safe to retry.
  self.check()?;let existing=self.client.get(format!("{}/{id}",self.files)).query(&[("fields","id,size,sha256Checksum,appProperties,parents,trashed")]).bearer_auth(self.token).send().map_err(network)?;
  let exists=if existing.status()==reqwest::StatusCode::NOT_FOUND{false}else{
   let v=google_json(existing)?;if !owned(&v,device,folder)||v["appProperties"]["job"]!=pending.id{return Err(Failure::local("The remote backup identifier does not belong to this operation. No file was overwritten."));}
   if verified(&v,pending){return Ok(());}true
  };
  let metadata=json!({"id":id,"name":format!("Inventory-{}-{}-{}.db",device_label(),pending.created_at.replace(':',"-"),&pending.id[..8]),"mimeType":"application/vnd.sqlite3","parents":[folder],"appProperties":{"app":MARKER,"role":"backup","device":device,"job":pending.id,"sha256":pending.sha256}});
  self.check()?;let request=if exists{self.client.patch(format!("{}/{id}",self.upload))}else{self.client.post(&self.upload)};
  let body=if exists{json!({})}else{metadata};
  let response=request.query(&[("uploadType","resumable"),("fields","id,size,sha256Checksum,parents,appProperties")]).bearer_auth(self.token).header("X-Upload-Content-Type","application/vnd.sqlite3").header("X-Upload-Content-Length",pending.bytes).json(&body).send().map_err(network)?;
  if !response.status().is_success(){google_json(response)?;return Err(Failure::local("Google could not start the upload."));}
  let location=response.headers().get(reqwest::header::LOCATION).and_then(|h|h.to_str().ok()).ok_or_else(||Failure::local("Google returned no upload session."))?;
  let url=reqwest::Url::parse(location).map_err(|_|Failure::local("Invalid Google upload session."))?;
  if !self.session_allowed(&url){return Err(Failure::local("Unexpected upload destination was rejected."));}
  self.check()?;let file=fs::File::open(path).map_err(|_|Failure::local("The queued backup is unavailable. Local inventory is unchanged."))?;
  let v=google_json(self.client.put(url).timeout(Duration::from_secs(300)).bearer_auth(self.token).header(reqwest::header::CONTENT_TYPE,"application/vnd.sqlite3").header(reqwest::header::CONTENT_LENGTH,pending.bytes).body(reqwest::blocking::Body::new(file)).send().map_err(network)?)?;
  if !owned(&v,device,folder)||v["id"]!=id||v["size"].as_str().and_then(|s|s.parse::<u64>().ok())!=Some(pending.bytes){return Err(Failure::local("Google upload confirmation did not match the backup. It will be checked again before retrying."));}
  if !verified(&v,pending){
   // Some Drive files do not expose sha256Checksum; verify actual remote bytes.
   self.check()?;let mut response=self.client.get(format!("{}/{id}",self.files)).query(&[("alt","media")]).timeout(Duration::from_secs(300)).bearer_auth(self.token).send().map_err(network)?;
   if !response.status().is_success(){google_json(response)?;return Err(Failure::local("Cannot verify uploaded backup."));}
   let mut hash=Sha256::new();let count=std::io::copy(&mut response.by_ref().take(pending.bytes+1),&mut HashWriter(&mut hash)).map_err(|_|Failure::local("Upload verification was interrupted. The backup remains queued."))?;
   if count!=pending.bytes||format!("{:x}",hash.finalize())!=pending.sha256{return Err(Failure::local("Uploaded backup checksum did not match. Local backups have been kept."));}
  }Ok(())
 }
 pub fn retain_history(&self,device:&str,folder:&str,protected:&str)->Result<(),Failure>{
  if !valid_id(folder){return Err(Failure::local("Invalid backup folder."));}
  let files=self.list(&format!("trashed=false and '{folder}' in parents and appProperties has {{key='app' and value='{MARKER}'}} and appProperties has {{key='role' and value='backup'}} and appProperties has {{key='device' and value='{device}'}}"))?;
  for id in prune_ids(&files,device,folder,protected){self.check()?;google_json(self.client.patch(format!("{}/{id}",self.files)).query(&[("fields","id")]).bearer_auth(self.token).json(&json!({"trashed":true})).send().map_err(network)?)?;}
  Ok(())
 }
}
pub(super) struct HashWriter<'a>(pub(super) &'a mut Sha256);
impl std::io::Write for HashWriter<'_>{fn write(&mut self,b:&[u8])->std::io::Result<usize>{self.0.update(b);Ok(b.len())}fn flush(&mut self)->std::io::Result<()>{Ok(())}}
fn verified(v:&Value,p:&Pending)->bool{v["size"].as_str().and_then(|s|s.parse::<u64>().ok())==Some(p.bytes)&&v["sha256Checksum"].as_str()==Some(p.sha256.as_str())}
fn owned(v:&Value,device:&str,folder:&str)->bool{v["trashed"]!=true&&v["appProperties"]["app"]==MARKER&&v["appProperties"]["role"]=="backup"&&v["appProperties"]["device"]==device&&v["parents"].as_array().is_some_and(|p|p.iter().any(|x|x==folder))}
fn prune_ids(files:&[Value],device:&str,folder:&str,protected:&str)->Vec<String>{
 let mut files=files.iter().filter(|v|owned(v,device,folder)&&v["id"].as_str().is_some_and(valid_id)&&v["appProperties"]["job"].as_str().is_some_and(|s|Uuid::parse_str(s).is_ok())&&v["createdTime"].as_str().is_some_and(|s|chrono::DateTime::parse_from_rfc3339(s).is_ok())).collect::<Vec<_>>();
 files.sort_by(|a,b|b["createdTime"].as_str().cmp(&a["createdTime"].as_str()));let mut days=std::collections::HashSet::new();let mut remove=Vec::new();
 for(i,v)in files.iter().enumerate(){let day=&v["createdTime"].as_str().unwrap()[..10];let daily=days.len()<30&&days.insert(day.to_owned());let id=v["id"].as_str().unwrap();if i>=30&&!daily&&id!=protected{remove.push(id.to_owned());}}remove
}
#[cfg(test)]mod tests{use super::*;
 use std::io::{Read,Write};
 fn mock(responses:Vec<(u16,String,Option<String>)>)->(String,std::thread::JoinHandle<Vec<(String,Vec<u8>)>>){
  let listener=std::net::TcpListener::bind(("127.0.0.1",0)).unwrap();listener.set_nonblocking(true).unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());let host=origin.clone();
  let task=std::thread::spawn(move||{let mut requests=Vec::new();for(status,body,location)in responses{let end=Instant::now()+Duration::from_secs(10);let mut stream=loop{match listener.accept(){Ok((s,_))=>break s,Err(_) if Instant::now()<end=>std::thread::sleep(Duration::from_millis(5)),Err(e)=>panic!("Mock HTTP request missing: {e}")}};stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();let mut bytes=Vec::new();let mut buf=[0;4096];let head=loop{let n=stream.read(&mut buf).unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n]);if let Some(i)=bytes.windows(4).position(|b|b==b"\r\n\r\n"){break i+4;}assert!(bytes.len()<16384);};let headers=String::from_utf8_lossy(&bytes[..head]);let line=headers.lines().next().unwrap().to_owned();let size=headers.lines().find_map(|l|l.split_once(':').filter(|(k,_)|k.eq_ignore_ascii_case("content-length")).map(|(_,v)|v.trim().parse::<usize>().unwrap())).unwrap_or(0);while bytes.len()<head+size{let n=stream.read(&mut buf).unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n]);}requests.push((line,bytes[head..head+size].to_vec()));let extra=location.map(|s|format!("Location: {}\r\n",s.replace("{origin}",&host))).unwrap_or_default();write!(stream,"HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{extra}\r\n{body}",body.len()).unwrap();}requests});(origin,task)
 }
 fn file(dir:&Path)->(Pending,PathBuf,String){let device=Uuid::new_v4().to_string();let path=dir.join("queued.db");fs::write(&path,b"SQLite format 3\0transport fixture").unwrap();let(hash,bytes)=hash_file(&path).unwrap();(Pending{id:Uuid::new_v4().to_string(),source:"inventory-auto-test.db".into(),created_at:"2026-10-25T12:00:00Z".into(),sha256:hash,bytes,remote_id:Some("file123".into())},path,device)}
 fn meta(p:&Pending,device:&str)->Value{json!({"id":"file123","size":p.bytes.to_string(),"sha256Checksum":p.sha256,"parents":["folder"],"appProperties":{"app":MARKER,"role":"backup","device":device,"job":p.id}})}
 fn test_client()->reqwest::blocking::Client{reqwest::blocking::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap()}
 #[test]fn resumable_upload_streams_the_exact_snapshot_and_is_idempotent(){
  let dir=tempfile::tempdir().unwrap();let(p,path,device)=file(dir.path());let(body,task)=mock(vec![(404,"{}".into(),None),(200,"{}".into(),Some("{origin}/upload/drive/v3/files?upload_id=test".into())),(200,meta(&p,&device).to_string(),None)]);let c=test_client();let mut d=Drive::new(&c,"fixture-access",Box::new(||true));d.files=format!("{body}/drive/v3/files");d.upload=format!("{body}/upload/drive/v3/files");d.upload(&device,"folder",&p,&path).unwrap();let requests=task.join().unwrap();assert!(requests[0].0.starts_with("GET /drive/v3/files/file123?"));assert!(requests[1].0.starts_with("POST /upload/drive/v3/files?"));let metadata:Value=serde_json::from_slice(&requests[1].1).unwrap();assert_eq!(metadata["id"],"file123");assert_eq!(metadata["appProperties"]["job"],p.id);assert_eq!(requests[2].1,fs::read(&path).unwrap());
  let(origin,task)=mock(vec![(200,meta(&p,&device).to_string(),None)]);d.files=format!("{origin}/drive/v3/files");d.upload(&device,"folder",&p,&path).unwrap();assert_eq!(task.join().unwrap().len(),1);
 }
 #[test]fn retry_updates_only_the_same_owned_incomplete_file(){let dir=tempfile::tempdir().unwrap();let(p,path,device)=file(dir.path());let mut partial=meta(&p,&device);partial["size"]=json!("0");partial["sha256Checksum"]=Value::Null;let(origin,task)=mock(vec![(200,partial.to_string(),None),(200,"{}".into(),Some("{origin}/upload/drive/v3/files?upload_id=test".into())),(200,meta(&p,&device).to_string(),None)]);let c=test_client();let mut d=Drive::new(&c,"fixture-access",Box::new(||true));d.files=format!("{origin}/drive/v3/files");d.upload=format!("{origin}/upload/drive/v3/files");d.upload(&device,"folder",&p,&path).unwrap();let req=task.join().unwrap();assert!(req[1].0.starts_with("PATCH /upload/drive/v3/files/file123?"));}
 #[test]fn credentials_are_never_sent_to_an_unexpected_upload_destination(){let dir=tempfile::tempdir().unwrap();let(p,path,device)=file(dir.path());let(origin,task)=mock(vec![(404,"{}".into(),None),(200,"{}".into(),Some("https://attacker.invalid/upload/drive/v3/files".into()))]);let c=test_client();let mut d=Drive::new(&c,"fixture-access",Box::new(||true));d.files=format!("{origin}/drive/v3/files");d.upload=format!("{origin}/upload/drive/v3/files");assert!(d.upload(&device,"folder",&p,&path).unwrap_err().message.contains("Unexpected"));assert_eq!(task.join().unwrap().len(),2);}
 #[test]fn checksum_failure_keeps_the_local_file_and_does_not_claim_success(){let dir=tempfile::tempdir().unwrap();let(p,path,device)=file(dir.path());let mut bad=meta(&p,&device);bad["sha256Checksum"]=json!("wrong");let(origin,task)=mock(vec![(404,"{}".into(),None),(200,"{}".into(),Some("{origin}/upload/drive/v3/files?upload_id=test".into())),(200,bad.to_string(),None),(200,"corrupt".into(),None)]);let c=test_client();let mut d=Drive::new(&c,"fixture-access",Box::new(||true));d.files=format!("{origin}/drive/v3/files");d.upload=format!("{origin}/upload/drive/v3/files");assert!(d.upload(&device,"folder",&p,&path).unwrap_err().message.contains("checksum"));assert!(path.is_file());assert_eq!(task.join().unwrap().len(),4);}
 #[test]fn expired_tokens_storage_full_and_server_errors_are_classified_without_token_leaks(){for(status,value,auth,word)in[(401,json!({"error":"invalid_grant","refresh_token":"must-not-leak"}),true,"Reconnect"),(403,json!({"error":{"errors":[{"reason":"storageQuotaExceeded"}]}}),false,"full"),(503,json!({"error":"unavailable"}),false,"busy")]{let(origin,task)=mock(vec![(status,value.to_string(),None)]);let r=test_client().get(origin).send().unwrap();let error=google_json(r).unwrap_err();assert_eq!(error.auth,auth);assert!(error.message.to_lowercase().contains(&word.to_lowercase()));assert!(!error.message.contains("must-not-leak"));task.join().unwrap();}}
 #[test]fn ids_cannot_inject_queries_or_urls(){for id in ["","../files","x' or true","https://evil.test","x?alt=media"]{assert!(!valid_id(id));}assert!(valid_id("1abc_23-XYZ"));}
 #[test]fn retention_only_trashes_owned_generations_and_keeps_history(){let device=Uuid::new_v4().to_string();let mut files=Vec::new();for i in 0..90{files.push(json!({"id":format!("id{i}"),"parents":["folder"],"appProperties":{"app":MARKER,"role":"backup","device":device,"job":Uuid::new_v4().to_string()},"createdTime":format!("2026-10-{:02}T{:02}:00:00Z",1+i/3,i%3)}));}let unrelated=json!({"id":"unrelated","parents":["folder"],"appProperties":{"app":"another-app"},"createdTime":"2020-01-01T00:00:00Z"});files.push(unrelated);let ids=prune_ids(&files,&device,"folder","id0");assert!(!ids.contains(&"unrelated".into()));assert!(!ids.contains(&"id0".into()));assert!(!ids.contains(&"id89".into()));assert!(!ids.contains(&"id2".into()));assert!(ids.contains(&"id1".into()));assert_eq!(ids.len(),39);}
}
