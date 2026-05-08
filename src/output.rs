use anyhow::{anyhow, Result};
use enigo::{Enigo, Keyboard, Settings};

/// Tape `text` dans la fenêtre active (saisie clavier simulée, supporte l'Unicode).
pub fn type_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| anyhow!("Initialisation enigo : {:?}", e))?;
    enigo
        .text(text)
        .map_err(|e| anyhow!("Écriture du texte : {:?}", e))?;
    Ok(())
}

/// Copie `text` dans le presse-papiers.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| anyhow!("Ouverture du presse-papiers : {}", e))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| anyhow!("Écriture dans le presse-papiers : {}", e))?;
    Ok(())
}
