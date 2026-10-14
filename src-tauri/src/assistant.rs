//! Offline planner with a supervised application-owned local inference process.
#[path = "assistant_conversation.rs"]
mod conversation;
#[path = "assistant_search.rs"]
mod search;
#[path = "assistant_voice.rs"]
mod voice;
use conversation::{Conversation, Plan};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
pub use voice::VoiceShutdown;
use voice::VoiceWorker;

/// Independent of the Assistant mutex, so shutdown also works during inference.
#[derive(Clone)]
pub struct RuntimeShutdown {
    child: Arc<Mutex<Option<Child>>>,
    closing: Arc<AtomicBool>,
    voice: VoiceShutdown,
}
impl RuntimeShutdown {
    pub fn shutdown(&self) {
        self.closing.store(true, Ordering::Release);
        terminate_owned(&self.child);
        self.voice.stop();
    }
}

pub struct Assistant {
    client: Client,
    child: Arc<Mutex<Option<Child>>>,
    closing: Arc<AtomicBool>,
    server_path: Option<PathBuf>,
    model_path: Option<PathBuf>,
    endpoint: String,
    api_key: String,
    diagnostics: Arc<Mutex<String>>,
    contexts: HashMap<String, Conversation>,
    voice: VoiceWorker,
    context_size: u32,
    cpu_threads: u32,
    desired: bool,
    state: &'static str,
    message: String,
    ready: bool,
    running: bool,
    failures: u32,
    retry_at: Instant,
    next_probe: Instant,
    started_at: Option<Instant>,
    healthy_since: Option<Instant>,
    unhealthy_probes: u32,
}
impl Assistant {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(90))
                .build()
                .map_err(|e| e.to_string())?,
            child: Arc::new(Mutex::new(None)),
            closing: Arc::new(AtomicBool::new(false)),
            server_path: None,
            model_path: None,
            endpoint: String::new(),
            api_key: String::new(),
            diagnostics: Arc::new(Mutex::new(String::new())),
            contexts: HashMap::new(),
            voice: VoiceWorker::new(),
            context_size: 4096,
            cpu_threads: 4,
            desired: false,
            state: "unconfigured",
            ready: false,
            running: false,
            failures: 0,
            retry_at: Instant::now(),
            next_probe: Instant::now(),
            started_at: None,
            healthy_since: None,
            unhealthy_probes: 0,
            message: "The local assistant is being prepared.".into(),
        })
    }
    pub fn set_performance(&mut self, context_size: u32, cpu_threads: u32) -> Result<(), String> {
        if !(2048..=8192).contains(&context_size) || !(1..=8).contains(&cpu_threads) {
            return Err(
                "Local AI profile must use 2048–8192 context tokens and 1–8 CPU threads.".into(),
            );
        }
        if self.context_size != context_size || self.cpu_threads != cpu_threads {
            self.context_size = context_size;
            self.cpu_threads = cpu_threads;
            if self.running {
                terminate_owned(&self.child);
                self.running = false;
                self.ready = false;
                self.retry_at = Instant::now();
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn owned_runtime_id(&self) -> Option<u32> {
        self.child
            .lock()
            .ok()
            .and_then(|owner| owner.as_ref().map(Child::id))
    }
    #[cfg(test)]
    pub fn simulate_owned_runtime_crash(&mut self) {
        if let Ok(mut owner) = self.child.lock() {
            if let Some(child) = owner.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    pub fn shutdown_handle(&self) -> RuntimeShutdown {
        RuntimeShutdown {
            child: self.child.clone(),
            closing: self.closing.clone(),
            voice: self.voice.shutdown_handle(),
        }
    }
    pub fn set_configuration(
        &mut self,
        server_path: String,
        model_path: String,
    ) -> Result<Value, String> {
        let validated = (|| {
            let server = local_path(&server_path, false)?;
            let model = local_path(&model_path, false)?;
            if cfg!(windows)
                && !server
                    .file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("llama-server.exe"))
            {
                return Err(
                    "Choose the local llama-server.exe supplied with the application.".to_string(),
                );
            }
            let mut magic = [0; 4];
            std::fs::File::open(&model)
                .and_then(|mut f| f.read_exact(&mut magic))
                .map_err(|e| format!("Cannot read local model: {e}"))?;
            if &magic != b"GGUF" {
                return Err("The selected local model is not a GGUF file.".into());
            }
            Ok((server, model))
        })();
        match validated {
            Ok((server, model)) => {
                terminate_owned(&self.child);
                self.server_path = Some(server);
                self.model_path = Some(model);
                self.ready = false;
                self.running = false;
                self.state = "stopped";
                self.failures = 0;
                self.retry_at = Instant::now();
                self.started_at = None;
                self.healthy_since = None;
                self.message = "The local assistant will start automatically.".into();
                Ok(self.status())
            }
            Err(error) => {
                self.state = "failed";
                self.message = error.clone();
                self.ready = false;
                Err(error)
            }
        }
    }
    pub fn request_start(&mut self) {
        if self.closing.load(Ordering::Acquire) {
            return;
        }
        self.desired = true;
        if !self.running && self.server_path.is_some() {
            self.state = "starting";
            self.retry_at = Instant::now();
            self.message = "Starting the local assistant…".into();
        }
    }
    pub fn configure(&mut self, server_path: String, model_path: String) -> Result<Value, String> {
        self.set_configuration(server_path, model_path)?;
        self.request_start();
        Ok(self.tick())
    }
    /// Cached status: no network or process waits.
    pub fn status(&self) -> Value {
        let v = self.voice.status();
        json!({"configured":self.server_path.is_some(),"running":self.running,"ready":self.ready,"state":self.state,"message":self.message,"attempts":self.failures,"retry_in_seconds":self.retry_at.saturating_duration_since(Instant::now()).as_secs(),"voice":v,"voice_configured":v["configured"],"voice_ready":v["ready"],"voice_message":v["message"]})
    }
    /// Background manager tick. Health calls take at most 200ms, once per second.
    pub fn tick(&mut self) -> Value {
        self.voice.tick();
        if self.closing.load(Ordering::Acquire) {
            self.ready = false;
            self.running = false;
            self.state = "stopped";
            return self.status();
        }
        if !self.desired {
            return self.status();
        }
        if self.server_path.is_none() {
            self.state = "unconfigured";
            return self.status();
        }
        let now = Instant::now();
        let exited = match self.child.lock() {
            Ok(mut owner) => match owner.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(Some(code)) => Some(format!("The local model stopped ({code}).")),
                    Err(e) => Some(format!("Could not inspect local model: {e}")),
                    Ok(None) => None,
                },
                None => None,
            },
            Err(_) => Some("The local model process state is unavailable.".into()),
        };
        if let Some(error) = exited {
            self.schedule_recovery(error);
            return self.status();
        }
        if !self.running {
            if now < self.retry_at {
                return self.status();
            }
            if let Err(error) = self.spawn_runtime() {
                self.schedule_recovery(error);
            }
            return self.status();
        }
        if now < self.next_probe {
            return self.status();
        }
        self.next_probe = now + Duration::from_secs(1);
        let healthy = self
            .client
            .get(format!("{}/health", self.endpoint))
            .bearer_auth(&self.api_key)
            .timeout(Duration::from_millis(200))
            .send()
            .is_ok_and(|r| r.status().is_success());
        if healthy {
            self.ready = true;
            self.state = "ready";
            self.message = "Ready — all processing stays on this computer.".into();
            self.unhealthy_probes = 0;
            let since = self.healthy_since.get_or_insert(now);
            if since.elapsed() > Duration::from_secs(30) {
                self.failures = 0;
            }
        } else {
            self.unhealthy_probes += 1;
            if self.ready && self.unhealthy_probes < 5 {
                return self.status();
            }
            self.ready = false;
            if self
                .started_at
                .is_some_and(|s| s.elapsed() > Duration::from_secs(120))
                || self.healthy_since.is_some()
            {
                self.schedule_recovery("The local model is not responding.".into());
            }
        }
        self.status()
    }
    fn spawn_runtime(&mut self) -> Result<(), String> {
        let server = self
            .server_path
            .as_ref()
            .ok_or("Local runtime is not configured")?;
        let model = self
            .model_path
            .as_ref()
            .ok_or("Local model is not configured")?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        self.api_key = uuid::Uuid::new_v4().to_string();
        self.endpoint = format!("http://127.0.0.1:{port}");
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().min(self.cpu_threads as usize))
            .unwrap_or(2)
            .to_string();
        let context_size = self.context_size.to_string();
        let mut command = Command::new(server);
        command
            .current_dir(server.parent().ok_or("Invalid runtime directory")?)
            .arg("--model")
            .arg(model)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--api-key",
                &self.api_key,
                "--ctx-size",
                &context_size,
                "--parallel",
                "1",
                "--threads",
                &threads,
                "--jinja",
                "--reasoning-budget",
                "256",
                "--offline",
                "--no-webui",
                "--no-slots",
                "--no-mmproj",
                "--n-gpu-layers",
                "0",
                "--alias",
                "inventory-intents",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy().to_uppercase();
            if name.starts_with("LLAMA_") || name.starts_with("HF_") || name.contains("PROXY") {
                command.env_remove(key);
            }
        }
        command
            .env("HF_HUB_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1");
        hidden(&mut command);
        drop(listener);
        self.diagnostics = Arc::new(Mutex::new(String::new()));
        let mut child = command
            .spawn()
            .map_err(|e| format!("Could not start local model: {e}"))?;
        if let Some(stderr) = child.stderr.take() {
            drain(stderr, self.diagnostics.clone(), 4096);
        }
        *self
            .child
            .lock()
            .map_err(|_| "Local runtime state unavailable")? = Some(child);
        self.running = true;
        self.ready = false;
        self.state = "starting";
        self.message = "Loading the local model…".into();
        self.started_at = Some(Instant::now());
        self.healthy_since = None;
        self.unhealthy_probes = 0;
        self.next_probe = Instant::now();
        Ok(())
    }
    fn schedule_recovery(&mut self, error: String) {
        terminate_owned(&self.child);
        self.running = false;
        self.ready = false;
        self.failures = self.failures.saturating_add(1);
        let seconds = (1u64 << self.failures.min(5)).min(30);
        self.retry_at = Instant::now() + Duration::from_secs(seconds);
        self.state = "recovering";
        self.message = format!("{error} Retrying automatically in {seconds} seconds.");
        self.started_at = None;
        self.healthy_since = None;
    }
    pub fn stop(&mut self) {
        self.desired = false;
        terminate_owned(&self.child);
        self.voice.stop();
        self.ready = false;
        self.running = false;
        self.state = "stopped";
        self.message = "The local assistant is stopped.".into();
    }
    pub fn clear_context(&mut self, key: &str) {
        self.contexts.remove(key);
    }
    pub fn clear_all_contexts(&mut self) {
        self.contexts.clear();
    }
    pub fn configure_bundled_voice(
        &mut self,
        executable: String,
        resources: String,
    ) -> Result<Value, String> {
        self.voice.configure_bundled(executable, resources)
    }
    pub fn start_voice(&mut self) -> Value {
        self.voice.start()
    }
    pub fn listen_voice(&mut self, language: &str) -> Result<Value, String> {
        self.voice.listen(language)
    }
    pub fn speak_voice(&mut self, text: &str, language: &str,input_language:&str) -> Result<Value, String> {
        self.voice.speak_with_input(text, language,input_language)
    }
    pub fn voice_shutdown_handle(&self) -> VoiceShutdown {
        self.voice.shutdown_handle()
    }
    /// Caller supplies a fresh approved read-only business snapshot. No database write API exists here.
    fn infer(&mut self,messages:Value,schema:Value,thinking:bool)->Result<String,String> {
        let payload = json!({"model":"inventory-intents","temperature":0,"max_tokens":if thinking {850}else{256},"stream":false,"chat_template_kwargs":{"enable_thinking":thinking},"response_format":{"type":"json_object","schema":schema},"messages":messages});
        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.endpoint))
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send();
        let response = match response {
            Ok(r) => r,
            Err(_) => {
                self.schedule_recovery("The assistant's connection was interrupted.".into());
                return Err("The local assistant is recovering. Your conversation has been kept; try again shortly.".into());
            }
        };
        if !response.status().is_success() {
            return Err(format!(
                "The assistant could not interpret that question (HTTP {}). Please try again.",
                response.status()
            ));
        }
        let mut body = String::new();
        response
            .take(65_537)
            .read_to_string(&mut body)
            .map_err(|e| e.to_string())?;
        if body.len() > 65_536 {
            return Err("The local model response exceeded its permitted size.".into());
        }
        let response: Value = serde_json::from_str(&body)
            .map_err(|_| "The assistant returned an unreadable response. Please try again.")?;
        let content = response
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or("The assistant did not finish interpreting the question. Please try again.")?;
        Ok(content.to_string())
    }
    pub fn ask(&mut self, question: &str, key: &str, snapshot: &Value) -> Result<Value, String> {
        let question = question.trim();
        if question.is_empty() || question.chars().count() > 1000 {
            return Err("Enter a question between 1 and 1,000 characters.".into());
        }
        if key.is_empty() || key.len() > 96 {
            return Err("A conversation identifier is required.".into());
        }
        self.tick();
        if !self.ready {
            return Err(self.message.clone());
        }
        if self.contexts.len() >= 64 && !self.contexts.contains_key(key) {
            if let Some(oldest) = self
                .contexts
                .iter()
                .min_by_key(|(_, c)| c.last_used)
                .map(|(k, _)| k.clone())
            {
                self.contexts.remove(&oldest);
            }
        }
        let context = self.contexts.entry(key.into()).or_default();
        let messages=conversation::planning_messages(question,context);
        let content=self.infer(messages,conversation::schema(),false)?;
        let mut plan:Plan=conversation::parse_plan(&content)?;
        let scope_messages=conversation::scope_check_messages(&plan,question,self.contexts.get(key).unwrap());
        if let Some(messages)=scope_messages {
            let scope=self.infer(messages,conversation::scope_schema(),false)?;
            conversation::apply_scope(&mut plan,&scope)?;
        }
        let context = self.contexts.entry(key.into()).or_default();
        let mut answer = conversation::respond(&plan, question, snapshot, context)?;
        answer["brief_text"] = json!(conversation::brief_response(&answer));
        #[cfg(test)]
        let answer = {
            let mut value = answer;
            value["interpreted_plan"] = serde_json::to_value(&plan).unwrap();
            value
        };
        Ok(answer)
    }
}
impl Drop for Assistant {
    fn drop(&mut self) {
        self.shutdown_handle().shutdown();
    }
}
fn terminate_owned(owner: &Arc<Mutex<Option<Child>>>) {
    if let Ok(mut owner) = owner.lock() {
        if let Some(mut child) = owner.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}
fn drain(mut reader: impl Read + Send + 'static, output: Arc<Mutex<String>>, cap: usize) {
    std::thread::spawn(move || {
        let mut buffer = [0; 2048];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            if let Ok(mut text) = output.lock() {
                text.push_str(&String::from_utf8_lossy(&buffer[..count]));
                if text.len() > cap {
                    let mut start = text.len() - cap;
                    while !text.is_char_boundary(start) {
                        start += 1;
                    }
                    text.drain(..start);
                }
            }
        }
    });
}
fn local_path(input: &str, directory: bool) -> Result<PathBuf, String> {
    let path = Path::new(input);
    let network = input.starts_with("\\\\") && !input.starts_with("\\\\?\\");
    if !path.is_absolute() || network || input.starts_with("\\\\?\\UNC\\") || input.contains("://")
    {
        return Err("Select a local absolute path; network paths are not supported.".into());
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Local path is unavailable: {e}"))?;
    if canonical.to_string_lossy().starts_with("\\\\?\\UNC\\") {
        return Err("Network paths are not supported.".into());
    }
    if directory && !canonical.is_dir() || !directory && !canonical.is_file() {
        return Err("The selected local file or directory is unavailable.".into());
    }
    Ok(canonical)
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    #[test]
    fn cached_status_does_not_probe() {
        let a = Assistant::new().unwrap();
        let start = Instant::now();
        for _ in 0..1000 {
            assert_eq!(a.status()["running"], false);
        }
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn recovery_preserves_session_and_backs_off() {
        let mut a = Assistant::new().unwrap();
        a.contexts.insert("s".into(), Conversation::default());
        a.schedule_recovery("test".into());
        assert_eq!(a.contexts.len(), 1);
        assert!(!a.ready);
        assert!(a.retry_at > Instant::now());
        assert_eq!(a.status()["state"], "recovering");
    }
    #[test]
    fn network_model_paths_are_rejected() {
        assert!(local_path("https://example.invalid/a.gguf", false).is_err());
        assert!(local_path(r"\\server\a.gguf", false).is_err());
    }
}
