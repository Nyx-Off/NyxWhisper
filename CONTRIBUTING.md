# Contributing

Merci de garder les changements petits, testables et alignes avec l'architecture existante.

## Environnement

Prerequis Windows :

- Rust stable
- Visual Studio Build Tools 2022 avec C++
- CMake
- LLVM (`libclang.dll`)

Installation rapide :

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\setup.ps1
```

## Avant d'ouvrir une PR

Executer depuis la racine :

```powershell
cargo fmt
cargo check --bin NyxWhisper
powershell -ExecutionPolicy Bypass -File .\scripts\build-cpu.ps1
```

Pour les changements UI/tray/hotkey, tester manuellement :

- demarrage et arret de la dictee
- raccourci global quand la fenetre est ouverte et minimisee
- menu tray : ouvrir, demarrer/arreter, quitter
- notifications Windows
- chargement/changement de modele
- modes de sortie clavier et presse-papiers

## Style

- Garder la logique UI dans `src/app.rs`.
- Garder audio/transcription/sortie dans leurs modules dedies.
- Ne pas commiter de modeles Whisper, DLL CUDA copiees, logs, installeurs ou dossiers `target*`.
- Ne pas modifier `assets/icon.ico` a la main : `src/icon.rs` est la source de verite.
- Utiliser `cargo fmt` avant toute PR.

## Pull requests

Inclure :

- resume court
- commandes testees
- backend impacte : CPU, CUDA, Vulkan, installer
- captures ou notes pour les changements UI/tray
- limitations connues
