//! Real HTTP server + two actual desktop gateways, isolated DBs/OS credentials.
#![cfg(windows)]
#[path="../../src-tauri/src/gateway.rs"] mod gateway;
pub use inventory_server::store;
#[path="../../src-tauri/src/turso.rs"] mod turso;
use serde_json::{json,Value};
use std::{process::{Command,Child},time::{Duration,Instant},fs};
struct Process(Child);
impl Drop for Process{fn drop(&mut self){let _=self.0.kill();let _=self.0.wait();}}
struct KeyCleanup(Vec<String>);
impl Drop for KeyCleanup{fn drop(&mut self){for id in &self.0{if let Ok(e)=keyring::Entry::new("com.repaircenter.inventorymanager.online",id){let _=e.delete_credential();}}}}
#[test] fn two_devices_share_data_securely_and_restore_invalidates_stale_edits(){
 let root=tempfile::tempdir().unwrap();let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();drop(listener);
 let url=format!("http://127.0.0.1:{port}");let token=format!("{}{}",uuid::Uuid::new_v4().simple(),uuid::Uuid::new_v4().simple());
 let mut process=Process(Command::new(env!("CARGO_BIN_EXE_inventory-server")).env("INVENTORY_BIND",format!("127.0.0.1:{port}")).env("INVENTORY_DATA_DIR",root.path().join("server")).env("INVENTORY_SERVER_TOKEN",&token).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap());
 let http=reqwest::blocking::Client::builder().timeout(Duration::from_secs(2)).build().unwrap();let start=Instant::now();while http.get(format!("{url}/v1/state")).bearer_auth(&token).send().is_err(){assert!(start.elapsed()<Duration::from_secs(15));std::thread::sleep(Duration::from_millis(50));}
 assert_eq!(http.get(format!("{url}/v1/state")).send().unwrap().status(),401);
 let mut keys=KeyCleanup(vec![]);let mut clients=vec![];
 for name in ["windows-a","windows-b"]{let data=root.path().join(name);fs::create_dir_all(&data).unwrap();let mut c=gateway::Gateway::open(&data,&data.join("backups")).unwrap();c.configure(&url,&token).unwrap();let config:Value=serde_json::from_slice(&fs::read(data.join("online-connection.json")).unwrap()).unwrap();assert!(!config.to_string().contains(&token));keys.0.push(config["credential_id"].as_str().unwrap().into());c.execute("snapshot",json!({})).unwrap();clients.push(c);assert!(!data.join("inventory.db").exists());}
 clients[0].execute("stock_entry",json!({"item_code":"1","name":"بطارية Battery","model_description":"Samsung S3","quantity":3,"unit_cost_baisa":2000})).unwrap();
 let stale=clients[1].execute("stock_entry",json!({"item_code":"1","quantity":9,"unit_cost_baisa":2000})).unwrap_err();assert!(stale.contains("changed"));
 let snapshot=clients[1].execute("snapshot",json!({})).unwrap();assert_eq!(snapshot["products"][0]["quantity"],3);assert_eq!(snapshot["products"][0]["name"],"بطارية Battery");
 let old=snapshot["products"][0].clone();clients[1].execute("stock_out",json!({"cr_number":"123","lines":[{"item_code":"1","quantity":1,"unit_price_baisa":25000}]})).unwrap();
 let mut editor=json!({"_online_state":old["_online_state"],"kind":"product","id":old["id"],"password":"1234","changes":{"item_code":"1","name":"stale name","model_description":"old"}});assert!(clients[0].execute("edit_record",editor.clone()).unwrap_err().contains("changed"));
 let snap=clients[0].execute("snapshot",json!({})).unwrap();assert_eq!(snap["summary"]["total_units"],2);let backup=root.path().join("portable.db");clients[0].backup(&backup).unwrap();
 clients[0].restore(&backup).unwrap();editor["_online_state"]=snap["online_state"].clone();assert!(clients[1].execute("edit_record",editor).is_err());
 let confirmed=clients[1].execute("snapshot",json!({})).unwrap();assert_eq!(confirmed["summary"]["total_units"],2);
 // Killing only the child we own simulates loss of the database service.
 process.0.kill().unwrap();process.0.wait().unwrap();
 let error=clients[0].execute("stock_entry",json!({"item_code":"1","quantity":1,"unit_cost_baisa":2000})).unwrap_err();assert!(error.contains("not queued"));
 assert!(!root.path().join("windows-a/pending-online-operation.json").exists());assert!(backup.exists());
}
