//! Bundled offline speech worker. Voice Mode explicitly starts and stops listening.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct VoiceShutdown(Arc<Mutex<Option<Child>>>, Arc<AtomicBool>, Arc<AtomicBool>);
impl VoiceShutdown {
    pub fn stop(&self) {
        self.1.store(false, Ordering::SeqCst);
        self.2.store(false, Ordering::SeqCst);
        self.stop_child();
    }
    pub fn interrupted(&self)->bool {self.2.load(Ordering::SeqCst)}
    fn stop_child(&self) {
        if let Ok(mut slot) = self.0.lock() {
            if let Some(mut child) = slot.take() {
                if let Some(mut input) = child.stdin.take() {
                    let _ = input.write_all(b"{\"shutdown\":true}\n");
                }
                let deadline = Instant::now() + Duration::from_millis(300);
                while matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(20));
                }
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

pub struct VoiceWorker {
    child: Arc<Mutex<Option<Child>>>,
    receiver: Option<mpsc::Receiver<Value>>,
    paths: Option<(PathBuf, PathBuf)>,
    ready: bool,
    desired: Arc<AtomicBool>,
    interrupted: Arc<AtomicBool>,
    message: String,
    started: Option<Instant>,
    retry_at: Instant,
    failures: u32,
}

impl VoiceWorker {
    pub fn new() -> Self {
        Self { child: Arc::new(Mutex::new(None)), receiver: None, paths: None,
            ready: false, desired: Arc::new(AtomicBool::new(false)), interrupted: Arc::new(AtomicBool::new(false)), message: "Voice resources are unavailable.".into(),
            started: None, retry_at: Instant::now(), failures: 0 }
    }
    pub fn shutdown_handle(&self) -> VoiceShutdown { VoiceShutdown(self.child.clone(), self.desired.clone(), self.interrupted.clone()) }

    pub fn configure_bundled(&mut self, executable:String,resources:String)->Result<Value,String> {
        match self.configure_paths(executable,resources) {Ok(status)=>Ok(status),Err(error)=>{self.message=error.clone();Err(error)}}
    }
    fn configure_paths(&mut self, executable: String, resources: String) -> Result<Value, String> {
        let executable = local_path(&executable, false)?;
        let resources = local_path(&resources, true)?;
        for file in ["stt/model.bin", "stt/config.json", "stt/tokenizer.json", "tts/nabra/model.onnx",
            "tts/nabra/vocab.json", "tts/nabra/voice.bin", "tts/kokoro/model.onnx", "tts/kokoro/vocab.json", "tts/kokoro/voice.bin",
            "tts/catt/catt_eo.int8.onnx", "tts/catt/tokenizer.json"] {
            if !resources.join(file).is_file() { return Err(format!("Bundled voice resource is missing: {file}. Repair the application installation.")); }
        }
        self.stop();
        self.paths = Some((executable, resources));
        self.message = "Voice is available. Select Start Conversation to load it.".into();
        Ok(self.status())
    }

    pub fn start(&mut self) -> Value {
        if self.paths.is_some() {
            if !self.desired.load(Ordering::SeqCst) { self.receiver = None; self.ready = false; self.started = None; }
            self.desired.store(true, Ordering::SeqCst);
            self.failures = 0;
            self.retry_at = Instant::now();
            self.tick();
        }
        self.status()
    }

    pub fn status(&self) -> Value {
        json!({"configured":self.paths.is_some(),"ready":self.ready && self.desired.load(Ordering::SeqCst),"message":self.message,"active":self.desired.load(Ordering::SeqCst)})
    }

    pub fn stop(&mut self) {
        self.desired.store(false, Ordering::SeqCst);
        self.interrupted.store(false,Ordering::SeqCst);
        self.ready = false;
        self.shutdown_handle().stop_child();
        self.receiver = None;
    }

    fn failed(&mut self, message: String) {
        self.shutdown_handle().stop_child();
        self.receiver = None;
        self.ready = false;
        self.started = None;
        self.failures = self.failures.saturating_add(1);
        self.retry_at = Instant::now() + Duration::from_secs(5u64.saturating_mul(1 << self.failures.min(4)));
        self.message = format!("Voice unavailable: {message} Automatic recovery will retry; typing remains available.");
    }

    pub fn tick(&mut self) {
        if !self.desired.load(Ordering::SeqCst) { self.ready = false; return; }
        let running = self.child.lock().ok().and_then(|mut slot| {
            slot.as_mut().map(|child| matches!(child.try_wait(), Ok(None)))
        });
        if running == Some(false) { self.failed("the local speech process exited".into()); }
        if let Some(receiver) = &self.receiver {
            let messages: Vec<_> = receiver.try_iter().collect();
            for message in messages {
                if message["ready"] == true {
                    self.ready = true;
                    self.failures = 0;
                    self.message = "Voice ready. Listening begins while the conversation is active.".into();
                } else if let Some(error) = message["error"].as_str() {
                    self.failed(error.to_owned());
                    return;
                }
            }
        }
        if self.started.is_some_and(|t| !self.ready && t.elapsed() > Duration::from_secs(120)) {
            self.failed("the speech model did not finish loading".into());
        }
        if self.receiver.is_none() && Instant::now() >= self.retry_at {
            if let Err(error) = self.spawn() { self.failed(error); }
        }
    }

    fn spawn(&mut self) -> Result<(), String> {
        let (executable, resources) = self.paths.as_ref().ok_or("Voice resources are unavailable")?;
        let mut command = Command::new(executable);
        command.args(["--worker", "--resources"]).arg(resources);
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy().to_uppercase();
            if name.starts_with("HF_") || name.contains("PROXY") { command.env_remove(key); }
        }
        command.env("HF_HUB_OFFLINE", "1").env("TRANSFORMERS_OFFLINE", "1").env("HF_HUB_DISABLE_TELEMETRY", "1")
            .env("PYTHONUTF8", "1").env("PYTHONIOENCODING", "utf-8");
        #[cfg(windows)] {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|e| format!("Cannot start the local speech environment: {e}"))?;
        let output = child.stdout.take().ok_or("Cannot read speech worker output")?;
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                match Read::by_ref(&mut reader).take(16_385).read_until(b'\n', &mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if line.len() > 16_384 => {
                        let _ = sender.send(json!({"error":"Speech response exceeded the permitted size."}));
                        break;
                    }
                    Ok(_) => match serde_json::from_slice::<Value>(&line) {
                        Ok(value) if value.is_object() => { if sender.send(value).is_err() { break; } }
                        _ => { let _ = sender.send(json!({"error":"Invalid speech worker response."})); break; }
                    }
                }
            }
        });
        *self.child.lock().map_err(|_| "Speech process lock failed")? = Some(child);
        self.receiver = Some(receiver);
        self.started = Some(Instant::now());
        self.ready = false;
        self.message = "Starting voice...".into();
        Ok(())
    }

    pub fn listen(&mut self, language: &str) -> Result<Value, String> {
        if !["auto", "en", "ar"].contains(&language) { return Err("Unsupported speech language.".into()); }
        let response = self.request(json!({"action":"listen","language":language}))?;
        if response["no_speech"] == true { return Ok(json!({"text":"","language":response["language"],"no_speech":true})); }
        validate_transcript(response, "validated")
    }

    pub fn speak(&mut self, text: &str, language: &str) -> Result<Value, String> {
        self.speak_with_input(text,language,"auto")
    }
    pub fn speak_with_input(&mut self,text:&str,language:&str,input_language:&str)->Result<Value,String> {
        if !["auto","en","ar"].contains(&input_language) {return Err("Unsupported input speech language.".into());}
        if !["auto", "en", "ar"].contains(&language) || !(1..=12000).contains(&text.chars().count()) {
            return Err("Invalid speech text or language.".into());
        }
        self.request(json!({"action":"speak","text":text,"language":language,"input_language":input_language}))
    }

    fn request(&mut self, mut payload: Value) -> Result<Value, String> {
        self.tick();
        if !self.ready { return Err(self.message.clone()); }
        let id = uuid::Uuid::new_v4().to_string();
        let speaking = payload["action"] == "speak";
        self.interrupted.store(false,Ordering::SeqCst);
        let deadline = Instant::now() + request_budget(&payload);
        payload["id"] = json!(id);
        let request = format!("{payload}\n");
        let sent = self.child.lock().map_err(|_| "Speech process lock failed")?.as_mut()
            .and_then(|child| child.stdin.as_mut()).ok_or("Voice is restarting")?
            .write_all(request.as_bytes());
        if let Err(error) = sent {
            self.failed(error.to_string());
            return Err(self.message.clone());
        }
        let mut played_samples = 0;
        loop {
        let response = self.receiver.as_ref().ok_or("Voice is restarting")?
            .recv_timeout(deadline.saturating_duration_since(Instant::now()).min(Duration::from_secs(180)));
        match response {
            Ok(mut value) => {
                if value["id"] != id { return Err("The voice response did not match this request.".into()); }
                if let Some(error) = value["error"].as_str() { return Err(error.to_string()); }
                if value["progress"] == true {
                    let count = value["played_samples"].as_u64().ok_or("Invalid speech playback progress.")?;
                    if !speaking || count > 24000 * 7200 || (count < played_samples && !self.interrupted.load(Ordering::SeqCst)) {
                        return Err("Invalid speech playback progress.".into());
                    }
                    if value["interrupted"]==true {self.interrupted.store(true,Ordering::SeqCst);}
                    played_samples = played_samples.max(count);
                    continue;
                }
                if speaking && value["interrupted"]==true {
                    if !self.interrupted.load(Ordering::SeqCst) || value["spoken"]!=false || value["samples"].as_u64()!=Some(played_samples) {
                        return Err("The speech interruption was not acknowledged correctly.".into());
                    }
                    if value["transcript"]["no_speech"]!=true {
                        let mut transcript=value["transcript"].clone();transcript["id"]=json!("validated");
                        value["transcript"]=validate_transcript(transcript,"validated")?;
                    }
                } else if speaking && (value["spoken"] != true || value["samples"].as_u64().unwrap_or(0) != played_samples) {
                    return Err("The speech response did not confirm complete playback.".into());
                }
                value["id"] = json!("validated");
                return Ok(value);
            },
            Err(_) => {
                if !self.desired.load(Ordering::SeqCst) { return Err("Voice conversation ended.".into()); }
                self.failed(if speaking { "speech playback stopped responding" } else { "the recording/transcription timed out" }.into());
                return Err(self.message.clone());
            }
        }
        }
    }
}
impl Drop for VoiceWorker { fn drop(&mut self) { self.stop(); } }

fn request_budget(payload: &Value) -> Duration {
    if payload["action"] == "speak" {
        // Allow a complete long reply, including CPU synthesis and real-time
        // playback. Progress is acknowledged separately from final completion.
        let characters = payload["text"].as_str().map_or(0, |text| text.chars().count().min(12000));
        Duration::from_secs(180 + characters as u64 / 3)
    } else { Duration::from_secs(180) }
}

fn validate_transcript(value: Value, id: &str) -> Result<Value, String> {
    if value["id"] != id { return Err("The speech response did not match this recording.".into()); }
    if let Some(error) = value["error"].as_str() { return Err(error.to_string()); }
    let text = value["text"].as_str().unwrap_or("").trim();
    if text.is_empty() { return Err("No speech was recognized. Try again or type your question.".into()); }
    if text.chars().count() > 1000 { return Err("The transcript is too long. Record a shorter question.".into()); }
    Ok(json!({"text":text,"language":value["language"]}))
}

fn local_path(input: &str, directory: bool) -> Result<PathBuf, String> {
    let path = Path::new(input);
    let network=input.starts_with("\\\\") && !input.starts_with("\\\\?\\");
    if !path.is_absolute() || network || input.to_ascii_uppercase().starts_with("\\\\?\\UNC\\") || input.starts_with("//") || input.contains("://") {
        return Err("Choose a local absolute path; network paths are not supported.".into());
    }
    let path = path.canonicalize().map_err(|e| format!("Local voice path is unavailable: {e}"))?;
    if path.to_string_lossy().to_ascii_uppercase().starts_with("\\\\?\\UNC\\") { return Err("Network voice paths are not supported.".into()); }
    if (directory && !path.is_dir()) || (!directory && !path.is_file()) { return Err("Local voice component is unavailable.".into()); }
    // CTranslate2 cannot open model files through Windows extended path syntax.
    // Canonical validation above rejects network paths before simplifying it.
    #[cfg(windows)]
    if let Some(local)=path.to_string_lossy().strip_prefix(r"\\?\") { return Ok(PathBuf::from(local)); }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Uses actual bundled models and the default speaker; opt-in hardware acceptance"]
    fn packaged_voice_playback_acceptance() {
        let resources = std::env::var("INVENTORY_VOICE_TEST_RESOURCES").map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/voice"));
        let executable = resources.join("runtime").join(if cfg!(windows) { "voice-worker.exe" } else { "voice-worker" });
        let mut worker = VoiceWorker::new();
        worker.configure_bundled(executable.to_string_lossy().into(), resources.to_string_lossy().into()).unwrap();
        worker.start();
        let deadline = Instant::now() + Duration::from_secs(120);
        while worker.status()["ready"] != true && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            worker.tick();
        }
        assert_eq!(worker.status()["ready"], true, "{}", worker.status());
        for (language, text) in [
            ("en", "You can search by the product name or device model. You do not need an item code to search."),
            ("ar", "يمكنك البحث عن شاشة Samsung S3 بالاسم أو بالموديل. رمز الصنف غير مطلوب للبحث."),
            ("en", "To add a product, open Products. Use Stock Entry to record the quantity and purchase cost."),
            ("ar", "لإضافة منتج، افتح Products، ثم أدخل رمز الصنف واسم المنتج والموديل، واضغط Save."),
        ] {
            let started = Instant::now();
            let response = worker.speak(text, language).unwrap();
            assert_eq!(response["spoken"], true);
            assert_eq!(response["characters"].as_u64(), Some(text.chars().count() as u64));
            if language == "ar" { assert!(response["arabic_characters"].as_u64().unwrap() > 30); }
            assert!(response["samples"].as_u64().unwrap() > 24000);
            println!("VOICE_ACCEPTANCE {language} elapsed={:.2}s {response}", started.elapsed().as_secs_f64());
        }
        if std::env::var("INVENTORY_LONG_VOICE_TEST").as_deref() == Ok("1") {
            let text = "The assistant should finish speaking before listening again. Open Products to add a product, then use Stock Entry to record its quantity and purchase cost. ".repeat(18)
                + "This is the final sentence, and the playback test is now complete.";
            let started = Instant::now();
            let response = worker.speak(&text, "en").unwrap();
            assert!(response["samples"].as_u64().unwrap() > 24000 * 150);
            assert!(started.elapsed() > Duration::from_secs(150));
            assert_eq!(response["characters"].as_u64(), Some(text.chars().count() as u64));
            println!("VOICE_LONG_ACCEPTANCE elapsed={:.2}s {response}", started.elapsed().as_secs_f64());
        }
        worker.stop();
        assert_eq!(worker.status()["active"], false);
        assert!(worker.child.lock().unwrap().is_none());
    }
    #[test]
    fn long_speech_has_time_for_complete_playback() {
        let payload = json!({"action":"speak", "text":"word ".repeat(1000)});
        assert!(request_budget(&payload) > Duration::from_secs(150));
        assert_eq!(request_budget(&json!({"action":"listen"})), Duration::from_secs(180));
        assert_eq!(request_budget(&json!({"action":"speak","text":"x".repeat(12001)})), Duration::from_secs(4180));
    }
    #[test]
    fn voice_requires_explicit_recording_and_validates_transcripts() {
        let mut worker = VoiceWorker::new();
        worker.tick();
        assert_eq!(worker.status()["configured"], false);
        assert!(worker.listen("auto").is_err());
        assert!(worker.listen("unsupported").is_err());
        assert!(validate_transcript(json!({"id":"a","text":"hello"}), "b").is_err());
        assert!(validate_transcript(json!({"id":"a","text":" "}), "a").is_err());
        assert!(validate_transcript(json!({"id":"a","text":"x".repeat(1001)}), "a").is_err());
        assert_eq!(validate_transcript(json!({"id":"a","text":" أين القطعة؟ ","language":"ar"}), "a").unwrap()["text"], "أين القطعة؟");
    }
    #[test]
    fn voice_accepts_canonical_local_resource_paths() {
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().canonicalize().unwrap();
        assert_eq!(local_path(&path.to_string_lossy(),true).unwrap().canonicalize().unwrap(),path);
        assert!(local_path(r"\\?\UNC\server\voice",true).is_err());
    }
    #[test]
    fn voice_rejects_network_paths_and_missing_resources() {
        assert!(local_path("https://example.com/model", true).is_err());
        assert!(local_path(r"\\server\voice", true).is_err());
        assert!(local_path("relative/model", true).is_err());
        let mut worker = VoiceWorker::new();
        assert!(worker.configure_bundled("missing.exe".into(), "missing-model".into()).is_err());
        assert_eq!(worker.status()["configured"], false);
    }
}
