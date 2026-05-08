use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;
use std::sync::Arc;

const TARGET_SAMPLE_RATE: u32 = 16_000;

pub struct AudioRecorder {
    device: Device,
    config: StreamConfig,
    sample_format: SampleFormat,
    stream: Option<Stream>,
    samples: Arc<Mutex<Vec<f32>>>,
    input_sample_rate: u32,
    input_channels: u16,
}

/// Liste les noms des microphones d'entrée disponibles.
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(n) = d.name() {
                names.push(n);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

impl AudioRecorder {
    pub fn new() -> Result<Self> {
        Self::new_with_device(None)
    }

    /// Crée le recorder. Si `device_name` est `None`, utilise le micro par défaut.
    /// Si le nom demandé n'est pas trouvé, retombe sur le micro par défaut.
    pub fn new_with_device(device_name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();

        let device = if let Some(name) = device_name {
            let mut found = None;
            if let Ok(devices) = host.input_devices() {
                for d in devices {
                    if d.name().ok().as_deref() == Some(name) {
                        found = Some(d);
                        break;
                    }
                }
            }
            match found {
                Some(d) => d,
                None => {
                    log::warn!("Microphone '{}' introuvable, repli sur le défaut.", name);
                    host.default_input_device()
                        .ok_or_else(|| anyhow!("Aucun microphone par défaut détecté"))?
                }
            }
        } else {
            host.default_input_device()
                .ok_or_else(|| anyhow!("Aucun microphone par défaut détecté"))?
        };

        let device_name = device.name().unwrap_or_else(|_| "<inconnu>".to_string());
        log::info!("Microphone : {}", device_name);

        let supported = device
            .default_input_config()
            .context("Impossible de récupérer la configuration audio par défaut")?;

        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let input_sample_rate = config.sample_rate.0;
        let input_channels = config.channels;

        log::info!(
            "Audio source : {} Hz, {} canal(aux), format {:?}",
            input_sample_rate,
            input_channels,
            sample_format
        );

        Ok(Self {
            device,
            config,
            sample_format,
            stream: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            input_sample_rate,
            input_channels,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        if self.stream.is_some() {
            return Err(anyhow!("Enregistrement déjà en cours"));
        }

        self.samples.lock().clear();
        let buf = Arc::clone(&self.samples);
        let channels = self.input_channels as usize;

        let err_fn = |e| log::error!("Erreur du flux audio : {}", e);

        let stream = match self.sample_format {
            SampleFormat::F32 => self.device.build_input_stream(
                &self.config,
                move |data: &[f32], _| append_mono(&buf, data, channels, |s| s),
                err_fn,
                None,
            )?,
            SampleFormat::I16 => self.device.build_input_stream(
                &self.config,
                move |data: &[i16], _| {
                    append_mono(&buf, data, channels, |s| s as f32 / i16::MAX as f32)
                },
                err_fn,
                None,
            )?,
            SampleFormat::U16 => self.device.build_input_stream(
                &self.config,
                move |data: &[u16], _| {
                    append_mono(&buf, data, channels, |s| (s as f32 - 32768.0) / 32768.0)
                },
                err_fn,
                None,
            )?,
            SampleFormat::I32 => self.device.build_input_stream(
                &self.config,
                move |data: &[i32], _| {
                    append_mono(&buf, data, channels, |s| s as f32 / i32::MAX as f32)
                },
                err_fn,
                None,
            )?,
            other => return Err(anyhow!("Format audio non supporté : {:?}", other)),
        };

        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }

    /// Stoppe la capture et retourne les échantillons mono à 16 kHz f32.
    pub fn stop(&mut self) -> Result<Vec<f32>> {
        if self.stream.take().is_none() {
            return Err(anyhow!("Aucun enregistrement en cours"));
        }
        // Le drop du stream stoppe la capture.
        let raw = std::mem::take(&mut *self.samples.lock());

        let resampled = if self.input_sample_rate != TARGET_SAMPLE_RATE {
            resample_linear(&raw, self.input_sample_rate, TARGET_SAMPLE_RATE)
        } else {
            raw
        };

        Ok(resampled)
    }

    /// Snapshot non-destructif : copie les échantillons capturés jusqu'à présent
    /// (sans stopper la capture), rééchantillonnés à 16 kHz mono.
    /// Si `last_seconds` est `Some(n)`, ne garde que les `n` dernières secondes.
    /// Indispensable pour la transcription en temps réel : on transcrit le buffer
    /// pendant que la capture continue.
    pub fn snapshot(&self, last_seconds: Option<f32>) -> Vec<f32> {
        let raw = self.samples.lock().clone();
        let resampled = if self.input_sample_rate != TARGET_SAMPLE_RATE {
            resample_linear(&raw, self.input_sample_rate, TARGET_SAMPLE_RATE)
        } else {
            raw
        };
        if let Some(secs) = last_seconds {
            let max_samples = (secs * TARGET_SAMPLE_RATE as f32) as usize;
            if resampled.len() > max_samples {
                let start = resampled.len() - max_samples;
                return resampled[start..].to_vec();
            }
        }
        resampled
    }

    pub fn is_recording(&self) -> bool {
        self.stream.is_some()
    }
}

fn append_mono<T: Copy>(
    buf: &Arc<Mutex<Vec<f32>>>,
    data: &[T],
    channels: usize,
    to_f32: impl Fn(T) -> f32,
) {
    let mut samples = buf.lock();
    if channels <= 1 {
        samples.reserve(data.len());
        for &s in data {
            samples.push(to_f32(s));
        }
    } else {
        samples.reserve(data.len() / channels);
        for frame in data.chunks(channels) {
            let mut sum = 0.0f32;
            for &s in frame {
                sum += to_f32(s);
            }
            samples.push(sum / channels as f32);
        }
    }
}

/// Rééchantillonnage linéaire simple (suffisant pour la voix vers 16 kHz).
fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() || from_rate == to_rate {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = ((input.len() as f64) / ratio).floor() as usize;
    let mut output = Vec::with_capacity(new_len);
    let last = input.len() - 1;
    for i in 0..new_len {
        let src = i as f64 * ratio;
        let i0 = src.floor() as usize;
        let i1 = (i0 + 1).min(last);
        let frac = (src - i0 as f64) as f32;
        output.push(input[i0] * (1.0 - frac) + input[i1] * frac);
    }
    output
}
