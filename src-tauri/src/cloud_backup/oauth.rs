use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD,Engine};
use std::{io::{Read,Write},net::TcpListener,time::Instant};
pub const SCOPE:&str="https://www.googleapis.com/auth/drive.file";
#[derive(Clone,Default,Deserialize)]
pub struct ClientConfig {pub client_id:String,#[serde(default)] pub client_secret:String}
impl ClientConfig {
 pub fn bundled()->Self{serde_json::from_str(include_str!(concat!(env!("OUT_DIR"),"/google-oauth-client.json"))).unwrap_or_default()}
 pub fn configured(&self)->bool{self.client_id.ends_with(".apps.googleusercontent.com")&&self.client_id.len()<256&&self.client_id.bytes().all(|b|b.is_ascii_alphanumeric()||b".-_".contains(&b))}
}
#[derive(Clone,Serialize,Deserialize)]
pub struct Tokens {pub refresh_token:String}
pub fn client()->Result<reqwest::blocking::Client,Failure>{reqwest::blocking::Client::builder().connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(45)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_|Failure::local("Cannot initialize secure Google Drive connection."))}
pub fn challenge(verifier:&str)->String{URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))}
pub fn browser(url:&str)->Result<(),String>{
 let u=reqwest::Url::parse(url).map_err(|_|"Invalid browser address.")?;
 if u.scheme()!="https"||!matches!(u.host_str(),Some("accounts.google.com"|"drive.google.com"))||!u.username().is_empty()||u.password().is_some(){return Err("Only Google sign-in and backup folders can be opened.".into());}
 #[cfg(windows)]{use std::os::windows::process::CommandExt;let exe=PathBuf::from(std::env::var_os("WINDIR").ok_or("Windows directory unavailable")?).join("System32/rundll32.exe");std::process::Command::new(exe).arg("url.dll,FileProtocolHandler").arg(url).creation_flags(0x08000000).spawn().map_err(|_|"Cannot open your default browser.")?;}
 #[cfg(target_os="macos")] {std::process::Command::new("/usr/bin/open").arg(url).spawn().map_err(|_|"Cannot open your default browser.")?;}
 Ok(())
}
fn callback(target:&str,state:&str)->Result<Option<String>,String>{
 let url=reqwest::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_|"Invalid sign-in response.")?;
 if url.path()!="/oauth/callback"{return Ok(None);}
 let pairs=url.query_pairs().collect::<Vec<_>>();
 let states=pairs.iter().filter(|(k,_)|k=="state").collect::<Vec<_>>();
 if states.len()!=1||states[0].1!=state{return Err("Invalid sign-in state.".into());}
 if pairs.iter().any(|(k,_)|k=="error"){return Err("Google sign-in was declined. Local backups are still active.".into());}
 let codes=pairs.iter().filter(|(k,_)|k=="code").collect::<Vec<_>>();
 if codes.len()!=1||codes[0].1.is_empty()||codes[0].1.len()>4096{return Err("Missing sign-in authorization code.".into());}
 Ok(Some(codes[0].1.to_string()))
}
pub fn sign_in(config:&ClientConfig,valid:impl Fn()->bool)->Result<Tokens,String>{
 let listener=TcpListener::bind(("127.0.0.1",0)).map_err(|_|"Cannot start the local sign-in callback.")?;listener.set_nonblocking(true).map_err(|_|"Cannot initialize sign-in.")?;
 let redirect=format!("http://127.0.0.1:{}/oauth/callback",listener.local_addr().map_err(|_|"Cannot initialize sign-in.")?.port());
 let verifier=format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple());let state=format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple());
 let mut url=reqwest::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
 url.query_pairs_mut().extend_pairs([("client_id",config.client_id.as_str()),("redirect_uri",redirect.as_str()),("response_type","code"),("scope",SCOPE),("access_type","offline"),("prompt","consent select_account"),("state",state.as_str()),("code_challenge",challenge(&verifier).as_str()),("code_challenge_method","S256")]);
 browser(url.as_str())?;let deadline=Instant::now()+Duration::from_secs(300);
 while valid()&&Instant::now()<deadline{
  if let Ok((mut stream,_))=listener.accept(){
   if stream.set_nonblocking(false).is_err(){continue;}
   let _=stream.set_read_timeout(Some(Duration::from_secs(2)));let _=stream.set_write_timeout(Some(Duration::from_secs(2)));
   let mut bytes=Vec::new();let mut buf=[0u8;1024];while bytes.len()<8192&&!bytes.windows(2).any(|b|b==b"\r\n"){match stream.read(&mut buf){Ok(0)|Err(_)=>break,Ok(n)=>bytes.extend_from_slice(&buf[..n])}}let request=String::from_utf8_lossy(&bytes);
   let parts=request.lines().next().unwrap_or("").split_whitespace().collect::<Vec<_>>();
   if parts.len()!=3||parts[0]!="GET"{let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\nInvalid request");continue;}
   match callback(parts[1],&state){
    Ok(Some(code))=>{
     let _=stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\n\r\nGoogle sign-in received. Return to Inventory Manager to check the backup status.");
     let _=stream.shutdown(std::net::Shutdown::Both);
     if !valid(){return Err("Sign-in cancelled.".into());}
     let c=client().map_err(|e|e.message)?;
     let r=c.post("https://oauth2.googleapis.com/token").form(&[("client_id",config.client_id.as_str()),("client_secret",config.client_secret.as_str()),("redirect_uri",redirect.as_str()),("code",code.as_str()),("code_verifier",verifier.as_str()),("grant_type","authorization_code")]).send().map_err(|_|"Cannot reach Google. Check internet and connect again.")?;
     let v=google_json(r).map_err(|e|e.message)?;
     if v.get("scope").and_then(Value::as_str).is_some_and(|s|!s.split_whitespace().any(|x|x==SCOPE)){return Err("Google Drive file permission was not granted.".into());}
     return Ok(Tokens{refresh_token:v["refresh_token"].as_str().filter(|s|!s.is_empty()).ok_or("Google did not return offline access. Connect again and approve backup access.")?.to_owned()});
    },Ok(None)=>{let _=stream.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\nNot found");},Err(e)=>{let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\nSign-in response rejected. Return to Inventory Manager.");if e.contains("declined"){return Err(e);}}
   }
  }
  std::thread::sleep(Duration::from_millis(100));
 }
 Err(if valid(){"Google sign-in timed out. Choose Connect to try again."}else{"Sign-in cancelled."}.into())
}
pub fn refresh(c:&reqwest::blocking::Client,config:&ClientConfig,tokens:&Tokens)->Result<String,Failure>{
 let r=c.post("https://oauth2.googleapis.com/token").form(&[("client_id",config.client_id.as_str()),("client_secret",config.client_secret.as_str()),("refresh_token",tokens.refresh_token.as_str()),("grant_type","refresh_token")]).send().map_err(network)?;
 let v=google_json(r)?;v["access_token"].as_str().map(str::to_owned).ok_or_else(||Failure::local("Google returned no access token. Please reconnect."))
}
pub fn vault_get(device:&str)->Result<Tokens,String>{let e=keyring::Entry::new("com.repaircenter.inventorymanager.google-drive",device).map_err(|_|"Secure credential storage is unavailable.")?;let text=e.get_password().map_err(|_|"Saved Google sign-in is unavailable. Please reconnect.")?;serde_json::from_str(&text).map_err(|_|"Saved Google sign-in is invalid. Please reconnect.".into())}
pub fn vault_set(device:&str,tokens:&Tokens)->Result<(),String>{keyring::Entry::new("com.repaircenter.inventorymanager.google-drive",device).map_err(|_|"Secure credential storage is unavailable.")?.set_password(&serde_json::to_string(tokens).map_err(|_|"Cannot save sign-in.")?).map_err(|_|"Cannot save Google sign-in in the system credential store.".into())}
pub fn vault_delete(device:&str)->Result<(),String>{match keyring::Entry::new("com.repaircenter.inventorymanager.google-drive",device).map_err(|_|"Secure credential storage is unavailable.")?.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(_)=>Err("Backups are paused, but the system could not remove the saved sign-in. Retry Disconnect.".into())}}
#[cfg(test)]mod tests{use super::*;
 #[test]fn pkce_matches_rfc7636(){assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),"E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");}
 #[test]fn callback_rejects_csrf_duplicate_and_wrong_paths(){assert_eq!(callback("/oauth/callback?state=good&code=abc%2B123","good").unwrap(),Some("abc+123".into()));assert!(callback("/oauth/callback?state=bad&code=x","good").is_err());assert!(callback("/oauth/callback?state=good&state=good&code=x","good").is_err());assert!(callback("/oauth/callback?state=good&code=x&code=y","good").is_err());assert!(callback("/favicon.ico","good").unwrap().is_none());assert!(callback("/oauth/callback?state=good&error=access_denied","good").unwrap_err().contains("declined"));}
}
