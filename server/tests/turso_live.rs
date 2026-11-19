//! Explicit opt-in tests only. Credential is read from the OS store, never env,
//! command line, source, logs or checked-in configuration.
#![cfg(windows)]
pub use inventory_server::store;
#[path="../../src-tauri/src/turso.rs"] mod turso;
#[path="../../src-tauri/src/gateway.rs"] mod gateway;
use serde_json::{json,Value};
const URL:&str="libsql://inventory-test-mhdddforai.aws-ap-south-1.turso.io";
fn connection(path:&std::path::Path)->turso::Turso{let token=keyring::Entry::new("com.repaircenter.inventorymanager.online","turso-test-development").unwrap().get_password().expect("Development credential unavailable; do not print or paste it in logs.");turso::Turso::new(URL,token,path).unwrap()}
fn good<T>(result:std::result::Result<T,turso::Failure>)->T{result.unwrap_or_else(|e|panic!("{}",e.message))}
#[test] #[ignore="Validates a private local recovery copy only; no cloud writes"]
fn validate_local_schema_recovery_backup(){let path=std::path::PathBuf::from(std::env::var_os("INVENTORY_SCHEMA_RECOVERY_BACKUP").expect("Set the private recovery-copy path"));store::Store::validate_backup(&path).unwrap();println!("Recovery copy passed the application's full schema, relationship, FIFO and financial validation.");}
#[test] #[ignore="Read-only metadata timing; does not create, edit or restore any business record"]
fn live_readonly_connection_pool_timing(){
 let d=tempfile::tempdir().unwrap();let token=keyring::Entry::new("com.repaircenter.inventorymanager.online","turso-test-development").unwrap().get_password().unwrap();
 let mut fresh=Vec::new();for _ in 0..3{let client=turso::Turso::new(URL,token.clone(),d.path()).unwrap();let start=std::time::Instant::now();good(client.state());fresh.push(start.elapsed().as_millis());}
 let shared=reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(20)).build().unwrap();let mut pooled=Vec::new();for _ in 0..3{let client=turso::Turso::with_client(URL,token.clone(),d.path(),shared.clone()).unwrap();let start=std::time::Instant::now();good(client.state());pooled.push(start.elapsed().as_millis());}
 println!("{}",json!({"business_records_changed":false,"fresh_connection_ms":fresh,"shared_connection_ms":pooled}));
}
#[test] #[ignore="Only compiles EXPLAIN bytecode; does not execute an INSERT or modify any record"]
fn live_readonly_revision_guard_sql_is_supported(){let d=tempfile::tempdir().unwrap();let c=connection(d.path());assert!(good(c.explain_revision_guard())>0);println!("Cloud revision guard compiled with EXPLAIN only; no business records or metadata changed.");}
#[test] #[ignore="Requires explicit authorization and the test database credential in the OS store"]
fn live_turso_connect_snapshot_and_portable_backup(){
 let d=tempfile::tempdir().unwrap();let c=connection(d.path());let state=c.initialize().unwrap();assert!(state["epoch"].is_string());
 let snap=good(c.snapshot(json!({})));assert_eq!(snap["result"]["summary"]["total_products"],0);assert_eq!(snap["result"]["summary"]["total_units"],0);
 let backup=d.path().join("verified.db");good(c.backup(&backup));store::Store::validate_backup(&backup).unwrap();
 println!("Live Turso connection, empty snapshot, schema validation and portable backup passed.");
}

#[test] #[ignore="Writes disposable records to inventory-test, then restores its verified empty baseline"]
fn live_turso_warranty_replay_concurrency_and_restore(){
 assert_eq!(std::env::var("INVENTORY_DESTRUCTIVE_TURSO_TESTS").as_deref(),Ok("EMPTY-DISPOSABLE-DATABASE-ONLY"),"The shared database is now in use. Run writes only against an explicitly isolated disposable database.");
 let directory=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.build/turso-live-validation");std::fs::create_dir_all(&directory).unwrap();let c=connection(&directory);c.initialize().unwrap();
 let baseline=directory.join(format!("before-live-test-{}.db",uuid::Uuid::new_v4()));let initial=good(c.snapshot(json!({})));assert_eq!(initial["result"]["summary"]["total_products"],0,"Only run against the empty test database");good(c.backup(&baseline));
 let run=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
  let save=|action:&str,payload:Value|{let state=good(c.state());good(c.mutate(&json!({"operation_id":uuid::Uuid::new_v4().to_string(),"action":action,"payload":payload,"epoch":state["epoch"],"expected_revision":state["revision"]})))};
  save("stock_entry",json!({"item_code":"999991","name":"TEST Battery بطارية","model_description":"Disposable connection test","quantity":2,"unit_cost_baisa":5000}));
  save("stock_out",json!({"cr_number":"99901","lines":[{"item_code":"999991","quantity":1,"unit_price_baisa":25000}]}));
  let snap=good(c.snapshot(json!({})));let original=snap["result"]["sale_lines"][0]["id"].as_str().unwrap().to_string();
  save("stock_entry",json!({"item_code":"999992","name":"TEST Replacement","model_description":"Disposable connection test","quantity":1,"unit_cost_baisa":2000}));
  save("warranty",json!({"password":"1234","line_id":original,"quantity":1,"new_cr_number":"99902","replacement_item_code":"999992","resolution":"replacement","refund_baisa":0,"extra_baisa":0,"note":"Disposable warranty test"}));
  let snap=good(c.snapshot(json!({})));let line=snap["result"]["sale_lines"].as_array().unwrap().iter().find(|l|l["id"]==original).unwrap();assert_eq!(line["adjusted_profit_baisa"],18000);assert_eq!(snap["result"]["summary"]["monthly_sales_baisa"],25000);assert_eq!(snap["result"]["summary"]["monthly_units_sold"],1);
  let state=good(c.state());let request=json!({"operation_id":uuid::Uuid::new_v4().to_string(),"epoch":state["epoch"],"expected_revision":state["revision"],"action":"stock_entry","payload":{"item_code":"999993","name":"TEST Final unit","model_description":"Disposable concurrency test","quantity":1,"unit_cost_baisa":1000}});let first=good(c.mutate(&request));assert_eq!(first,good(c.mutate(&request)));
  let expected=good(c.state());let barrier=std::sync::Arc::new(std::sync::Barrier::new(2));let mut handles=vec![];
  for cr in ["99903","99904"]{let expected=expected.clone();let barrier=barrier.clone();handles.push(std::thread::spawn(move||{let temp=tempfile::tempdir().unwrap();let client=connection(temp.path());barrier.wait();client.mutate(&json!({"operation_id":uuid::Uuid::new_v4().to_string(),"epoch":expected["epoch"],"expected_revision":expected["revision"],"action":"stock_out","payload":{"cr_number":cr,"lines":[{"item_code":"999993","quantity":1,"unit_price_baisa":1000}]}})).is_ok()}));}
  let successes=handles.into_iter().filter(|h|h.thread().id()!=std::thread::current().id()).map(|h|h.join().unwrap()).filter(|ok|*ok).count();assert_eq!(successes,1);
  let backup=directory.join(format!("verified-warranty-{}.db",uuid::Uuid::new_v4()));good(c.backup(&backup));store::Store::validate_backup(&backup).unwrap();
 }));
 // Always attempt cleanup, including after an assertion fails. Preserve both
 // baseline and pre-cleanup copies if connectivity prevents recovery.
 let current=good(c.state());let recovery=directory.join(format!("before-cleanup-{}.db",uuid::Uuid::new_v4()));good(c.restore(&baseline,&current,&recovery));let final_snapshot=good(c.snapshot(json!({})));assert_eq!(final_snapshot["result"]["summary"]["total_products"],0);assert_eq!(final_snapshot["result"]["sale_lines"].as_array().unwrap().len(),0);
 assert!(run.is_ok(),"Live validation failed; empty baseline was restored successfully");
 println!("Live Turso stock entry, sale, warranty, exact profit, replay, concurrent sale and restore passed; test records removed.");
}

#[test] #[ignore="Tests two real desktop gateways against the empty Turso test DB, then removes test records"]
fn live_two_desktop_gateways_refresh_and_backup(){
 assert_eq!(std::env::var("INVENTORY_DESTRUCTIVE_TURSO_TESTS").as_deref(),Ok("EMPTY-DISPOSABLE-DATABASE-ONLY"),"The shared database is now in use. Run writes only against an explicitly isolated disposable database.");
 let directory=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.build/turso-desktop-validation");std::fs::create_dir_all(&directory).unwrap();let c=connection(&directory);c.initialize().unwrap();let before=good(c.snapshot(json!({})));assert_eq!(before["result"]["summary"]["total_products"],0);let baseline=directory.join(format!("before-{}.db",uuid::Uuid::new_v4()));good(c.backup(&baseline));
 let run=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
  let mut clients=Vec::new();for name in ["device-a","device-b"]{let path=directory.join(name);std::fs::create_dir_all(&path).unwrap();std::fs::write(path.join("online-connection.json"),json!({"url":URL,"credential_id":"turso-test-development"}).to_string()).unwrap();let mut client=gateway::Gateway::open(&path,&path.join("backups")).unwrap();let snapshot=client.execute("snapshot",json!({})).unwrap();assert_eq!(snapshot["summary"]["total_products"],0);clients.push(client);}
  clients[0].execute("stock_entry",json!({"item_code":"999999","name":"TEST Screen شاشة","model_description":"Disposable desktop gateway test","quantity":2,"unit_cost_baisa":1100})).unwrap();
  let second=clients[1].execute("snapshot",json!({})).unwrap();assert_eq!(second["products"][0]["quantity"],2);assert_eq!(clients[1].execute("snapshot",json!({})).unwrap(),second,"Unchanged revision uses only a verified cache");
  clients[0].execute("snapshot",json!({})).unwrap();clients[0].execute("stock_entry",json!({"item_code":"999999","quantity":1,"unit_cost_baisa":1100})).unwrap();
  let latest=clients[1].execute("snapshot",json!({})).unwrap();assert_eq!(latest["products"][0]["quantity"],3);assert_ne!(latest["online_state"],second["online_state"]);
  let conflict=clients[1].execute("edit_record",json!({"_online_state":second["online_state"],"kind":"product","id":second["products"][0]["id"],"password":"1234","changes":{"item_code":"999999","name":"stale edit","model_description":"not saved"}})).unwrap_err();assert!(conflict.contains("changed"));
  let path=directory.join("device-b");let mut restarted=gateway::Gateway::open(&path,&path.join("backups")).unwrap();let from_backup=restarted.execute("snapshot",json!({})).unwrap();assert_eq!(from_backup["products"][0]["quantity"],3);assert_eq!(from_backup["products"][0]["name"],"TEST Screen شاشة");assert_eq!(restarted.status()["read_source"],"verified_backup");
  let index_path=path.join("online-backup-index.json");let mut index:Value=serde_json::from_slice(&std::fs::read(&index_path).unwrap()).unwrap();index["sha256"]=json!("corrupt checksum");std::fs::write(&index_path,index.to_string()).unwrap();let mut rebuilt=gateway::Gateway::open(&path,&path.join("backups")).unwrap();let current=rebuilt.execute("snapshot",json!({})).unwrap();assert_eq!(current["products"][0]["quantity"],3);assert_eq!(rebuilt.status()["read_source"],"cloud");
  let missing=directory.join("missing-credential");std::fs::create_dir_all(&missing).unwrap();std::fs::write(missing.join("online-connection.json"),json!({"url":URL,"credential_id":uuid::Uuid::new_v4().to_string()}).to_string()).unwrap();let mut no_access=gateway::Gateway::open(&missing,&path.join("backups")).unwrap();assert!(no_access.execute("snapshot",json!({})).is_err(),"Local data must not bypass a failed cloud connection");
  for name in ["device-a","device-b"]{let path=directory.join(name);assert!(!path.join("inventory.db").exists());assert!(!path.join("pending-online-operation.json").exists());let backups=std::fs::read_dir(path.join("backups")).unwrap().filter_map(std::result::Result::ok).collect::<Vec<_>>();assert!(!backups.is_empty());for backup in backups{store::Store::validate_backup(&backup.path()).unwrap();}}
 }));
 let state=good(c.state());good(c.restore(&baseline,&state,&directory.join(format!("before-cleanup-{}.db",uuid::Uuid::new_v4()))));assert_eq!(good(c.snapshot(json!({})))["result"]["summary"]["total_products"],0);assert!(run.is_ok(),"Desktop validation failed; empty baseline restored");println!("Two desktop gateways read/write the same Turso data, refresh, reject stale edits and create validated .db backups; test records removed.");
}
