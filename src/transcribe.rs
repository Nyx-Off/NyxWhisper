use anyhow::{anyhow, Result};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperTranscriber {
    context: WhisperContext,
    language: String,
}

/// Le backend GPU compilé dans ce binaire (info pour l'UI).
pub const COMPILED_GPU_BACKEND: &str = if cfg!(feature = "cuda") {
    "CUDA"
} else if cfg!(feature = "vulkan") {
    "Vulkan"
} else {
    "CPU only"
};

/// `true` si ce binaire a été compilé avec un backend GPU activé.
pub const HAS_GPU_BACKEND: bool = cfg!(any(feature = "cuda", feature = "vulkan"));

impl WhisperTranscriber {
    pub fn new(model_path: &Path, language: &str) -> Result<Self> {
        Self::new_with_options(model_path, language, true)
    }

    /// `use_gpu` n'a d'effet que si le binaire a été compilé avec une feature
    /// GPU (cuda/vulkan). Sinon, c'est CPU dans tous les cas.
    pub fn new_with_options(model_path: &Path, language: &str, use_gpu: bool) -> Result<Self> {
        let mut params = WhisperContextParameters::default();
        params.use_gpu = use_gpu && HAS_GPU_BACKEND;
        let context = WhisperContext::new_with_params(model_path, params)
            .map_err(|e| anyhow!("Échec du chargement du modèle Whisper : {}", e))?;
        Ok(Self {
            context,
            language: language.to_string(),
        })
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        // Ignore les enregistrements trop courts (< 0,3 s à 16 kHz).
        if samples.len() < 4_800 {
            return Ok(String::new());
        }

        let mut state = self
            .context
            .create_state()
            .map_err(|e| anyhow!("create_state : {}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.language));

        // Tous les coeurs disponibles (jusqu'à 12) pour de meilleures perfs CPU.
        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4)
            .min(12);
        params.set_n_threads(n_threads);

        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_no_context(true);
        params.set_single_segment(false);

        // Initial prompt : oriente le modèle vers du FR propre avec ponctuation
        // et accents corrects (majuscules, virgules, apostrophes, mots techniques).
        if self.language == "fr" {
            params.set_initial_prompt(
                "Voici une dictée en français, avec ponctuation et accents corrects.",
            );
        }

        // Anti-hallucination : Whisper a tendance à halluciner des "Sous-titrage..."
        // ou "Merci d'avoir regardé..." sur les silences. On limite ça :
        params.set_temperature(0.0); // déterministe
        params.set_no_speech_thold(0.6); // skip si pas de parole détectée

        state
            .full(params, samples)
            .map_err(|e| anyhow!("Whisper full() : {}", e))?;

        let n = state.full_n_segments();
        let mut out = String::new();
        for i in 0..n {
            if let Some(seg) = state.get_segment(i) {
                let text = seg
                    .to_str_lossy()
                    .map_err(|e| anyhow!("segment {} : {}", i, e))?;
                out.push_str(&text);
            }
        }
        Ok(out)
    }
}
