mod store;
mod assistant;
#[cfg(any(target_os="macos",test))]
mod desktop_settings;
#[cfg(test)]
mod assistant_acceptance;

use assistant::{Assistant, RuntimeShutdown, VoiceShutdown};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, sync::{Mutex, atomic::{AtomicBool, Ordering}}, time::Duration};
use store::Store;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

struct AppState {
    #[cfg(target_os="macos")]
    preferences: Mutex<desktop_settings::Preferences>,
    #[cfg(target_os="macos")]
    data_dir: PathBuf,
    store: Mutex<Store>,
    assistant: Mutex<Assistant>,
    assistant_status: Mutex<Value>,
    runtime_shutdown: RuntimeShutdown,
    voice_shutdown: VoiceShutdown,
    manager_shutdown: AtomicBool,
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> String {
    "An application operation failed. Restart the application safely.".into()
}
fn field(payload: &Value, name: &str) -> Result<String, String> {
    payload[name].as_str().filter(|s| !s.trim().is_empty()).map(str::to_string).ok_or_else(|| format!("Missing {name}."))
}
fn session_id(payload: &Value) -> Result<String, String> {
    let id = field(payload, "session_id")?;
    if id.len() > 96 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Invalid conversation identifier.".into());
    }
    Ok(id)
}

/// Tauri uses a resources subdirectory on Windows and Contents/Resources on macOS.
/// Resolve afresh on every launch; saved development paths are never needed.
fn bundled_directory(resource_dir: &Path, executable_dir: &Path, component: &str) -> PathBuf {
    let candidates = [resource_dir.join("resources").join(component), resource_dir.join(component), executable_dir.join("resources").join(component)];
    candidates.iter().find(|p| p.is_dir()).cloned().unwrap_or_else(|| candidates[0].clone())
}
fn runtime_name(stem: &str) -> String {
    if cfg!(windows) { format!("{stem}.exe") } else { stem.into() }
}
fn safe_resource_filename(value: Option<&str>, default: &str) -> String {
    value.filter(|v| !v.is_empty() && !v.contains(['/', '\\', ':']) && *v != "." && *v != "..").unwrap_or(default).to_string()
}
fn resolve_config(saved: &Value, bundled: &Path) -> Value {
    let manifest: Value = fs::read(bundled.join("runtime.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    let profile = if saved["performance_profile"] == "low_memory" { "low_memory" } else { "balanced" };
    let available = std::thread::available_parallelism().map(|n| n.get() as u64).unwrap_or(2);
    let threads = manifest["cpu_threads"].as_u64().unwrap_or(4).clamp(1, 8).min(available);
    let context = if profile == "low_memory" { 2048 } else { manifest["context_size"].as_u64().unwrap_or(4096).clamp(2048, 8192) };
    let model = safe_resource_filename(manifest["model_file"].as_str(), "Qwen3-4B-Instruct-2507-Q4_K_M.gguf");
    json!({"model_source":"bundled","performance_profile":profile,"context_size":context,"cpu_threads":threads,
        "server_path":bundled.join(runtime_name("llama-server")).to_string_lossy(),"model_path":bundled.join(model).to_string_lossy()})
}
fn write_config(data_dir: &Path, config: &Value) -> Result<(), String> {
    let saved = json!({"model_source":"bundled","performance_profile":config["performance_profile"]});
    // This small file contains preferences only. Model/resource paths are resolved
    // from the installation on every launch, including after an application update.
    fs::write(data_dir.join("settings.json"), serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?).map_err(|e| format!("Cannot save local settings: {e}"))
}
fn cached_status(state: &AppState) -> Result<Value, String> { let mut status=state.assistant_status.lock().map_err(lock_error)?.clone();status["voice_interrupted"]=json!(state.voice_shutdown.interrupted());Ok(status) }
fn cache_status(state: &AppState, status: Value) { if let Ok(mut cached) = state.assistant_status.lock() { *cached = status; } }

#[cfg(target_os="macos")]
fn remember_ai_pause(state: &AppState, status: &Value) {
    if status["enabled"] == false && matches!(status["state"].as_str(),Some("failed"|"timed_out")) {
        if let Ok(mut preferences) = state.preferences.lock() {
            if preferences.ai_enabled {
                preferences.ai_enabled = false;
                preferences.ai_pause_reason = status["state"].as_str().unwrap_or("failed").into();
                if let Err(error) = preferences.save(&state.data_dir) { eprintln!("{error}"); }
            }
        }
    }
}

fn start_assistant_manager(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        while !state.manager_shutdown.load(Ordering::Acquire) {
            // Model inference/voice may own this lock. Cached status and SQLite
            // have separate locks so inventory stays usable throughout loading.
            if let Ok(mut assistant) = state.assistant.try_lock() {
                let status = assistant.tick();
                #[cfg(target_os="macos")]
                remember_ai_pause(&state, &status);
                cache_status(&state, status);
            }
            for _ in 0..10 {
                if state.manager_shutdown.load(Ordering::Acquire) { return; }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
}

#[tauri::command]
async fn api(app: tauri::AppHandle, action: String, payload: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || dispatch(&app, &action, payload)).await.map_err(|e| e.to_string())?
}

fn validate_export(payload: &Value, excel: bool) -> Result<(Vec<u8>, String), String> {
    let bytes: Vec<u8> = serde_json::from_value(payload["bytes"].clone()).map_err(|_| "Invalid document data.")?;
    if bytes.len() > 50_000_000 || (if excel { !bytes.starts_with(b"PK\x03\x04") } else { !bytes.starts_with(b"%PDF-") }) {
        return Err("Invalid or oversized report document.".into());
    }
    let extension = if excel { ".xlsx" } else { ".pdf" };
    let fallback = format!("inventory-report{extension}");
    let name = payload["suggested_name"].as_str().filter(|n| n.ends_with(extension) && !n.contains(['/', '\\', ':']) && n.len() <= 200).unwrap_or(&fallback);
    Ok((bytes, name.to_owned()))
}

fn dispatch(app: &tauri::AppHandle, action: &str, payload: Value) -> Result<Value, String> {
    if !payload.is_object() { return Err("Invalid request.".into()); }
    let state = app.state::<AppState>();
    match action {
        #[cfg(target_os="macos")]
        "settings_get" => Ok(serde_json::to_value(state.preferences.lock().map_err(lock_error)?.clone()).map_err(|e|e.to_string())?),
        #[cfg(target_os="macos")]
        "settings_theme" => {
            let mut saved = state.preferences.lock().map_err(lock_error)?;
            let mut next = saved.clone();
            next.set_theme(&field(&payload,"theme")?)?;
            next.save(&state.data_dir)?; *saved = next;
            Ok(json!({"theme":saved.theme}))
        },
        #[cfg(target_os="macos")]
        "assistant_set_enabled" => {
            let enabled = payload["enabled"].as_bool().ok_or("AI enabled must be true or false.")?;
            // Match the manager's assistant -> preferences lock order. A retry
            // cannot otherwise race the manager persisting the previous failure.
            let mut starting = if enabled {Some(state.assistant.lock().map_err(lock_error)?)} else {None};
            {
                let mut preferences = state.preferences.lock().map_err(lock_error)?;
                let mut next = preferences.clone();
                next.ai_enabled = enabled; next.ai_pause_reason = if enabled {String::new()} else {"manual".into()};
                next.save(&state.data_dir)?; *preferences = next;
            }
            if !enabled {
                state.runtime_shutdown.pause();
                let mut status = cached_status(&state)?;
                status["enabled"]=json!(false);status["ready"]=json!(false);status["running"]=json!(false);status["state"]=json!("disabled");status["retry_available"]=json!(false);status["voice_ready"]=json!(false);
                status["voice"]["active"]=json!(false);status["voice"]["ready"]=json!(false);
                status["message"]=json!("AI is turned off. Inventory management remains available.");
                cache_status(&state,status.clone());
                return Ok(status);
            }
            let config = resolve_config(&serde_json::to_value(state.preferences.lock().map_err(lock_error)?.clone()).map_err(|e| e.to_string())?, &bundled_directory(&app.path().resource_dir().map_err(|e|e.to_string())?, std::env::current_exe().map_err(|e|e.to_string())?.parent().ok_or("Application directory unavailable")?, "ai"));
            let assistant = starting.as_mut().ok_or("AI startup state unavailable")?;
            if !assistant.status()["configured"].as_bool().unwrap_or(false) {
                assistant.set_configuration(config["server_path"].as_str().unwrap().into(),config["model_path"].as_str().unwrap().into())?;
            }
            assistant.request_start();
            let status = assistant.status();cache_status(&state,status.clone());Ok(status)
        },
        "assistant_status" => cached_status(&state),
        "voice_status" => Ok(cached_status(&state)?["voice"].clone()),
        "assistant_new_conversation" => {
            let session = session_id(&payload)?;
            state.assistant.lock().map_err(lock_error)?.clear_context(&session);
            Ok(json!({"ok":true}))
        },
        "assistant_ask" => {
            #[cfg(target_os="macos")]
            if !state.runtime_shutdown.ai_enabled() { return Err("AI is turned off. Enable it in Settings or choose Retry AI.".into()); }
            let question = field(&payload, "question")?;
            let session = session_id(&payload)?;
            if question.chars().count() > 1000 { return Err("Please ask a shorter question (up to 1,000 characters).".into()); }
            let mut assistant = state.assistant.lock().map_err(lock_error)?;
            // Only the approved Store snapshot crosses into the assistant. The
            // model has no database connection, write tool, SQL or OS interface.
            let snapshot = state.store.lock().map_err(lock_error)?.execute("snapshot", json!({}))?;
            let result = assistant.ask(&question, &session, &snapshot);
            #[cfg(target_os="macos")]
            remember_ai_pause(&state, &assistant.status());
            cache_status(&state, assistant.status());
            result
        },
        "voice_start" => {
            #[cfg(target_os="macos")]
            if !state.runtime_shutdown.ai_enabled() { return Err("AI and voice are turned off. Enable AI in Settings.".into()); }
            let mut assistant = state.assistant.lock().map_err(lock_error)?;
            let result = assistant.start_voice();
            cache_status(&state, assistant.status());
            Ok(result)
        },
        "voice_stop" => {
            // End Conversation must cancel a blocked microphone/TTS operation
            // without waiting for either the inference or voice request lock.
            state.voice_shutdown.stop();
            if let Ok(mut cached) = state.assistant_status.lock() {
                cached["voice"]["active"] = json!(false);
                cached["voice"]["ready"] = json!(false);
            }
            Ok(json!({"ok":true}))
        },
        "voice_listen" | "voice_speak" => {
            #[cfg(target_os="macos")]
            if !state.runtime_shutdown.ai_enabled() { return Err("AI and voice are turned off.".into()); }
            let language = payload["language"].as_str().unwrap_or("auto");
            let mut assistant = state.assistant.lock().map_err(lock_error)?;
            let result = if action == "voice_listen" { assistant.listen_voice(language) } else { assistant.speak_voice(&field(&payload, "text")?, language,payload["input_language"].as_str().unwrap_or("auto")) };
            cache_status(&state, assistant.status());
            result
        },
        "save_pdf" | "save_excel" => {
            let excel = action == "save_excel";
            let (bytes, name) = validate_export(&payload, excel)?;
            let extension = if excel { "xlsx" } else { "pdf" };
            let label = if excel { "Excel workbook" } else { "PDF document" };
            let selection = app.dialog().file().add_filter(label, &[extension]).set_file_name(name).blocking_save_file();
            if let Some(file) = selection {
                let path = file.into_path().map_err(|e| e.to_string())?;
                if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)) { return Err(format!("Choose a file with the .{extension} extension.")); }
                fs::write(&path, bytes).map_err(|e| format!("Cannot save report: {e}"))?;
                Ok(json!({"path":path.to_string_lossy()}))
            } else { Ok(json!({"cancelled":true})) }
        },
        "backup" | "restore" => {
            let dialog = app.dialog().file().add_filter("Inventory database", &["db"]);
            let selection = if action == "backup" { dialog.set_file_name(format!("inventory-backup-{}.db", chrono::Local::now().format("%Y-%m-%d-%H%M%S"))).blocking_save_file() } else { dialog.blocking_pick_file() };
            if let Some(file) = selection {
                let path = file.into_path().map_err(|e| e.to_string())?;
                if action == "restore" { state.voice_shutdown.stop(); }
                // Serialize restore against chat, so a completed restore can
                // never leave a pending answer based on the former database.
                let mut assistant = if action == "restore" { Some(state.assistant.lock().map_err(lock_error)?) } else { None };
                let result = {
                    let mut store = state.store.lock().map_err(lock_error)?;
                    if action == "backup" { store.backup(&path) } else { store.restore(&path) }?
                };
                if let Some(assistant) = assistant.as_mut() { assistant.clear_all_contexts(); }
                Ok(result)
            } else { Ok(json!({"cancelled":true})) }
        },
        _ => state.store.lock().map_err(lock_error)?.execute(action, payload),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| { if let Some(window) = app.get_webview_window("main") { let _ = window.show(); let _ = window.set_focus(); } }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?.join("data");
            fs::create_dir_all(&data_dir)?;
            let resource_dir = app.path().resource_dir()?;
            let executable = std::env::current_exe()?;
            let executable_dir = executable.parent().ok_or_else(|| std::io::Error::other("Cannot resolve the application directory."))?;
            let bundled = bundled_directory(&resource_dir, executable_dir, "ai");
            let voice = bundled_directory(&resource_dir, executable_dir, "voice");
            let saved: Value = fs::read(data_dir.join("settings.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
            #[cfg(target_os="macos")]
            let preferences = desktop_settings::Preferences::load(&data_dir);
            let config = resolve_config(&saved, &bundled);
            let mut assistant = Assistant::new().map_err(std::io::Error::other)?;
            assistant.set_performance(config["context_size"].as_u64().unwrap_or(4096) as u32, config["cpu_threads"].as_u64().unwrap_or(4) as u32).map_err(std::io::Error::other)?;
            let configured = assistant.set_configuration(config["server_path"].as_str().unwrap().into(), config["model_path"].as_str().unwrap().into()).is_ok();
            #[cfg(target_os="macos")]
            if !preferences.ai_enabled { assistant.restore_paused(&preferences.ai_pause_reason); }
            else if configured { assistant.request_start(); }
            #[cfg(not(target_os="macos"))]
            if configured { assistant.request_start(); }
            if let Err(error)=assistant.configure_bundled_voice(voice.join("runtime").join(runtime_name("voice-worker")).to_string_lossy().into_owned(), voice.to_string_lossy().into_owned()) { eprintln!("Bundled voice initialization: {error}; resource directory: {}",voice.display()); }
            let status = assistant.status();
            let runtime_shutdown = assistant.shutdown_handle();
            let voice_shutdown = assistant.voice_shutdown_handle();
            let store = Store::open(&data_dir.join("inventory.db")).map_err(std::io::Error::other)?;
            #[cfg(not(target_os="macos"))]
            if let Err(error) = write_config(&data_dir, &config) { eprintln!("{error}"); }
            #[cfg(target_os="macos")]
            if let Err(error) = preferences.save(&data_dir) { eprintln!("{error}"); }
            app.manage(AppState {
                #[cfg(target_os="macos")]
                preferences: Mutex::new(preferences),
                #[cfg(target_os="macos")]
                data_dir: data_dir.clone(),
                store: Mutex::new(store), assistant: Mutex::new(assistant), assistant_status: Mutex::new(status), runtime_shutdown, voice_shutdown, manager_shutdown: AtomicBool::new(false) });
            start_assistant_manager(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![api])
        .build(tauri::generate_context!())
        .expect("Could not start Inventory Manager. Check the local data directory permissions.")
        .run(|app, event| { if let tauri::RunEvent::Exit = event { if let Some(state) = app.try_state::<AppState>() { state.manager_shutdown.store(true, Ordering::Release); state.runtime_shutdown.shutdown(); } } });
}

#[cfg(test)]
mod integration_config_tests {
    use super::*;
    #[test]
    fn portable_resources_ignore_obsolete_custom_development_paths() {
        let root = Path::new("C:/Installed/Inventory/resources/ai");
        let config = resolve_config(&json!({"model_source":"custom","server_path":"D:/OldDev/llama-server.exe","python_path":"D:/OldDev/python.exe"}), root);
        assert_eq!(config["model_source"], "bundled");
        assert!(config["server_path"].as_str().unwrap().contains("Installed"));
        let temp = tempfile::tempdir().unwrap();
        write_config(temp.path(), &config).unwrap();
        let saved: Value = serde_json::from_slice(&fs::read(temp.path().join("settings.json")).unwrap()).unwrap();
        assert!(saved.get("server_path").is_none());
        assert!(saved.get("model_path").is_none());
        assert!(saved.get("python_path").is_none());
    }
    #[test]
    fn profiles_and_manifest_remain_bounded_and_installation_relative() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("runtime.json"), r#"{"model_file":"../../outside.gguf","cpu_threads":999,"context_size":99999}"#).unwrap();
        let config = resolve_config(&json!({"performance_profile":"low_memory"}), temp.path());
        assert_eq!(config["context_size"], 2048);
        assert!(config["cpu_threads"].as_u64().unwrap() <= 8);
        assert_eq!(Path::new(config["model_path"].as_str().unwrap()).parent(), Some(temp.path()));
        assert_eq!(safe_resource_filename(Some("/absolute.gguf"), "default.gguf"), "default.gguf");
    }
    #[test]
    fn ipc_validates_conversation_and_report_formats() {
        assert!(session_id(&json!({"session_id":"session-123"})).is_ok());
        assert!(session_id(&json!({"session_id":"../other"})).is_err());
        assert!(session_id(&json!({"session_id":"x".repeat(97)})).is_err());
        assert!(validate_export(&json!({"bytes":[1,2,3]}), false).is_err());
        assert!(validate_export(&json!({"bytes":b"%PDF-fixture".to_vec(),"suggested_name":"../../bad.pdf"}), false).unwrap().1 == "inventory-report.pdf");
        assert!(validate_export(&json!({"bytes":b"PK\x03\x04fixture".to_vec(),"suggested_name":"sales.xlsx"}), true).is_ok());
    }
}
