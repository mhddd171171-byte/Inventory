//! Mac data protection. Business SQL/schema remain in the existing Store.
use crate::store::Store;
use chrono::Utc;
use serde_json::{json, Value};
use std::{collections::HashSet, fs, path::{Path, PathBuf}};
use uuid::Uuid;

pub struct DataSafety {
    pub directory: PathBuf,
    extra: Option<PathBuf>,
    pub recovery_pending: bool,
    pub database_was_missing: bool,
    warning: String,
    last_capture_succeeded: bool,
    latest: Option<PathBuf>,
    recovery_marker: Option<PathBuf>,
}
impl DataSafety {
    pub fn new(directory: PathBuf, extra: Option<PathBuf>, database_existed: bool) -> Self {
        let previous = records(&directory);
        let extra_exists = extra.as_ref().is_some_and(|p| !records(p).is_empty());
        Self { recovery_marker:None, directory, extra, recovery_pending: !database_existed && (!previous.is_empty() || extra_exists),
            database_was_missing: !database_existed, warning: String::new(), last_capture_succeeded:false, latest: previous.first().cloned() }
    }
    pub fn protect_recovery(&mut self, data_dir:&Path)->Result<(),String> {
        let marker=data_dir.join("recovery-required.flag");
        if marker.exists(){self.recovery_pending=true;}
        if self.recovery_pending && !marker.exists(){fs::write(&marker,b"Earlier backups detected; restore before new entries.").map_err(|e|e.to_string())?;}
        self.recovery_marker=Some(marker);Ok(())
    }
    pub fn status(&self) -> Value {
        json!({"enabled":true,"version":env!("CARGO_PKG_VERSION"),"database_directory":self.recovery_marker.as_ref().and_then(|p|p.parent()),"directory":self.directory,"additional_directory":self.extra,
            "latest_backup":self.latest,"backup_count":records(&self.directory).len(),
            "warning":self.warning,"last_capture_succeeded":self.last_capture_succeeded,"recovery_pending":self.recovery_pending,
            "database_was_missing":self.database_was_missing})
    }
    pub fn set_extra(&mut self, directory: PathBuf, data_dir: &Path) -> Result<(),String> {
        if !directory.is_absolute() || !directory.is_dir() { return Err("Choose an existing local backup folder.".into()); }
        let selected=directory.canonicalize().map_err(|e|e.to_string())?;
        let data=data_dir.canonicalize().map_err(|e|e.to_string())?;
        if selected.starts_with(&data) || selected.components().any(|p|p.as_os_str().to_string_lossy().ends_with(".app")) {
            return Err("Choose a backup folder outside the application and its live data folder.".into());
        }
        self.extra=Some(selected); Ok(())
    }
    /// The business result stays successful if the already-committed save cannot
    /// be backed up. Returning a save error here would invite duplicate sales.
    pub fn execute(&mut self, store:&mut Store, action:&str, payload:Value)->Result<Value,String> {
        if self.recovery_pending { return Err("An earlier backup was found but the live database was missing. Use Backup & restore before entering new data; existing backups have been preserved.".into()); }
        let mut result=store.execute(action,payload)?;
        self.database_was_missing=false;
        self.capture(store);
        if !self.warning.is_empty() {result["backup_warning"]=json!(self.warning);}
        Ok(result)
    }
    pub fn restored(&mut self, store:&Store) {
        self.recovery_pending=false; self.database_was_missing=false; self.capture(store);
        if let Some(marker)=&self.recovery_marker {if marker.exists(){if let Err(e)=fs::remove_file(marker){self.warning.push_str(&format!(" Recovery marker could not be cleared: {e}"));}}}
    }
    pub fn startup(&mut self, store:&Store) {
        if self.database_was_missing || self.recovery_pending { return; }
        // Also catch a prior committed save whose backup failed. Compare the
        // complete business records, not SQLite header/WAL implementation bytes.
        // Reopening unchanged data must not rotate useful versions away.
        let previous=self.latest.clone();
        self.capture_inner(store,false);
        if let (Some(old),Some(current))=(previous,self.latest.clone()) {
            if old!=current && same_records(&old,&current) {
                let _=fs::remove_file(&current);
                if let Some(extra)=&self.extra {let duplicate=extra.join(current.file_name().unwrap());if duplicate!=old && duplicate!=current {let _=fs::remove_file(duplicate);}}
                self.latest=Some(old);return;
            }
        }
        if let Some(current)=&self.latest {if let Some(name)=current.file_name().and_then(|s|s.to_str()){prune(&self.directory,Some(name));if let Some(extra)=&self.extra{prune(extra,Some(name));}}}
    }
    pub fn capture(&mut self, store:&Store) {self.capture_inner(store,true);}
    fn capture_inner(&mut self, store:&Store, rotate:bool) {
        self.warning.clear();
        let name=format!("inventory-auto-{}-{}.db",Utc::now().format("%Y%m%dT%H%M%S%.9fZ"),Uuid::new_v4().simple());
        let mut successful=false;
        match save(store,&self.directory,&name) {
            Ok(path)=>{self.latest=Some(path);successful=true;if rotate {prune(&self.directory,Some(&name));}},
            Err(e)=>self.warning=format!("Inventory is saved, but its automatic backup could not be created: {e}. Use Backup & restore to save a separate copy."),
        }
        if let Some(extra)=&self.extra {
            // Never recreate an absent /Volumes/USB directory on the internal
            // disk when a removable drive has been disconnected.
            let attempt=if extra.is_dir(){save(store,extra,&name)}else{Err("the additional drive/folder is unavailable".into())};
            match attempt {
                Ok(path)=>{if !successful {self.latest=Some(path);} if rotate {prune(extra,Some(&name));}},
                Err(e)=>{if !self.warning.is_empty(){self.warning.push(' ');}self.warning.push_str(&format!("Additional backup unavailable: {e}. Reconnect the drive and choose Back up now."));}
            }
        }
        self.last_capture_succeeded=successful || self.latest.as_ref().and_then(|p|p.file_name()).is_some_and(|n|n==std::ffi::OsStr::new(&name));
    }
}
fn same_records(left:&Path,right:&Path)->bool {
    fn read(path:&Path)->rusqlite::Result<Vec<Vec<Vec<rusqlite::types::Value>>>> {
        let db=rusqlite::Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        db.pragma_update(None,"trusted_schema","OFF")?;
        let mut tables=Vec::new();
        for table in ["products","stock_entries","sales","sale_lines","fifo_allocations","legacy_archive","migration_info"] {
            let mut stmt=db.prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))?;
            let rows=stmt.query_map([],|row|(0..row.as_ref().column_count()).map(|i|row.get(i)).collect::<rusqlite::Result<Vec<rusqlite::types::Value>>>())?.collect::<rusqlite::Result<Vec<_>>>()?;
            tables.push(rows);
        }
        Ok(tables)
    }
    match (read(left),read(right)){(Ok(a),Ok(b))=>a==b,_=>false}
}
fn save(store:&Store, directory:&Path, name:&str)->Result<PathBuf,String> {
    #[cfg(unix)]
    let existed=directory.exists();
    fs::create_dir_all(directory).map_err(|e|e.to_string())?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; if !existed {fs::set_permissions(directory,fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;} }
    let path=directory.join(name);store.backup(&path)?;
    // SQLite's backup API includes committed WAL changes. Sync the finished
    // standalone copy before pruning an older generation.
    fs::OpenOptions::new().read(true).write(true).open(&path).and_then(|f|f.sync_all()).map_err(|e|e.to_string())?;
    Ok(path)
}
fn is_owned(name:&str)->bool {
    name.strip_prefix("inventory-auto-").and_then(|s|s.strip_suffix(".db")).and_then(|s|s.rsplit_once('-'))
        .is_some_and(|(time,id)| time.len()==26 && time.bytes().all(|b|b.is_ascii_digit()||matches!(b,b'T'|b'Z'|b'.')) && Uuid::parse_str(id).is_ok())
}
fn records(directory:&Path)->Vec<PathBuf> {
    let mut result=fs::read_dir(directory).into_iter().flatten().filter_map(Result::ok)
        .filter(|e|e.file_type().is_ok_and(|t|t.is_file()) && e.file_name().to_str().is_some_and(is_owned))
        .map(|e|e.path()).collect::<Vec<_>>();
    result.sort_by(|a,b|b.file_name().cmp(&a.file_name()));result
}
fn prune(directory:&Path, protected:Option<&str>) {
    let files=records(directory);let mut days=HashSet::new();
    for (i,path) in files.iter().enumerate() {
        let name=path.file_name().unwrap().to_string_lossy();let day=&name[15..23];
        let daily=days.len()<30 && days.insert(day.to_string());
        if i>=30 && !daily && protected!=Some(name.as_ref()) {let _=fs::remove_file(path);}
    }
}

pub fn validate_live_path(database:&Path)->Result<bool,String> {
    if database.exists(){return Ok(true);}
    if ["-wal","-shm"].iter().any(|s|PathBuf::from(format!("{}{s}",database.display())).exists()) {
        return Err("The inventory database is missing, but SQLite recovery files remain. Do not delete these files or start a new database. Preserve the complete data folder for recovery.".into());
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry()->Value {json!({"item_code":"11234","name":"شاشة سامسونج","model_description":"Samsung S3","quantity":5,"unit_cost_baisa":1500})}
    #[test] fn every_committed_change_is_a_portable_wal_complete_backup() {
        let dir=tempfile::tempdir().unwrap();let data=dir.path().join("data");fs::create_dir(&data).unwrap();
        let mut store=Store::open(&data.join("inventory.db")).unwrap();let root=dir.path().join("separate backups");let mut safe=DataSafety::new(root.clone(),None,true);
        safe.execute(&mut store,"stock_entry",entry()).unwrap();let first=safe.latest.clone().unwrap();
        safe.execute(&mut store,"stock_out",json!({"cr_number":"321","lines":[{"item_code":"11234","quantity":2,"unit_price_baisa":2500}]})).unwrap();
        assert_eq!(records(&root).len(),2);
        let product=store.snapshot(&json!({})).unwrap()["products"][0].clone();
        safe.execute(&mut store,"save_product",json!({"password":"1234","product":{"id":product["id"],"item_code":"11234","name":"شاشة سامسونج محدثة","model_description":"GT-I9300"}})).unwrap();assert_eq!(records(&root).len(),3);
        let mut other=Store::open(&dir.path().join("windows.db")).unwrap();other.restore(safe.latest.as_ref().unwrap()).unwrap();
        let snapshot=other.snapshot(&json!({})).unwrap();assert_eq!(snapshot["summary"]["total_units"],3);assert_eq!(snapshot["sale_lines"][0]["cr_number"],"321");
        other.restore(&first).unwrap();assert_eq!(other.snapshot(&json!({})).unwrap()["summary"]["total_units"],5);
        let before=records(&root).len();assert!(safe.execute(&mut store,"stock_out",json!({"cr_number":"321","lines":[]})).is_err());assert_eq!(records(&root).len(),before);
        if let Some(path)=std::env::var_os("INVENTORY_MAC_UPDATE_FIXTURE") {store.backup(Path::new(&path)).unwrap();}
        drop(store); fs::remove_file(&data.join("inventory.db")).unwrap();assert!(safe.latest.as_ref().unwrap().exists());
    }
    #[test] fn backup_failure_does_not_report_a_committed_sale_as_failed() {
        let dir=tempfile::tempdir().unwrap();let blocked=dir.path().join("blocked");fs::write(&blocked,b"not a directory").unwrap();
        let mut store=Store::open(&dir.path().join("inventory.db")).unwrap();let mut safe=DataSafety::new(blocked,None,true);
        let result=safe.execute(&mut store,"stock_entry",entry()).unwrap();assert!(result["backup_warning"].as_str().unwrap().contains("Inventory is saved"));
        assert_eq!(store.snapshot(&json!({})).unwrap()["summary"]["total_units"],5);
    }
    #[test] fn missing_database_with_history_blocks_empty_overwrite_and_restore_recovers() {
        let dir=tempfile::tempdir().unwrap();let root=dir.path().join("backups");let path=dir.path().join("old.db");let mut store=Store::open(&path).unwrap();
        let mut safe=DataSafety::new(root.clone(),None,true);safe.execute(&mut store,"stock_entry",entry()).unwrap();let backup=safe.latest.clone().unwrap();
        let mut empty=Store::open(&dir.path().join("fresh.db")).unwrap();let mut recovery=DataSafety::new(root.clone(),None,false);assert!(recovery.recovery_pending);
        recovery.protect_recovery(dir.path()).unwrap();let mut restarted=DataSafety::new(root,None,true);restarted.protect_recovery(dir.path()).unwrap();assert!(restarted.recovery_pending);
        assert!(recovery.execute(&mut empty,"stock_entry",entry()).is_err());recovery.startup(&empty);assert_eq!(records(&recovery.directory).len(),1);
        empty.restore(&backup).unwrap();recovery.restored(&empty);assert!(!recovery.recovery_pending);assert!(!dir.path().join("recovery-required.flag").exists());assert_eq!(empty.snapshot(&json!({})).unwrap()["summary"]["total_units"],5);
        let missing=dir.path().join("missing.db");fs::write(dir.path().join("missing.db-wal"),b"keep").unwrap();assert!(validate_live_path(&missing).is_err());assert!(!missing.exists());
    }
    #[test] fn startup_protects_existing_data_and_disconnected_extra_drive_warns_without_losing_primary_copy() {
        let dir=tempfile::tempdir().unwrap();let data=dir.path().join("data");fs::create_dir(&data).unwrap();let mut store=Store::open(&data.join("inventory.db")).unwrap();store.execute("stock_entry",entry()).unwrap();
        let primary=dir.path().join("backups");let extra=dir.path().join("USB");fs::create_dir(&extra).unwrap();let mut safe=DataSafety::new(primary.clone(),None,true);
        assert!(safe.set_extra(data.clone(),&data).is_err());safe.set_extra(extra.clone(),&data).unwrap();safe.startup(&store);assert_eq!(records(&primary).len(),1);assert_eq!(records(&extra).len(),1);safe.startup(&store);assert_eq!(records(&primary).len(),1);
        store.execute("stock_entry",entry()).unwrap();safe.startup(&store);assert_eq!(records(&primary).len(),2);
        for p in records(&extra){fs::remove_file(p).unwrap();}fs::remove_dir(&extra).unwrap();
        safe.execute(&mut store,"stock_entry",entry()).unwrap();assert_eq!(records(&primary).len(),3);assert!(!extra.exists());assert!(safe.status()["warning"].as_str().unwrap().contains("Additional backup unavailable"));
    }
    #[test] fn retention_keeps_recent_and_daily_history_and_never_removes_manual_files() {
        let dir=tempfile::tempdir().unwrap();let manual=dir.path().join("manual.db");fs::write(&manual,b"keep").unwrap();
        for i in 0..70 {let name=format!("inventory-auto-20261018T1200{:02}.000000000Z-{}.db",i%60,Uuid::new_v4().simple());fs::write(dir.path().join(name),b"snapshot").unwrap();}
        assert_eq!(records(dir.path()).len(),70);prune(dir.path(),None);assert_eq!(records(dir.path()).len(),30);assert!(manual.exists());
    }
}
