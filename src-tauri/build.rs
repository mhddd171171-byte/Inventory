fn main() {
 println!("cargo:rerun-if-env-changed=INVENTORY_GOOGLE_OAUTH_JSON");
 println!("cargo:rerun-if-env-changed=INVENTORY_REQUIRE_GOOGLE_DRIVE");
 let path=std::env::var_os("INVENTORY_GOOGLE_OAUTH_JSON").map(std::path::PathBuf::from).unwrap_or_else(||"google-oauth-client.json".into());
 println!("cargo:rerun-if-changed={}",path.display());
 let mut client=serde_json::json!({"client_id":"","client_secret":""});
 if path.is_file(){
  let input:serde_json::Value=serde_json::from_slice(&std::fs::read(&path).expect("Cannot read Desktop OAuth client JSON")).expect("Invalid OAuth client JSON");
  let v=input.get("installed").expect("Google OAuth client must be Desktop app (installed), not Web application");
  let id=v["client_id"].as_str().filter(|s|s.ends_with(".apps.googleusercontent.com")&&s.len()<256&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b".-_".contains(&b))).expect("Invalid Google Desktop client ID");
  let secret=v["client_secret"].as_str().unwrap_or("");assert!(secret.len()<256&&!secret.chars().any(char::is_control),"Invalid Google Desktop client configuration");
  client=serde_json::json!({"client_id":id,"client_secret":secret});
 }
 assert!(std::env::var_os("INVENTORY_REQUIRE_GOOGLE_DRIVE").is_none()||client["client_id"]!="","Google Drive release requires the registered Desktop OAuth client JSON");
 std::fs::write(std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("google-oauth-client.json"),client.to_string()).expect("Cannot prepare bundled OAuth configuration");
 tauri_build::build()
}
