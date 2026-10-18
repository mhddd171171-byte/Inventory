//! Small, local preferences. Business data and model paths never enter this file.
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub ai_enabled: bool,
    pub additional_backup_directory: Option<String>,
    pub ai_pause_reason: String,
    pub theme: String,
    pub performance_profile: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self { additional_backup_directory: None, ai_enabled: true, ai_pause_reason: String::new(), theme: "system".into(), performance_profile: "balanced".into() }
    }
}
impl Preferences {
    pub fn load(directory: &Path) -> Self {
        let mut preferences: Self = std::fs::read(directory.join("settings.json")).ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        if !["system", "light", "dark"].contains(&preferences.theme.as_str()) { preferences.theme = "system".into(); }
        preferences
    }
    pub fn save(&self, directory: &Path) -> Result<(), String> {
        let destination = directory.join("settings.json");
        let temporary = directory.join("settings.json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&temporary, bytes).map_err(|e| format!("Cannot save preferences: {e}"))?;
        // macOS rename atomically replaces this application's preferences file.
        std::fs::rename(&temporary, &destination).map_err(|e| format!("Cannot save preferences: {e}"))
    }
    pub fn set_theme(&mut self, theme: &str) -> Result<(), String> {
        if !["system", "light", "dark"].contains(&theme) { return Err("Choose System, Light or Dark appearance.".into()); }
        self.theme = theme.into(); Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn off_state_and_theme_survive_restart_without_changing_inventory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("inventory.db"), b"business data").unwrap();
        let mut p = Preferences::default(); p.ai_enabled = false; p.ai_pause_reason = "timed_out".into(); p.set_theme("dark").unwrap();
        p.save(dir.path()).unwrap();
        let loaded = Preferences::load(dir.path());
        assert!(!loaded.ai_enabled); assert_eq!(loaded.ai_pause_reason, "timed_out"); assert_eq!(loaded.theme, "dark");
        assert_eq!(std::fs::read(dir.path().join("inventory.db")).unwrap(), b"business data");
        assert!(p.set_theme("arbitrary").is_err());
    }
    #[test]
    fn existing_profile_survives_upgrade() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), br#"{"performance_profile":"low_memory","model_source":"bundled"}"#).unwrap();
        let p = Preferences::load(dir.path()); assert!(p.ai_enabled); assert_eq!(p.theme,"system"); assert_eq!(p.performance_profile,"low_memory");
    }
}
