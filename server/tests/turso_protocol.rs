//! Exercise the real cloud adapter against Hrana batch/transaction semantics
//! backed by real SQLite. Never uses the customer's online database or token.
pub use inventory_server::store;
#[path="../../src-tauri/src/turso.rs"] mod turso;
use rusqlite::{Connection,types::Value as SqlValue};
use serde_json::{json,Value};
use std::{collections::HashMap,sync::{Arc,atomic::{AtomicBool,AtomicUsize,Ordering}},time::Duration};

struct Mock {folder:tempfile::TempDir,url:String,stop:Arc<AtomicBool>,requests:Arc<AtomicUsize>,fail_insert:Arc<AtomicBool>,drop_commit:Arc<AtomicBool>,thread:Option<std::thread::JoinHandle<()>>}
impl Mock {
 fn new()->Self{
  let folder=tempfile::tempdir().unwrap();let database=folder.path().join("cloud.db");let mut store=store::Store::open(&database).unwrap();store.online_initialize().unwrap();drop(store);
  let server=tiny_http::Server::http("127.0.0.1:0").unwrap();let url=format!("http://{}",server.server_addr());
  let stop=Arc::new(AtomicBool::new(false));let requests=Arc::new(AtomicUsize::new(0));let fail_insert=Arc::new(AtomicBool::new(false));let drop_commit=Arc::new(AtomicBool::new(false));
  let (halt,count,fail,lose)=(stop.clone(),requests.clone(),fail_insert.clone(),drop_commit.clone());
  let thread=std::thread::spawn(move||{
   let mut sessions:HashMap<String,Connection>=HashMap::new();
   while !halt.load(Ordering::SeqCst){let Some(mut request)=server.recv_timeout(Duration::from_millis(100)).unwrap() else{continue};count.fetch_add(1,Ordering::SeqCst);let mut bytes=String::new();request.as_reader().read_to_string(&mut bytes).unwrap();let body:Value=serde_json::from_str(&bytes).unwrap();let baton=body["baton"].as_str().map(str::to_string).unwrap_or_else(||uuid::Uuid::new_v4().to_string());
    let mut conn=Some(sessions.remove(&baton).unwrap_or_else(||{let c=Connection::open(&database).unwrap();c.busy_timeout(Duration::from_secs(2)).unwrap();c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;").unwrap();c}));let mut replies=Vec::new();let mut committed=false;
    for operation in body["requests"].as_array().unwrap(){
     if operation["type"]=="close"{conn.take();replies.push(json!({"type":"ok","response":{"type":"close"}}));continue;}
     assert_eq!(operation["type"],"batch");let mut results:Vec<Value>=vec![];let mut errors=vec![];
     for step in operation["batch"]["steps"].as_array().unwrap(){
      if let Some(prior)=step["condition"]["step"].as_u64(){if results.get(prior as usize).is_none_or(Value::is_null){results.push(Value::Null);errors.push(Value::Null);continue;}}
      let sql=step["stmt"]["sql"].as_str().unwrap();let error=sql.starts_with("INSERT INTO stock_entries")&&fail.swap(false,Ordering::SeqCst);
      let outcome=if error{Err(rusqlite::Error::InvalidQuery)}else{execute(conn.as_ref().unwrap(),&step["stmt"])};
      match outcome{Ok(value)=>{committed|=sql=="COMMIT";results.push(value);errors.push(Value::Null);},Err(_)=>{results.push(Value::Null);errors.push(json!({"code":"SQLITE_CONSTRAINT","message":"Controlled SQL rejection"}));}}
     }
     replies.push(json!({"type":"ok","response":{"type":"batch","result":{"step_results":results,"step_errors":errors}}}));
    }
    let next=if let Some(conn)=conn{sessions.insert(baton.clone(),conn);Some(baton)}else{None};
    if committed&&lose.swap(false,Ordering::SeqCst){request.respond(tiny_http::Response::from_string("Lost commit response").with_status_code(500)).unwrap();continue;}
    request.respond(tiny_http::Response::from_string(json!({"baton":next,"base_url":null,"results":replies}).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
   }
  });Self{folder,url,stop,requests,fail_insert,drop_commit,thread:Some(thread)}
 }
 fn client(&self)->turso::Turso{turso::Turso::test_connection(self.url.clone(),self.folder.path())}
}
impl Drop for Mock{fn drop(&mut self){self.stop.store(true,Ordering::SeqCst);self.thread.take().unwrap().join().unwrap();}}
fn execute(conn:&Connection,stmt:&Value)->rusqlite::Result<Value>{
 let args=stmt["args"].as_array().unwrap().iter().map(|v|match v["type"].as_str().unwrap(){"integer"=>SqlValue::Integer(v["value"].as_str().unwrap().parse().unwrap()),"text"=>SqlValue::Text(v["value"].as_str().unwrap().into()),"null"=>SqlValue::Null,_=>panic!("Unexpected test argument")}).collect::<Vec<_>>();
 let mut statement=conn.prepare(stmt["sql"].as_str().unwrap())?;let columns=statement.column_names().iter().map(|n|json!({"name":n})).collect::<Vec<_>>();
 let (rows,affected)=if columns.is_empty(){(vec![],statement.execute(rusqlite::params_from_iter(args))?)}else{let mut source=statement.query(rusqlite::params_from_iter(args))?;let mut rows=vec![];while let Some(row)=source.next()?{let mut values=vec![];for index in 0..columns.len(){let value:SqlValue=row.get(index)?;values.push(match value{SqlValue::Integer(n)=>json!({"type":"integer","value":n.to_string()}),SqlValue::Text(s)=>json!({"type":"text","value":s}),SqlValue::Null=>json!({"type":"null"}),_=>panic!("Unexpected test cell")});}rows.push(values);}(rows,0)};
 Ok(json!({"cols":columns,"rows":rows,"affected_row_count":affected,"last_insert_rowid":null}))
}
fn good<T>(r:Result<T,turso::Failure>)->T{r.unwrap_or_else(|e|panic!("{}",e.message))}
fn entry()->Value{json!({"item_code":"1","name":"Battery بطارية","model_description":"Test fixture","quantity":1,"unit_cost_baisa":2000})}
fn request(state:&Value,action:&str,payload:Value)->Value{json!({"operation_id":uuid::Uuid::new_v4().to_string(),"epoch":state["epoch"],"expected_revision":state["revision"],"action":action,"payload":payload})}

#[test] fn one_atomic_publish_after_verified_cache_and_no_readback_after_commit(){
 let cloud=Mock::new();let client=cloud.client();let (state,time)=good(client.state_and_clock());let backup=cloud.folder.path().join("verified.db");good(client.backup(&backup));
 cloud.requests.store(0,Ordering::SeqCst);let mut confirmed=None;let req=request(&state,"stock_entry",entry());good(client.mutate_observe(&req,Some(turso::VerifiedSeed{path:backup,state,timestamp:time}),|store,_|confirmed=Some(store.snapshot(&json!({})).unwrap())));
 assert_eq!(cloud.requests.load(Ordering::SeqCst),1,"The write must only publish the guarded delta; no full readback or second backup download");assert_eq!(confirmed.unwrap()["summary"]["total_units"],1);
 assert_eq!(good(client.snapshot(json!({})))["result"]["summary"]["total_units"],1);
}
#[test] fn competing_cached_sales_cannot_sell_the_same_last_unit(){
 let cloud=Mock::new();let client=cloud.client();let initial=good(client.state());good(client.mutate(&request(&initial,"stock_entry",entry())));let (state,time)=good(client.state_and_clock());let backup=cloud.folder.path().join("before-sale.db");good(client.backup(&backup));
 let mut success=0;for cr in ["100","200"]{let req=request(&state,"stock_out",json!({"cr_number":cr,"lines":[{"item_code":"1","quantity":1,"unit_price_baisa":3000}]}));if client.mutate_observe(&req,Some(turso::VerifiedSeed{path:backup.clone(),state:state.clone(),timestamp:time.clone()}),|_,_|{}).is_ok(){success+=1;}}
 assert_eq!(success,1);let snapshot=good(client.snapshot(json!({})));assert_eq!(snapshot["result"]["summary"]["total_units"],0);assert_eq!(snapshot["result"]["summary"]["monthly_sales_baisa"],3000);
}
#[test] fn a_mid_batch_failure_does_not_commit_or_show_success(){
 let cloud=Mock::new();let client=cloud.client();let state=good(client.state());cloud.fail_insert.store(true,Ordering::SeqCst);let mut confirmed=false;assert!(client.mutate_observe(&request(&state,"stock_entry",entry()),None,|_,_|confirmed=true).is_err());assert!(!confirmed);
 assert_eq!(good(client.state()),state);assert_eq!(good(client.snapshot(json!({})))["result"]["summary"]["total_products"],0,"Both changed data and dropped immutable guards must roll back");
}
#[test] fn lost_commit_acknowledgement_replays_once_instead_of_duplicating(){
 let cloud=Mock::new();let client=cloud.client();let req=request(&good(client.state()),"stock_entry",entry());cloud.drop_commit.store(true,Ordering::SeqCst);let mut premature=false;assert!(client.mutate_observe(&req,None,|_,_|premature=true).is_err());assert!(!premature);
 let first=good(client.mutate(&req));let second=good(client.mutate(&req));assert_eq!(first,second);assert_eq!(good(client.snapshot(json!({})))["result"]["summary"]["total_units"],1);
}
