use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

/// Modèle proposé à l'onboarding.
#[derive(Debug, Clone, Copy)]
pub struct ModelOption {
    pub name: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
    pub description: &'static str,
}

pub const MODEL_OPTIONS: &[ModelOption] = &[
    ModelOption {
        name: "tiny",
        file: "ggml-tiny.bin",
        size_mb: 75,
        description: "Très rapide, qualité faible. Tests / matériel modeste.",
    },
    ModelOption {
        name: "base",
        file: "ggml-base.bin",
        size_mb: 145,
        description: "Rapide, qualité moyenne.",
    },
    ModelOption {
        name: "small",
        file: "ggml-small.bin",
        size_mb: 465,
        description: "Bon compromis pour le français. Recommandé sur CPU.",
    },
    ModelOption {
        name: "medium",
        file: "ggml-medium.bin",
        size_mb: 1500,
        description: "Très bonne qualité FR. Lent en CPU, rapide sur GPU.",
    },
    ModelOption {
        name: "large-v3",
        file: "ggml-large-v3.bin",
        size_mb: 3094,
        description: "Précision maximale. Idéal sur GPU (CUDA/Vulkan).",
    },
];

const HF_BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

#[derive(Debug, Default, Clone)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub finished: bool,
    pub cancelled: bool,
    pub error: Option<String>,
    /// Chemin final si finished == true et error == None
    pub final_path: Option<PathBuf>,
}

pub type SharedProgress = Arc<Mutex<DownloadProgress>>;

pub fn new_progress() -> SharedProgress {
    Arc::new(Mutex::new(DownloadProgress::default()))
}

/// Lance un thread de téléchargement. Retourne la handle pour attendre, et le
/// caller utilise le SharedProgress pour suivre l'avancement.
pub fn spawn_download(
    file_name: String,
    dest_dir: PathBuf,
    progress: SharedProgress,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name(format!("dl-{}", file_name))
        .spawn(move || {
            let url = format!("{}{}", HF_BASE, file_name);
            let dest = dest_dir.join(&file_name);
            // .part pour ne pas laisser un fichier corrompu si la DL échoue.
            let part = dest_dir.join(format!("{}.part", file_name));
            let result = (|| -> Result<PathBuf> {
                std::fs::create_dir_all(&dest_dir)
                    .with_context(|| format!("création {}", dest_dir.display()))?;

                let resp = ureq::get(&url)
                    .timeout(std::time::Duration::from_secs(60))
                    .call()
                    .with_context(|| format!("HTTP GET {}", url))?;

                let total = resp
                    .header("Content-Length")
                    .and_then(|s| s.parse::<u64>().ok());
                {
                    let mut p = progress.lock();
                    p.total = total;
                }

                let mut reader = resp.into_reader();
                let mut file = std::fs::File::create(&part)
                    .with_context(|| format!("création {}", part.display()))?;
                let mut buf = vec![0u8; 256 * 1024];
                let mut downloaded: u64 = 0;
                loop {
                    if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        return Err(anyhow!("Téléchargement annulé"));
                    }
                    let n = reader.read(&mut buf).context("lecture HTTP")?;
                    if n == 0 {
                        break;
                    }
                    file.write_all(&buf[..n]).context("écriture fichier")?;
                    downloaded += n as u64;
                    let mut p = progress.lock();
                    p.downloaded = downloaded;
                }
                file.flush().ok();
                drop(file);
                std::fs::rename(&part, &dest).with_context(|| {
                    format!("renommage {} -> {}", part.display(), dest.display())
                })?;
                Ok(dest)
            })();

            let mut p = progress.lock();
            p.finished = true;
            match result {
                Ok(path) => {
                    p.final_path = Some(path);
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&part);
                    if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        p.cancelled = true;
                    }
                    p.error = Some(format!("{:#}", e));
                }
            }
        })
        .expect("spawn download thread")
}

pub fn human_bytes(b: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if b >= GB {
        format!("{:.2} Go", b as f64 / GB as f64)
    } else if b >= MB {
        format!("{:.1} Mo", b as f64 / MB as f64)
    } else if b >= KB {
        format!("{:.0} Ko", b as f64 / KB as f64)
    } else {
        format!("{} o", b)
    }
}

/// Cherche un fichier `name` dans `dirs`, renvoie le 1er trouvé.
pub fn find_in_dirs(name: &str, dirs: &[&Path]) -> Option<PathBuf> {
    for d in dirs {
        let p = d.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}
