use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputMode {
    /// Tape le texte dans la fenêtre active.
    Type,
    /// Copie le texte dans le presse-papiers.
    Clipboard,
}

impl Default for OutputMode {
    fn default() -> Self {
        Self::Type
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub model_path: PathBuf,
    pub language: String,
    pub hotkey: String,
    pub output_mode: OutputMode,
    /// `None` = micro par défaut du système.
    #[serde(default)]
    pub microphone: Option<String>,
    /// Si `true` et que le binaire est compilé avec un backend GPU, l'inférence
    /// se fait sur GPU. Sinon, CPU.
    #[serde(default = "default_true")]
    pub use_gpu: bool,
    /// Quand on ferme la fenêtre, l'app se cache dans le system tray au lieu
    /// de quitter. Le raccourci global continue de fonctionner.
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    /// Affiche des notifications Windows (toasts) lors de start/stop dictée.
    #[serde(default = "default_true")]
    pub notifications: bool,
    /// Affiche un mini-overlay flottant pendant l'enregistrement avec le live.
    #[serde(default = "default_true")]
    pub live_overlay: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("models/ggml-small.bin"),
            language: "fr".to_string(),
            hotkey: "Control+Alt+Space".to_string(),
            output_mode: OutputMode::Type,
            microphone: None,
            use_gpu: true,
            close_to_tray: true,
            notifications: true,
            live_overlay: true,
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| anyhow!("Impossible de déterminer le dossier de configuration"))?
            .join("NyxWhisper");
        std::fs::create_dir_all(&dir).ok();
        Ok(dir.join("config.toml"))
    }

    /// Dossier des modèles par défaut côté utilisateur (`%LOCALAPPDATA%\NyxWhisper\models\`
    /// sur Windows, équivalent ailleurs). Toujours en écriture sans privilèges admin.
    pub fn user_models_dir() -> PathBuf {
        if let Some(local) = dirs::data_local_dir() {
            return local.join("NyxWhisper").join("models");
        }
        PathBuf::from("models")
    }

    pub fn load() -> Self {
        match Self::try_load() {
            Ok(cfg) => cfg,
            Err(e) => {
                log::warn!(
                    "Lecture config impossible : {}. Utilisation des valeurs par défaut.",
                    e
                );
                Self::default()
            }
        }
    }

    fn try_load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Lecture {}", path.display()))?;
        let cfg: Self = toml::from_str(&text).context("Parsing config TOML")?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let text = toml::to_string_pretty(self).context("Sérialisation config TOML")?;
        std::fs::write(&path, text).with_context(|| format!("Écriture {}", path.display()))?;
        Ok(())
    }
}

/// Liste les fichiers `.bin` présents dans le dossier `models/`.
pub fn list_models(models_dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(models_dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("bin"))
                .unwrap_or(false)
        })
        .collect();
    out.sort();
    out
}
