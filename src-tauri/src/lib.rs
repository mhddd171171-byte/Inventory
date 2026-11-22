mod limited;
use serde_json::Value;
use std::sync::Mutex;
use tauri::Manager;
#[tauri::command]
async fn api(app:tauri::AppHandle,action:String,payload:Value)->Result<Value,String>{tauri::async_runtime::spawn_blocking(move||{if !payload.is_object(){return Err("Invalid request.".into());}let state=app.state::<Mutex<limited::Backend>>();let mut backend=state.lock().map_err(|_|"Limited Access is busy. Please reopen the app.")?;backend.dispatch(&action,&payload)}).await.map_err(|_|"Read-only request could not complete.".to_string())?}
pub fn run(){tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app,_,_|{if let Some(window)=app.get_webview_window("main"){let _=window.show();let _=window.set_focus();}})).setup(|app|{let directory=app.path().app_local_data_dir()?.join("data");let resources=app.path().resource_dir()?;let seed=resources.join("resources/limited/seed.json");let backend=limited::Backend::open(&directory,&seed).map_err(std::io::Error::other)?;app.manage(Mutex::new(backend));Ok(())}).invoke_handler(tauri::generate_handler![api]).run(tauri::generate_context!()).expect("Limited Access could not start");}
