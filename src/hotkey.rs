use anyhow::{anyhow, Result};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

/// Parse une combinaison du type "Control+Alt+Space" en HotKey.
pub fn parse(combo: &str) -> Result<HotKey> {
    let mut mods = Modifiers::empty();
    let mut key: Option<Code> = None;

    for part in combo.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            "super" | "win" | "meta" => mods |= Modifiers::SUPER,
            _ => {
                use std::str::FromStr;
                key =
                    Some(Code::from_str(part).map_err(|_| {
                        anyhow!("Touche inconnue : '{}' (ex: Space, F9, KeyD)", part)
                    })?);
            }
        }
    }

    let key = key.ok_or_else(|| anyhow!("Aucune touche principale dans : '{}'", combo))?;
    Ok(HotKey::new(
        if mods.is_empty() { None } else { Some(mods) },
        key,
    ))
}

pub fn human_label(combo: &str) -> String {
    combo.replace('+', " + ")
}
