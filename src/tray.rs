use anyhow::{anyhow, Result};
use tray_icon::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

/// Identifiants des items de menu (string utilisée par tray-icon).
pub mod ids {
    pub const SHOW: &str = "show";
    pub const TOGGLE_REC: &str = "toggle_rec";
    pub const QUIT: &str = "quit";
}

pub struct AppTray {
    _icon: TrayIcon,
    _item_show: MenuItem,
    _item_toggle: MenuItem,
    _item_quit: MenuItem,
}

impl AppTray {
    pub fn new() -> Result<Self> {
        let icon = make_icon()?;

        let menu = Menu::new();
        let item_show = MenuItem::with_id(ids::SHOW, "Ouvrir NyxWhisper", true, None);
        let item_toggle =
            MenuItem::with_id(ids::TOGGLE_REC, "Démarrer / Arrêter la dictée", true, None);
        let item_quit = MenuItem::with_id(ids::QUIT, "Quitter", true, None);
        menu.append(&item_show)
            .map_err(|e| anyhow!("tray menu append: {}", e))?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| anyhow!("tray menu append: {}", e))?;
        menu.append(&item_toggle)
            .map_err(|e| anyhow!("tray menu append: {}", e))?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| anyhow!("tray menu append: {}", e))?;
        menu.append(&item_quit)
            .map_err(|e| anyhow!("tray menu append: {}", e))?;

        let tray = TrayIconBuilder::new()
            .with_tooltip("NyxWhisper — Dictée vocale")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .build()
            .map_err(|e| anyhow!("tray build: {}", e))?;

        Ok(Self {
            _icon: tray,
            _item_show: item_show,
            _item_toggle: item_toggle,
            _item_quit: item_quit,
        })
    }
}

/// Charge l'icône NyxWhisper (N grunge sur fond noir) en 64x64 RGBA.
fn make_icon() -> Result<Icon> {
    const SIZE: u32 = 64;
    let rgba = crate::icon::n_grunge_rgba(SIZE);
    Icon::from_rgba(rgba, SIZE, SIZE).map_err(|e| anyhow!("Icon::from_rgba: {}", e))
}

/// Notification Windows discrète (toast). Best-effort, avec log si Windows la refuse.
pub fn notify(title: &str, body: &str) {
    if let Err(e) = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .appname("NyxWhisper")
        .timeout(notify_rust::Timeout::Milliseconds(2500))
        .show()
    {
        log::warn!("Notification Windows indisponible : {}", e);
    }
}
