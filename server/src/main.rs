use inventory_server::Service;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{env,io::Read,path::PathBuf};
use tiny_http::{Header,Method,Response,Server,StatusCode};
const LIMIT:usize=32*1024*1024;
fn main()->Result<(),Box<dyn std::error::Error>>{
 let token=env::var("INVENTORY_SERVER_TOKEN").map_err(|_|"Set a private INVENTORY_SERVER_TOKEN of at least 32 characters.")?;
 if token.len()<32{return Err("Server credential is too short.".into());}
 let directory=PathBuf::from(env::var("INVENTORY_DATA_DIR").unwrap_or("./data".into()));
 let mut service=Service::open(&directory).map_err(std::io::Error::other)?;
 let bind=env::var("INVENTORY_BIND").unwrap_or("127.0.0.1:8787".into());
 let server=Server::http(&bind).map_err(|e|std::io::Error::other(e.to_string()))?;
 eprintln!("Inventory service listening on {bind}; HTTPS reverse proxy required for public access.");
 let expected=Sha256::digest(format!("Bearer {token}"));
 service.checkpoint().map_err(std::io::Error::other)?;
 loop {
  let received=server.recv_timeout(std::time::Duration::from_secs(60))?;
  let Some(mut request)=received else {if let Err(_)=service.checkpoint(){eprintln!("Scheduled backup failed; inspect server storage.");}continue;};
  let supplied=request.headers().iter().find(|h|h.field.equiv("Authorization")).map(|h|h.value.as_str()).unwrap_or("");
  let digest=Sha256::digest(supplied);
  if expected.iter().zip(digest).fold(0u8,|d,(a,b)|d|(a^b))!=0 {let _=request.respond(Response::from_string("Unauthorized").with_status_code(401));continue;}
  let path=request.url().to_string();let method=request.method().clone();
  if method==Method::Get&&path=="/v1/backup" {
   let state=service.state().unwrap_or(Value::Null).to_string();let result=service.backup();match result {Ok(bytes)=>{let hash=format!("{:x}",Sha256::digest(&bytes));let _=request.respond(Response::from_data(bytes).with_header(Header::from_bytes("Content-Type","application/octet-stream").unwrap()).with_header(Header::from_bytes("X-Content-SHA256",hash).unwrap()).with_header(Header::from_bytes("X-Inventory-State",state).unwrap()));},Err(_)=>{let _=request.respond(Response::from_string("Backup unavailable").with_status_code(500));}}continue;
  }
  let mut body=Vec::new();
  if request.body_length().is_some_and(|n|n>LIMIT)||request.as_reader().take((LIMIT+1)as u64).read_to_end(&mut body).is_err()||body.len()>LIMIT{let _=request.respond(Response::from_string("Request too large").with_status_code(413));continue;}
  let result:Result<Value,String>=match (method,path.as_str()) {
   (Method::Get,"/v1/state")=>service.state().map(|state|json!({"state":state})),
   (Method::Post,"/v1/action")=>serde_json::from_slice::<Value>(&body).map_err(|_|"Invalid JSON.".into()).and_then(|r|if r["action"]=="snapshot"{service.snapshot(r["payload"].clone())}else{service.mutate(&r)}),
   (Method::Post,"/v1/restore")=>{
    let state=request.headers().iter().find(|h|h.field.equiv("X-Inventory-State")).and_then(|h|serde_json::from_str::<Value>(h.value.as_str()).ok());
    state.ok_or("Missing restore state.".into()).and_then(|s|service.restore(&body,&s))
   },
   _=>Err("Unknown endpoint.".into())
  };
  let (status,value)=match result{Ok(v)=>(200,v),Err(e)=>(409,json!({"error":e}))};
  let _=request.respond(Response::from_string(value.to_string()).with_status_code(StatusCode(status)).with_header(Header::from_bytes("Content-Type","application/json").unwrap()).with_header(Header::from_bytes("Cache-Control","no-store").unwrap()));
 }
}
