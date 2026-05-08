use crate::audio::AudioRecorder;
use crate::config::OutputMode;
use crate::output;
use crate::transcribe::WhisperTranscriber;
use crossbeam_channel::{Receiver, Sender};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

/// Fenêtre maximale envoyée à Whisper pour la transcription partielle (live).
/// Whisper a un contexte audio fixe de 30 s, on reste légèrement en dessous.
const LIVE_WINDOW_SECS: f32 = 25.0;
/// Intervalle minimum entre deux transcriptions partielles (live).
const LIVE_INTERVAL_MS: u64 = 1200;
/// Timeout court pour scruter les commandes pendant l'enregistrement live.
const LIVE_POLL_MS: u64 = 100;

#[derive(Debug, Clone)]
pub enum Cmd {
    /// Charger ou recharger le modèle Whisper.
    LoadModel {
        path: PathBuf,
        language: String,
        use_gpu: bool,
    },
    /// Démarrer la capture + transcription en streaming.
    StartLive { device: Option<String> },
    /// Stopper, transcrire l'intégralité, sortir le texte (mode = output_mode).
    Stop { output_mode: OutputMode },
    /// Quitter le worker proprement.
    Quit,
}

#[derive(Debug, Clone)]
pub enum Evt {
    ModelLoading {
        path: PathBuf,
    },
    ModelLoaded {
        path: PathBuf,
    },
    ModelError {
        path: PathBuf,
        error: String,
    },
    /// Capture audio démarrée (l'utilisateur peut parler).
    RecordingStarted,
    /// Tick périodique : durée capturée jusqu'à présent (s).
    RecordingTick {
        secs: f32,
    },
    /// Transcription partielle (live) : peut bouger entre 2 events.
    LivePartial {
        text: String,
    },
    /// Transcription finale appelée par Stop : texte + tentative de sortie.
    Result {
        text: String,
        output_done: bool,
    },
    /// Erreur générique.
    Error(String),
}

pub struct WorkerHandle {
    pub tx: Sender<Cmd>,
    pub rx: Receiver<Evt>,
    pub join: Option<thread::JoinHandle<()>>,
}

pub fn spawn() -> WorkerHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<Cmd>();
    let (evt_tx, evt_rx) = crossbeam_channel::unbounded::<Evt>();

    let join = thread::Builder::new()
        .name("nyxwhisper-worker".to_string())
        .spawn(move || run_loop(cmd_rx, evt_tx))
        .expect("failed to spawn worker thread");

    WorkerHandle {
        tx: cmd_tx,
        rx: evt_rx,
        join: Some(join),
    }
}

fn run_loop(rx: Receiver<Cmd>, tx: Sender<Evt>) {
    let mut transcriber: Option<WhisperTranscriber> = None;
    let mut recorder: Option<AudioRecorder> = None;
    let mut record_started: Option<Instant> = None;
    let mut last_partial_at: Option<Instant> = None;

    let send = |evt: Evt| {
        let _ = tx.send(evt);
    };

    loop {
        // Mode live : poll court pour pouvoir intercaler des transcriptions partielles
        // ; sinon, blocant.
        let cmd_opt = if recorder.is_some() {
            match rx.recv_timeout(Duration::from_millis(LIVE_POLL_MS)) {
                Ok(c) => Some(Some(c)),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => Some(None),
                Err(_) => return,
            }
        } else {
            match rx.recv() {
                Ok(c) => Some(Some(c)),
                Err(_) => return,
            }
        };

        let Some(cmd_opt) = cmd_opt else { continue };

        // ----- 1) Tick / transcription live (timeout) -----
        if cmd_opt.is_none() {
            // Tick UI
            if let Some(start) = record_started {
                send(Evt::RecordingTick {
                    secs: start.elapsed().as_secs_f32(),
                });
            }
            // Transcription partielle si l'intervalle est écoulé
            let due = match last_partial_at {
                None => true,
                Some(t) => t.elapsed() >= Duration::from_millis(LIVE_INTERVAL_MS),
            };
            if due {
                if let (Some(rec), Some(t)) = (recorder.as_ref(), transcriber.as_ref()) {
                    let samples = rec.snapshot(Some(LIVE_WINDOW_SECS));
                    // Marque maintenant pour ne pas repartir avant la fin.
                    last_partial_at = Some(Instant::now());
                    if samples.len() >= 16_000 {
                        // au moins 1 s
                        match t.transcribe(&samples) {
                            Ok(text) => {
                                let trimmed = text.trim().to_string();
                                send(Evt::LivePartial { text: trimmed });
                            }
                            Err(e) => log::warn!("Partial transcribe : {}", e),
                        }
                        // Repositionne la marque APRÈS la transcription pour
                        // que LIVE_INTERVAL_MS soit l'intervalle entre fins.
                        last_partial_at = Some(Instant::now());
                    }
                }
            }
            continue;
        }

        // ----- 2) Commande reçue -----
        let cmd = cmd_opt.unwrap();
        match cmd {
            Cmd::LoadModel {
                path,
                language,
                use_gpu,
            } => {
                send(Evt::ModelLoading { path: path.clone() });
                match WhisperTranscriber::new_with_options(&path, &language, use_gpu) {
                    Ok(t) => {
                        transcriber = Some(t);
                        send(Evt::ModelLoaded { path });
                    }
                    Err(e) => {
                        send(Evt::ModelError {
                            path,
                            error: format!("{:#}", e),
                        });
                    }
                }
            }

            Cmd::StartLive { device } => {
                if recorder.is_some() {
                    log::debug!("StartLive ignoré (déjà en cours)");
                    continue;
                }
                if transcriber.is_none() {
                    send(Evt::Error("Aucun modèle chargé".to_string()));
                    continue;
                }
                let mut rec = match AudioRecorder::new_with_device(device.as_deref()) {
                    Ok(r) => r,
                    Err(e) => {
                        send(Evt::Error(format!("Audio : {:#}", e)));
                        continue;
                    }
                };
                if let Err(e) = rec.start() {
                    send(Evt::Error(format!("Démarrage capture : {:#}", e)));
                    continue;
                }
                recorder = Some(rec);
                record_started = Some(Instant::now());
                last_partial_at = None;
                send(Evt::RecordingStarted);
            }

            Cmd::Stop { output_mode } => {
                let Some(mut rec) = recorder.take() else {
                    send(Evt::Error("Aucun enregistrement en cours".to_string()));
                    continue;
                };
                record_started = None;
                last_partial_at = None;

                let samples = match rec.stop() {
                    Ok(s) => s,
                    Err(e) => {
                        send(Evt::Error(format!("Arrêt capture : {:#}", e)));
                        continue;
                    }
                };

                let Some(t) = transcriber.as_ref() else {
                    send(Evt::Error("Aucun modèle chargé".to_string()));
                    continue;
                };

                let text = match t.transcribe(&samples) {
                    Ok(s) => s.trim().to_string(),
                    Err(e) => {
                        send(Evt::Error(format!("Transcription : {:#}", e)));
                        continue;
                    }
                };

                let mut output_done = false;
                if !text.is_empty() {
                    let res = match output_mode {
                        OutputMode::Type => output::type_text(&text),
                        OutputMode::Clipboard => output::copy_to_clipboard(&text),
                    };
                    match res {
                        Ok(()) => output_done = true,
                        Err(e) => {
                            log::warn!("Sortie : {}, fallback presse-papiers", e);
                            if let Err(e2) = output::copy_to_clipboard(&text) {
                                send(Evt::Error(format!("Sortie : {} / fallback : {}", e, e2)));
                            } else {
                                output_done = true;
                            }
                        }
                    }
                }

                send(Evt::Result { text, output_done });
            }

            Cmd::Quit => return,
        }
    }
}
