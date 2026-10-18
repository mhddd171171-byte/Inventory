use std::path::Path;

pub fn installed(executable:&Path, home:&Path)->bool {
    let bundle=executable.ancestors().find(|p|p.extension().is_some_and(|s|s=="app"));
    // Cargo development binaries are not distributable .app bundles.
    bundle.is_none_or(|app|app.parent()==Some(Path::new("/Applications")) || app.parent()==Some(home.join("Applications").as_path()))
}
#[cfg(target_os="macos")]
pub fn require_installation()->bool {
    let executable=match std::env::current_exe(){Ok(p)=>p,Err(_)=>return false};
    let home=match std::env::var_os("HOME"){Some(p)=>std::path::PathBuf::from(p),None=>return false};
    if installed(&executable,&home){return true;}
    // Do not open a database or start models when launched from a mounted DMG,
    // Downloads, or a temporary App Translocation location.
    let script=r#"display dialog "Install Inventory Manager first: drag Inventory Manager from the installer window into Applications, then open it from Applications. An app used directly from the installer is unavailable after the disk is ejected or the Mac restarts. Your existing inventory data has not been deleted or replaced." with title "Install Inventory Manager" buttons {"Quit", "Show Applications"} default button "Show Applications""#;
    if std::process::Command::new("/usr/bin/osascript").args(["-e",script]).output().is_ok_and(|r|r.status.success() && String::from_utf8_lossy(&r.stdout).contains("Show Applications")) {
        let _=std::process::Command::new("/usr/bin/open").arg("/Applications").status();
    }
    false
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn installer_download_and_translocated_launches_do_not_open_data() {
        let home=Path::new("/Users/customer");
        for p in ["/Volumes/Inventory Manager/Inventory Manager.app/Contents/MacOS/inventory-manager","/Users/customer/Downloads/Inventory Manager.app/Contents/MacOS/inventory-manager","/private/var/folders/AppTranslocation/Inventory Manager.app/Contents/MacOS/inventory-manager"] {assert!(!installed(Path::new(p),home));}
        assert!(installed(Path::new("/Applications/Inventory Manager.app/Contents/MacOS/inventory-manager"),home));
        assert!(installed(Path::new("/Users/customer/Applications/Inventory Manager.app/Contents/MacOS/inventory-manager"),home));
        assert!(installed(Path::new("/build/target/release/inventory-manager"),home));
    }
}
