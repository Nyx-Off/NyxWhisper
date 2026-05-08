use crate::config::{list_models, Config, OutputMode};
use crate::hotkey;
use crate::worker::{self, Cmd, Evt, WorkerHandle};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicIsize, Ordering},
    Arc, Mutex,
};
use std::time::Instant;
use tray_icon::{menu::MenuEvent, MouseButton, MouseButtonState, TrayIconEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
enum UiState {
    NoModel,
    LoadingModel,
    ModelError(String),
    Idle,
    Recording,
    Transcribing,
}

#[derive(Debug, Clone)]
enum RecordingTransition {
    StartRequested,
    StopRequested,
    Failed(String),
}

#[derive(Debug, Clone)]
enum ExternalEvent {
    Recording(RecordingTransition),
    RestoreMainWindow,
    Quit,
}

#[derive(Clone)]
struct RecordingController {
    config: Arc<Mutex<Config>>,
    worker_tx: Sender<Cmd>,
    is_recording: Arc<AtomicBool>,
    model_ready: Arc<AtomicBool>,
}

impl RecordingController {
    fn toggle_recording(&self) -> RecordingTransition {
        if !self.is_recording.load(Ordering::SeqCst) && !self.model_ready.load(Ordering::SeqCst) {
            return RecordingTransition::Failed("Modèle Whisper pas encore prêt.".to_string());
        }

        let cfg = self.config.lock().unwrap().clone();

        if self
            .is_recording
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            if let Err(e) = self.worker_tx.send(Cmd::StartLive {
                device: cfg.microphone,
            }) {
                self.is_recording.store(false, Ordering::SeqCst);
                return RecordingTransition::Failed(format!(
                    "Impossible de démarrer l'enregistrement : {}",
                    e
                ));
            }
            if cfg.notifications {
                crate::tray::notify(
                    "NyxWhisper",
                    &format!(
                        "Enregistrement en cours. Réappuie sur {} pour transcrire.",
                        hotkey::human_label(&cfg.hotkey)
                    ),
                );
            }
            RecordingTransition::StartRequested
        } else {
            self.is_recording.store(false, Ordering::SeqCst);
            if let Err(e) = self.worker_tx.send(Cmd::Stop {
                output_mode: cfg.output_mode,
            }) {
                self.is_recording.store(true, Ordering::SeqCst);
                return RecordingTransition::Failed(format!(
                    "Impossible d'arrêter l'enregistrement : {}",
                    e
                ));
            }
            if cfg.notifications {
                crate::tray::notify("NyxWhisper", "Transcription en cours…");
            }
            RecordingTransition::StopRequested
        }
    }
}

pub struct App {
    config: Arc<Mutex<Config>>,

    worker: WorkerHandle,
    recording_controller: RecordingController,
    hotkey_manager: GlobalHotKeyManager,
    current_hotkey: Option<HotKey>,
    hotkey_input: String,
    hotkey_error: Option<String>,

    state: UiState,
    record_started: Option<Instant>,
    record_secs: f32,
    last_audio_secs: f32,

    /// Texte partiel reçu pendant la capture (live).
    live_text: String,
    last_text: String,
    history: VecDeque<String>,
    status_msg: String,

    available_models: Vec<PathBuf>,
    models_dir: PathBuf,

    available_microphones: Vec<String>,

    /// Téléchargement du modèle en cours (onboarding ou bouton "télécharger").
    download_state: Option<DownloadState>,
    /// Affiche la fenêtre modale "Télécharger un autre modèle".
    show_download_dialog: bool,

    /// Tray icon (None si l'init a échoué ; on continue sans).
    _tray: Option<crate::tray::AppTray>,
    /// Si `true`, l'app a reçu une demande de fermeture et doit quitter
    /// (par opposition à se cacher dans le tray).
    quit_requested: Arc<AtomicBool>,
    external_rx: Receiver<ExternalEvent>,
    window_hwnd: Arc<AtomicIsize>,
    _model_ready: Arc<AtomicBool>,

    is_recording: Arc<AtomicBool>,
}

struct DownloadState {
    model_name: &'static str,
    progress: crate::download::SharedProgress,
    cancel_flag: Arc<AtomicBool>,
    _join: std::thread::JoinHandle<()>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        log::info!("App init: config");
        let config = Arc::new(Mutex::new(Config::load()));
        log::info!("App init: worker");
        let worker = worker::spawn();
        log::info!("App init: hotkey manager");
        let hotkey_manager = GlobalHotKeyManager::new()
            .expect("Impossible de créer le gestionnaire de raccourcis globaux");

        log::info!("App init: models dir");
        let models_dir = locate_models_dir();

        log::info!("App init: tray");
        let tray = match crate::tray::AppTray::new() {
            Ok(t) => Some(t),
            Err(e) => {
                log::warn!("Tray icon indisponible : {}", e);
                None
            }
        };

        let is_recording = Arc::new(AtomicBool::new(false));
        let model_ready = Arc::new(AtomicBool::new(false));
        let quit_requested = Arc::new(AtomicBool::new(false));
        let window_hwnd = Arc::new(AtomicIsize::new(0));
        let (external_tx, external_rx) = crossbeam_channel::unbounded();
        let recording_controller = RecordingController {
            config: Arc::clone(&config),
            worker_tx: worker.tx.clone(),
            is_recording: Arc::clone(&is_recording),
            model_ready: Arc::clone(&model_ready),
        };

        let hotkey_input = config.lock().unwrap().hotkey.clone();
        log::info!("App init: list models/devices");
        let available_models = list_models(&models_dir);
        let available_microphones = crate::audio::list_input_devices();
        log::info!("App init: build app state");
        let mut app = Self {
            hotkey_input,
            available_models,
            models_dir,
            available_microphones,
            config,
            worker,
            recording_controller,
            hotkey_manager,
            current_hotkey: None,
            hotkey_error: None,
            state: UiState::NoModel,
            record_started: None,
            record_secs: 0.0,
            last_audio_secs: 0.0,
            live_text: String::new(),
            last_text: String::new(),
            history: VecDeque::with_capacity(8),
            status_msg: String::new(),
            download_state: None,
            show_download_dialog: false,
            _tray: tray,
            quit_requested,
            external_rx,
            window_hwnd,
            _model_ready: model_ready,
            is_recording,
        };

        // Enregistrer le raccourci actuel
        log::info!("App init: register hotkey");
        app.try_register_hotkey();
        app.install_external_event_handlers(cc.egui_ctx.clone(), external_tx);

        // Charger le modèle si présent
        log::info!("App init: load model if present");
        let cfg = app.config.lock().unwrap().clone();
        if cfg.model_path.exists() {
            app.send_load_model();
        } else if let Some(first) = app.available_models.first().cloned() {
            app.config.lock().unwrap().model_path = first;
            let _ = app.config.lock().unwrap().save();
            app.send_load_model();
        }

        log::info!("App init: done");
        app
    }

    fn send_load_model(&mut self) {
        let cfg = self.config.lock().unwrap().clone();
        self._model_ready.store(false, Ordering::SeqCst);
        self.state = UiState::LoadingModel;
        self.status_msg = format!("Chargement du modèle {}…", cfg.model_path.display());
        let _ = self.worker.tx.send(Cmd::LoadModel {
            path: cfg.model_path,
            language: cfg.language,
            use_gpu: cfg.use_gpu,
        });
    }

    fn try_register_hotkey(&mut self) {
        if let Some(prev) = self.current_hotkey.take() {
            let _ = self.hotkey_manager.unregister(prev);
        }
        let hotkey = self.config.lock().unwrap().hotkey.clone();
        match hotkey::parse(&hotkey) {
            Ok(hk) => match self.hotkey_manager.register(hk) {
                Ok(()) => {
                    self.current_hotkey = Some(hk);
                    self.hotkey_error = None;
                }
                Err(e) => {
                    self.hotkey_error =
                        Some(format!("Impossible d'enregistrer ({}) : {}", hotkey, e));
                }
            },
            Err(e) => {
                self.hotkey_error = Some(format!("{}", e));
            }
        }
    }

    fn install_external_event_handlers(&self, ctx: egui::Context, tx: Sender<ExternalEvent>) {
        let hotkey_tx = tx.clone();
        let hotkey_ctx = ctx.clone();
        let hotkey_controller = self.recording_controller.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |evt: GlobalHotKeyEvent| {
            if evt.state == HotKeyState::Pressed {
                let transition = hotkey_controller.toggle_recording();
                let _ = hotkey_tx.send(ExternalEvent::Recording(transition));
                hotkey_ctx.request_repaint();
            }
        }));

        let menu_tx = tx.clone();
        let menu_ctx = ctx.clone();
        let menu_controller = self.recording_controller.clone();
        let menu_hwnd = Arc::clone(&self.window_hwnd);
        let menu_quit_requested = Arc::clone(&self.quit_requested);
        MenuEvent::set_event_handler(Some(move |menu_evt: MenuEvent| {
            let id = menu_evt.id().0.as_str().to_string();
            match id.as_str() {
                crate::tray::ids::SHOW => {
                    restore_native_window(&menu_hwnd);
                    let _ = menu_tx.send(ExternalEvent::RestoreMainWindow);
                    menu_ctx.request_repaint();
                }
                crate::tray::ids::TOGGLE_REC => {
                    let transition = menu_controller.toggle_recording();
                    let _ = menu_tx.send(ExternalEvent::Recording(transition));
                    menu_ctx.request_repaint();
                }
                crate::tray::ids::QUIT => {
                    menu_quit_requested.store(true, Ordering::SeqCst);
                    restore_native_window(&menu_hwnd);
                    let _ = menu_tx.send(ExternalEvent::Quit);
                    menu_ctx.request_repaint();
                }
                _ => {}
            }
        }));

        let icon_tx = tx;
        let icon_ctx = ctx;
        let icon_hwnd = Arc::clone(&self.window_hwnd);
        TrayIconEvent::set_event_handler(Some(move |evt: TrayIconEvent| {
            if matches!(
                evt,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                restore_native_window(&icon_hwnd);
                let _ = icon_tx.send(ExternalEvent::RestoreMainWindow);
                icon_ctx.request_repaint();
            }
        }));
    }

    fn toggle_recording(&mut self) {
        let transition = self.recording_controller.toggle_recording();
        self.apply_recording_transition(transition);
    }

    fn apply_recording_transition(&mut self, transition: RecordingTransition) {
        match transition {
            RecordingTransition::StartRequested => {
                self.state = UiState::Recording;
                self.record_secs = 0.0;
                self.live_text.clear();
                self.status_msg = "Démarrage de l'enregistrement…".to_string();
            }
            RecordingTransition::StopRequested => {
                self.state = UiState::Transcribing;
                self.status_msg = "Finalisation de la transcription…".to_string();
            }
            RecordingTransition::Failed(msg) => {
                self.status_msg = msg;
                if matches!(self.state, UiState::Recording | UiState::Transcribing) {
                    self.state = UiState::Idle;
                    self.record_started = None;
                }
            }
        }
    }

    fn restore_main_window(ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn process_external_events(&mut self, ctx: &egui::Context) {
        while let Ok(evt) = self.external_rx.try_recv() {
            match evt {
                ExternalEvent::Recording(transition) => {
                    self.apply_recording_transition(transition);
                }
                ExternalEvent::RestoreMainWindow => Self::restore_main_window(ctx),
                ExternalEvent::Quit => {
                    self.quit_requested.store(true, Ordering::SeqCst);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn remember_native_window(&self, frame: &eframe::Frame) {
        #[cfg(target_os = "windows")]
        {
            if self.window_hwnd.load(Ordering::SeqCst) != 0 {
                return;
            }
            let Ok(handle) = frame.window_handle() else {
                return;
            };
            if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                self.window_hwnd.store(handle.hwnd.get(), Ordering::SeqCst);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = frame;
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        // Important : on minimise au lieu de rendre la fenêtre invisible.
        // Les callbacks natifs du tray/raccourci prennent ensuite le relais si
        // eframe ne repeint plus pendant que la fenêtre est minimisée.
        let quit_requested = self.quit_requested.load(Ordering::SeqCst);
        if self.config.lock().unwrap().close_to_tray && !quit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            if self.config.lock().unwrap().notifications {
                crate::tray::notify("NyxWhisper", "Réduit. Le raccourci reste actif.");
            }
        }
        // Sinon, on laisse la fenêtre se fermer (eframe va quitter).
    }

    fn start_download(&mut self, opt: &crate::download::ModelOption) {
        if self.download_state.is_some() {
            return;
        }
        let progress = crate::download::new_progress();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let dest_dir = self.models_dir.clone();
        let join = crate::download::spawn_download(
            opt.file.to_string(),
            dest_dir,
            std::sync::Arc::clone(&progress),
            std::sync::Arc::clone(&cancel_flag),
        );
        self.download_state = Some(DownloadState {
            model_name: opt.name,
            progress,
            cancel_flag,
            _join: join,
        });
    }

    /// Vérifie l'état du téléchargement courant et nettoie / charge le modèle
    /// quand il est terminé.
    fn poll_download(&mut self) {
        let Some(ds) = self.download_state.as_ref() else {
            return;
        };
        let p = ds.progress.lock().clone();
        if !p.finished {
            return;
        }
        let model_name = ds.model_name;
        self.download_state = None;
        if let Some(err) = p.error {
            self.status_msg = if p.cancelled {
                format!("Téléchargement {} annulé.", model_name)
            } else {
                format!("Erreur téléchargement {} : {}", model_name, err)
            };
            return;
        }
        if let Some(path) = p.final_path {
            self.status_msg = format!(
                "Modèle {} téléchargé ({}).",
                model_name,
                crate::download::human_bytes(p.downloaded)
            );
            self.available_models = list_models(&self.models_dir);
            self.config.lock().unwrap().model_path = path;
            let _ = self.config.lock().unwrap().save();
            self.send_load_model();
        }
    }

    fn render_onboarding(&mut self, ui: &mut egui::Ui) {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.heading("Bienvenue dans NyxWhisper");
            ui.add_space(6.0);
            ui.weak("Aucun modèle Whisper trouvé. Choisis un modèle à télécharger.");
            ui.weak(format!("Destination : {}", self.models_dir.display()));
        });
        ui.add_space(16.0);

        let mut to_download: Option<crate::download::ModelOption> = None;
        egui::ScrollArea::vertical()
            .id_source("onboarding")
            .show(ui, |ui| {
                for opt in crate::download::MODEL_OPTIONS {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.strong(format!("{} ({} Mo)", opt.name, opt.size_mb));
                                ui.weak(opt.description);
                                ui.weak(format!("Fichier : {}", opt.file));
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let btn = egui::Button::new(
                                        egui::RichText::new("⬇  Télécharger")
                                            .color(egui::Color32::WHITE),
                                    )
                                    .fill(egui::Color32::from_rgb(70, 130, 220))
                                    .min_size(egui::vec2(140.0, 36.0));
                                    if ui.add(btn).clicked() {
                                        to_download = Some(opt.clone());
                                    }
                                },
                            );
                        });
                    });
                    ui.add_space(4.0);
                }
            });
        if let Some(opt) = to_download {
            self.start_download(&opt);
        }
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("📁 Ouvrir le dossier des modèles").clicked() {
                let _ = std::fs::create_dir_all(&self.models_dir);
                let _ = open_path(&self.models_dir);
            }
            if ui.button("⟳ J'ai placé un .bin manuellement").clicked() {
                self.available_models = list_models(&self.models_dir);
                if let Some(first) = self.available_models.first().cloned() {
                    self.config.lock().unwrap().model_path = first;
                    let _ = self.config.lock().unwrap().save();
                    self.send_load_model();
                }
            }
        });
    }

    fn render_download_panel(&mut self, ui: &mut egui::Ui) {
        let Some(ds) = self.download_state.as_ref() else {
            return;
        };
        let p = ds.progress.lock().clone();
        let model_name = ds.model_name;

        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.heading(format!("Téléchargement de {}…", model_name));
            ui.add_space(8.0);
            let downloaded = crate::download::human_bytes(p.downloaded);
            let total = p
                .total
                .map(crate::download::human_bytes)
                .unwrap_or_else(|| "?".to_string());
            ui.label(format!("{} / {}", downloaded, total));

            let frac = match p.total {
                Some(t) if t > 0 => p.downloaded as f32 / t as f32,
                _ => 0.0,
            };
            let bar = egui::ProgressBar::new(frac.clamp(0.0, 1.0))
                .show_percentage()
                .desired_width(440.0);
            ui.add(bar);

            ui.add_space(12.0);
            if ui.button("Annuler").clicked() {
                ds.cancel_flag
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });
    }

    /// Mini-fenêtre flottante always-on-top affichée pendant l'enregistrement.
    /// Utilise un viewport secondaire egui (multi-fenêtre).
    fn render_live_overlay(&mut self, ctx: &egui::Context) {
        let recording = matches!(self.state, UiState::Recording);
        let secs = self.record_secs;
        let live = self.live_text.clone();
        let viewport_id = egui::ViewportId::from_hash_of("nyx-live-overlay");
        let title = if recording {
            format!("● Dictée — {:.1}s", secs)
        } else {
            "Transcription…".to_string()
        };

        let builder = egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([520.0, 130.0])
            .with_min_inner_size([320.0, 100.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false)
            .with_taskbar(false)
            .with_transparent(false);

        ctx.show_viewport_immediate(viewport_id, builder, move |ctx, _class| {
            let frame = egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(15, 15, 22, 245))
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .stroke(egui::Stroke::new(
                    1.5,
                    if recording {
                        egui::Color32::from_rgb(220, 70, 70)
                    } else {
                        egui::Color32::from_rgb(220, 170, 60)
                    },
                ))
                .rounding(egui::Rounding::same(8.0));

            egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (col, dot) = if recording {
                        (egui::Color32::from_rgb(220, 70, 70), "●")
                    } else {
                        (egui::Color32::from_rgb(220, 170, 60), "⌛")
                    };
                    ui.colored_label(col, egui::RichText::new(dot).size(18.0).strong());
                    let header = if recording {
                        format!("Dictée  ·  {:.1} s", secs)
                    } else {
                        "Transcription en cours…".to_string()
                    };
                    ui.label(
                        egui::RichText::new(header)
                            .color(egui::Color32::from_rgb(220, 220, 230))
                            .size(13.0),
                    );
                });
                ui.add_space(4.0);
                let display = if live.is_empty() {
                    if recording {
                        "Parle maintenant…".to_string()
                    } else {
                        "…".to_string()
                    }
                } else {
                    live.clone()
                };
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(display)
                            .color(egui::Color32::from_rgb(240, 240, 245))
                            .size(15.0),
                    )
                    .wrap(),
                );
            });

            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        });
    }

    /// Fenêtre modale "Télécharger un autre modèle". Accessible n'importe quand
    /// via le bouton ⬇ Télécharger… du panneau de droite.
    fn render_download_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_download_dialog;
        let download_active = self.download_state.is_some();
        // Liste des fichiers déjà installés (par nom de fichier).
        let installed: std::collections::HashSet<String> = self
            .available_models
            .iter()
            .filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(String::from))
            .collect();

        let mut to_download: Option<crate::download::ModelOption> = None;
        let mut close_after = false;

        egui::Window::new("Télécharger un modèle Whisper")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.weak(format!("Destination : {}", self.models_dir.display()));
                ui.add_space(4.0);
                if download_active {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 170, 60),
                        "Un téléchargement est déjà en cours…",
                    );
                    ui.add_space(4.0);
                }
                egui::ScrollArea::vertical()
                    .id_source("dl_dialog_scroll")
                    .max_height(360.0)
                    .show(ui, |ui| {
                        for opt in crate::download::MODEL_OPTIONS {
                            let already = installed.contains(opt.file);
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.strong(format!("{} ({} Mo)", opt.name, opt.size_mb));
                                            if already {
                                                ui.colored_label(
                                                    egui::Color32::from_rgb(80, 180, 120),
                                                    "✓ Installé",
                                                );
                                            }
                                        });
                                        ui.weak(opt.description);
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let label = if already {
                                                "Re-télécharger"
                                            } else {
                                                "Télécharger"
                                            };
                                            let btn = egui::Button::new(label)
                                                .min_size(egui::vec2(120.0, 32.0));
                                            if ui.add_enabled(!download_active, btn).clicked() {
                                                to_download = Some(*opt);
                                                close_after = true;
                                            }
                                        },
                                    );
                                });
                            });
                            ui.add_space(2.0);
                        }
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Fermer").clicked() {
                        close_after = true;
                    }
                });
            });

        if let Some(opt) = to_download {
            // Pour re-télécharger, on supprime le .bin existant pour forcer la
            // récupération propre.
            if installed.contains(opt.file) {
                let p = self.models_dir.join(opt.file);
                let _ = std::fs::remove_file(&p);
            }
            self.start_download(&opt);
        }
        if close_after {
            self.show_download_dialog = false;
        } else {
            self.show_download_dialog = open;
        }
    }

    fn process_worker_events(&mut self) {
        while let Ok(evt) = self.worker.rx.try_recv() {
            match evt {
                Evt::ModelLoading { path } => {
                    self._model_ready.store(false, Ordering::SeqCst);
                    self.state = UiState::LoadingModel;
                    self.status_msg = format!("Chargement de {}…", path.display());
                }
                Evt::ModelLoaded { path } => {
                    self._model_ready.store(true, Ordering::SeqCst);
                    self.state = UiState::Idle;
                    self.status_msg = format!("Modèle prêt : {}", path.display());
                }
                Evt::ModelError { path, error } => {
                    self._model_ready.store(false, Ordering::SeqCst);
                    self.state = UiState::ModelError(error.clone());
                    self.status_msg = format!("Erreur modèle ({}) : {}", path.display(), error);
                }
                Evt::RecordingStarted => {
                    self.state = UiState::Recording;
                    self.record_started = Some(Instant::now());
                    self.record_secs = 0.0;
                    self.live_text.clear();
                    self.is_recording.store(true, Ordering::SeqCst);
                    self.status_msg = "Enregistrement…".to_string();
                }
                Evt::RecordingTick { secs } => {
                    self.record_secs = secs;
                }
                Evt::LivePartial { text } => {
                    self.live_text = text;
                }
                Evt::Result { text, output_done } => {
                    self.state = UiState::Idle;
                    self.record_started = None;
                    self.is_recording.store(false, Ordering::SeqCst);
                    if text.is_empty() {
                        self.status_msg = "Aucun texte détecté.".to_string();
                        if self.config.lock().unwrap().notifications {
                            crate::tray::notify(
                                "NyxWhisper",
                                "Aucun texte détecté dans l'enregistrement.",
                            );
                        }
                    } else {
                        self.status_msg = if output_done {
                            match self.config.lock().unwrap().output_mode {
                                OutputMode::Type => {
                                    "Texte tapé dans la fenêtre active.".to_string()
                                }
                                OutputMode::Clipboard => {
                                    "Texte copié dans le presse-papiers.".to_string()
                                }
                            }
                        } else {
                            "Texte transcrit (sortie échouée).".to_string()
                        };
                        if self.config.lock().unwrap().notifications {
                            let preview: String = text.chars().take(80).collect();
                            let preview = if text.chars().count() > 80 {
                                format!("{}…", preview)
                            } else {
                                preview
                            };
                            crate::tray::notify(
                                &format!(
                                    "NyxWhisper — {} caractères transcrits",
                                    text.chars().count()
                                ),
                                &preview,
                            );
                        }
                        self.history.push_front(text.clone());
                        while self.history.len() > 6 {
                            self.history.pop_back();
                        }
                        self.last_text = text;
                    }
                }
                Evt::Error(e) => {
                    self.status_msg = format!("Erreur : {}", e);
                    if matches!(self.state, UiState::Recording | UiState::Transcribing) {
                        self.state = UiState::Idle;
                        self.record_started = None;
                        self.is_recording.store(false, Ordering::SeqCst);
                    }
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.remember_native_window(frame);
        // Les callbacks natifs font le travail critique même si la fenêtre est
        // minimisée ; ce channel sert à synchroniser l'état UI dès qu'elle repeint.
        self.process_external_events(ctx);
        self.process_worker_events();
        self.poll_download();
        self.handle_close_request(ctx);

        // Repaint régulier pendant les phases actives pour rafraîchir le timer
        // ou la progression d'un téléchargement.
        if matches!(
            self.state,
            UiState::Recording | UiState::Transcribing | UiState::LoadingModel
        ) || self.download_state.is_some()
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else {
            // L'app doit poller les channels externes (raccourcis globaux, tray)
            // même quand elle est idle, particulièrement si la fenêtre est cachée
            // dans le tray (sinon eframe s'endort indéfiniment).
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }

        // ===== Fenêtre modale : Télécharger un autre modèle =====
        if self.show_download_dialog {
            self.render_download_dialog(ctx);
        }

        // ===== Mini-overlay live (viewport secondaire flottant) =====
        if self.config.lock().unwrap().live_overlay
            && matches!(self.state, UiState::Recording | UiState::Transcribing)
        {
            self.render_live_overlay(ctx);
        }

        // ===== Barre supérieure : statut =====
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let (color, label) = match &self.state {
                    UiState::NoModel => (egui::Color32::from_rgb(180, 140, 0), "Aucun modèle"),
                    UiState::LoadingModel => {
                        (egui::Color32::from_rgb(120, 160, 220), "Chargement…")
                    }
                    UiState::ModelError(_) => (egui::Color32::from_rgb(220, 90, 90), "Erreur"),
                    UiState::Idle => (egui::Color32::from_rgb(80, 180, 120), "Prêt"),
                    UiState::Recording => {
                        (egui::Color32::from_rgb(220, 70, 70), "● Enregistrement")
                    }
                    UiState::Transcribing => {
                        (egui::Color32::from_rgb(220, 170, 60), "Transcription…")
                    }
                };
                ui.colored_label(color, egui::RichText::new(label).strong().size(16.0));
                ui.separator();
                ui.label(&self.status_msg);
            });
            ui.add_space(6.0);
        });

        // ===== Panneau latéral : réglages =====
        egui::SidePanel::right("settings")
            .resizable(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("Réglages");
                ui.add_space(8.0);

                ui.label("Modèle Whisper :");
                let current = self.config.lock().unwrap().model_path.clone();
                let label = current
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "(aucun)".to_string());
                let mut model_choice: Option<PathBuf> = None;
                egui::ComboBox::from_id_source("model_combo")
                    .selected_text(label)
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        if self.available_models.is_empty() {
                            ui.label("(dossier models/ vide)");
                        }
                        for m in &self.available_models {
                            let name = m
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if ui.selectable_label(*m == current, name).clicked() {
                                model_choice = Some(m.clone());
                            }
                        }
                    });
                if let Some(chosen) = model_choice {
                    self.config.lock().unwrap().model_path = chosen;
                    let _ = self.config.lock().unwrap().save();
                    self.send_load_model();
                }
                ui.horizontal(|ui| {
                    if ui
                        .button("⟳")
                        .on_hover_text("Rafraîchir la liste")
                        .clicked()
                    {
                        self.available_models = list_models(&self.models_dir);
                    }
                    if ui
                        .button("📁")
                        .on_hover_text("Ouvrir le dossier des modèles")
                        .clicked()
                    {
                        let _ = open_path(&self.models_dir);
                    }
                    if ui
                        .button("⬇ Télécharger…")
                        .on_hover_text("Télécharger un autre modèle Whisper")
                        .clicked()
                    {
                        self.show_download_dialog = true;
                    }
                });

                ui.add_space(10.0);
                ui.label("Microphone :");
                let mic_label = self
                    .config
                    .lock()
                    .unwrap()
                    .microphone
                    .clone()
                    .unwrap_or_else(|| "(défaut système)".to_string());
                let mut mic_choice: Option<Option<String>> = None;
                egui::ComboBox::from_id_source("mic_combo")
                    .selected_text(mic_label.clone())
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        let is_default = self.config.lock().unwrap().microphone.is_none();
                        if ui
                            .selectable_label(is_default, "(défaut système)")
                            .clicked()
                        {
                            mic_choice = Some(None);
                        }
                        for name in &self.available_microphones {
                            let selected =
                                self.config.lock().unwrap().microphone.as_deref() == Some(name);
                            if ui.selectable_label(selected, name).clicked() {
                                mic_choice = Some(Some(name.clone()));
                            }
                        }
                    });
                if let Some(choice) = mic_choice {
                    self.config.lock().unwrap().microphone = choice;
                    let _ = self.config.lock().unwrap().save();
                }
                if ui
                    .button("⟳")
                    .on_hover_text("Rafraîchir la liste des micros")
                    .clicked()
                {
                    self.available_microphones = crate::audio::list_input_devices();
                }

                ui.add_space(10.0);
                ui.label("Langue (code ISO) :");
                let mut lang = self.config.lock().unwrap().language.clone();
                if ui.text_edit_singleline(&mut lang).changed() {
                    self.config.lock().unwrap().language = lang;
                }
                if ui.button("Appliquer la langue").clicked() {
                    let _ = self.config.lock().unwrap().save();
                    self.send_load_model();
                }

                ui.add_space(10.0);
                ui.label("Raccourci (ex: Control+Alt+Space) :");
                ui.text_edit_singleline(&mut self.hotkey_input);
                ui.horizontal(|ui| {
                    if ui.button("Appliquer le raccourci").clicked() {
                        self.config.lock().unwrap().hotkey = self.hotkey_input.trim().to_string();
                        let _ = self.config.lock().unwrap().save();
                        self.try_register_hotkey();
                    }
                });
                if let Some(err) = &self.hotkey_error {
                    ui.colored_label(egui::Color32::from_rgb(220, 90, 90), err);
                } else {
                    ui.weak(format!(
                        "Actif : {}",
                        hotkey::human_label(&self.config.lock().unwrap().hotkey)
                    ));
                }

                ui.add_space(10.0);
                ui.label("Sortie :");
                let mut mode = self.config.lock().unwrap().output_mode;
                ui.radio_value(&mut mode, OutputMode::Type, "Saisie clavier (texte tapé)");
                ui.radio_value(&mut mode, OutputMode::Clipboard, "Presse-papiers (Ctrl+V)");
                if mode != self.config.lock().unwrap().output_mode {
                    self.config.lock().unwrap().output_mode = mode;
                    let _ = self.config.lock().unwrap().save();
                }

                ui.add_space(10.0);
                ui.label("Comportement :");
                let mut close_to_tray = self.config.lock().unwrap().close_to_tray;
                if ui
                    .checkbox(
                        &mut close_to_tray,
                        "Réduire dans la barre des tâches (au lieu de quitter)",
                    )
                    .changed()
                {
                    self.config.lock().unwrap().close_to_tray = close_to_tray;
                    let _ = self.config.lock().unwrap().save();
                }
                let mut notifications = self.config.lock().unwrap().notifications;
                if ui
                    .checkbox(
                        &mut notifications,
                        "Notifications Windows (start / stop / résultat)",
                    )
                    .changed()
                {
                    self.config.lock().unwrap().notifications = notifications;
                    let _ = self.config.lock().unwrap().save();
                }
                let mut live_overlay = self.config.lock().unwrap().live_overlay;
                if ui
                    .checkbox(&mut live_overlay, "Mini-overlay live pendant la dictée")
                    .changed()
                {
                    self.config.lock().unwrap().live_overlay = live_overlay;
                    let _ = self.config.lock().unwrap().save();
                }

                ui.add_space(10.0);
                ui.label("Calcul (CPU / GPU) :");
                ui.weak(format!(
                    "Backend GPU compilé : {}",
                    crate::transcribe::COMPILED_GPU_BACKEND
                ));
                if crate::transcribe::HAS_GPU_BACKEND {
                    let mut use_gpu = self.config.lock().unwrap().use_gpu;
                    if ui
                        .checkbox(
                            &mut use_gpu,
                            format!(
                                "Utiliser le GPU ({})",
                                crate::transcribe::COMPILED_GPU_BACKEND
                            ),
                        )
                        .changed()
                    {
                        self.config.lock().unwrap().use_gpu = use_gpu;
                        let _ = self.config.lock().unwrap().save();
                        self.send_load_model();
                    }
                } else {
                    ui.weak("Pour activer le GPU, recompiler avec :");
                    ui.code("scripts\\build-cuda.ps1   (NVIDIA)");
                    ui.code("scripts\\build-vulkan.ps1 (universel)");
                }

                ui.add_space(16.0);
                ui.separator();
                ui.weak(format!(
                    "Config : {}",
                    Config::config_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| "?".into())
                ));
            });

        // ===== Centre =====
        egui::CentralPanel::default().show(ctx, |ui| {
            // 1) Onboarding : aucun modèle local + pas de téléchargement -> propose un choix.
            // 2) Téléchargement en cours : progress bar.
            // 3) Mode normal.

            if self.download_state.is_some() {
                self.render_download_panel(ui);
                return;
            }
            if self.available_models.is_empty() {
                self.render_onboarding(ui);
                return;
            }

            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                let busy = matches!(
                    self.state,
                    UiState::LoadingModel
                        | UiState::Transcribing
                        | UiState::NoModel
                        | UiState::ModelError(_)
                );
                let recording = matches!(self.state, UiState::Recording);

                let label = if recording {
                    format!("⏹  Stop & Transcrire ({:.1}s)", self.record_secs)
                } else {
                    "🎙  Démarrer (ou raccourci)".to_string()
                };
                let color = if recording {
                    egui::Color32::from_rgb(220, 70, 70)
                } else {
                    egui::Color32::from_rgb(70, 130, 220)
                };

                let btn = egui::Button::new(
                    egui::RichText::new(label)
                        .color(egui::Color32::WHITE)
                        .size(18.0),
                )
                .fill(color)
                .min_size(egui::vec2(320.0, 64.0));

                if ui.add_enabled(!busy, btn).clicked() {
                    self.toggle_recording();
                }

                ui.add_space(6.0);
                ui.weak(format!(
                    "Raccourci global : {}",
                    hotkey::human_label(&self.config.lock().unwrap().hotkey)
                ));
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // Zone live (visible pendant l'enregistrement)
            if matches!(self.state, UiState::Recording | UiState::Transcribing) {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(220, 70, 70), "● LIVE");
                    ui.weak("(transcription en cours, peut bouger)");
                });
                ui.add_space(2.0);
                egui::ScrollArea::vertical()
                    .id_source("live")
                    .max_height(120.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        let display = if self.live_text.is_empty() {
                            "…".to_string()
                        } else {
                            self.live_text.clone()
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(display)
                                    .italics()
                                    .color(egui::Color32::from_rgb(180, 200, 220)),
                            )
                            .wrap(),
                        );
                    });
                ui.add_space(8.0);
            }

            ui.heading("Dernière transcription");
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_source("last")
                .max_height(140.0)
                .show(ui, |ui| {
                    let mut text = self.last_text.clone();
                    ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
            ui.horizontal(|ui| {
                if ui.button("📋 Copier").clicked() && !self.last_text.is_empty() {
                    let _ = crate::output::copy_to_clipboard(&self.last_text);
                    self.status_msg = "Copié.".to_string();
                }
                if ui.button("⌨ Retaper").clicked() && !self.last_text.is_empty() {
                    if let Err(e) = crate::output::type_text(&self.last_text) {
                        self.status_msg = format!("Erreur saisie : {}", e);
                    } else {
                        self.status_msg = "Retapé dans la fenêtre active.".to_string();
                    }
                }
                if ui.button("🗑 Effacer").clicked() {
                    self.last_text.clear();
                }
            });

            if !self.history.is_empty() {
                ui.add_space(12.0);
                ui.collapsing("Historique récent", |ui| {
                    egui::ScrollArea::vertical()
                        .id_source("hist")
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for (i, h) in self.history.iter().enumerate() {
                                ui.group(|ui| {
                                    ui.label(egui::RichText::new(h).monospace());
                                    ui.horizontal(|ui| {
                                        if ui.small_button("Copier").clicked() {
                                            let _ = crate::output::copy_to_clipboard(h);
                                        }
                                        if ui.small_button("Taper").clicked() {
                                            let _ = crate::output::type_text(h);
                                        }
                                        ui.weak(format!("#{}", i + 1));
                                    });
                                });
                            }
                        });
                });
            }
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.worker.tx.send(Cmd::Quit);
        if let Some(prev) = self.current_hotkey.take() {
            let _ = self.hotkey_manager.unregister(prev);
        }
    }
}

#[cfg(target_os = "windows")]
fn restore_native_window(hwnd: &Arc<AtomicIsize>) {
    let hwnd = hwnd.load(Ordering::SeqCst);
    if hwnd == 0 {
        return;
    }
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
        };

        ShowWindow(hwnd, SW_RESTORE);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}

#[cfg(not(target_os = "windows"))]
fn restore_native_window(_hwnd: &Arc<AtomicIsize>) {}

#[cfg(target_os = "windows")]
fn open_path(path: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("explorer")
        .arg(path.as_os_str())
        .spawn()
        .map(|_| ())
}

#[cfg(not(target_os = "windows"))]
fn open_path(path: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
}

/// Cherche un dossier `models/` qui contient au moins un .bin.
/// Ordre de priorité :
/// 1. Dossier utilisateur `%LOCALAPPDATA%\NyxWhisper\models\` (le plus stable, écriture libre)
/// 2. Dossier courant `./models`
/// 3. À côté de l'exe (déploiement portable)
/// 4. 1-4 niveaux au-dessus de l'exe (cas dev : target\release\)
///
/// Si aucun ne contient de .bin, retourne `%LOCALAPPDATA%\NyxWhisper\models\`
/// (qui sera créé au premier téléchargement).
fn locate_models_dir() -> PathBuf {
    let user_dir = crate::config::Config::user_models_dir();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| cwd.clone());

    let mut candidates: Vec<PathBuf> =
        vec![user_dir.clone(), cwd.join("models"), exe_dir.join("models")];
    let mut up = exe_dir.clone();
    for _ in 0..4 {
        if let Some(parent) = up.parent() {
            up = parent.to_path_buf();
            candidates.push(up.join("models"));
        } else {
            break;
        }
    }

    for c in &candidates {
        if let Ok(rd) = std::fs::read_dir(c) {
            for entry in rd.flatten() {
                if entry
                    .path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("bin"))
                    .unwrap_or(false)
                {
                    return c.clone();
                }
            }
        }
    }

    user_dir
}
