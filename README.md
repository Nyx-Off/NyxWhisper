# NyxWhisper

NyxWhisper est une application Windows de dictee vocale locale en Rust. Elle capture le micro, transcrit avec Whisper via `whisper-rs`/`whisper.cpp`, puis tape le texte dans la fenetre active ou le copie dans le presse-papiers.

Tout tourne en local : aucun audio n'est envoye vers un service externe.

## Fonctionnalites

- Interface native Windows avec `eframe`/`egui`
- Raccourci clavier global configurable, par defaut `Control+Alt+Space`
- Capture audio locale avec selection du microphone
- Transcription Whisper en francais par defaut
- Mode saisie clavier ou presse-papiers
- Mini-overlay live pendant la dictee
- Icone tray avec ouvrir, demarrer/arreter et quitter
- Notifications Windows start/stop/resultat
- Telechargement de modeles depuis l'application
- Builds CPU et CUDA, avec pipeline d'installeur Inno Setup

## Etat du projet

Le projet est utilisable, mais encore jeune. Il n'y a pas encore de suite de tests automatises complete. Avant une release publique, verifier manuellement les workflows critiques : modele charge, raccourci global, overlay, tray, notifications, fermeture/reouverture et mode CPU/CUDA.

## Prerequis developpement

Windows 10/11 x64 est la cible principale.

Outils requis :

- Rust stable (`rustup`)
- Visual Studio Build Tools 2022 avec workload `Desktop development with C++`
- CMake
- LLVM, pour `libclang.dll`
- Inno Setup 6, uniquement pour compiler l'installeur
- CUDA Toolkit, uniquement pour le build NVIDIA CUDA
- Vulkan SDK, uniquement pour le build Vulkan experimental

Le script suivant installe ou verifie les prerequis principaux via `winget` :

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\setup.ps1
```

Fermer puis rouvrir le terminal apres ce script, afin que `PATH` et `LIBCLANG_PATH` soient pris en compte.

## Modeles Whisper

Les modeles ne sont pas versionnes dans Git. Ils doivent etre telecharges au runtime depuis l'application, ou manuellement avec :

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\download-model.ps1 -Model small
```

Modeles acceptes par le script : `tiny`, `base`, `small`, `medium`, `large-v3`.

Pour la dictee francaise :

| Modele | Taille approx. | Usage |
| --- | ---: | --- |
| `small` | 465 Mo | Bon compromis rapidite/qualite |
| `medium` | 1.5 Go | Meilleure qualite, plus lent |
| `large-v3` | 3 Go | Meilleure precision, tres lent hors GPU |

En build installe, les modeles utilisateur doivent vivre dans :

```text
%LOCALAPPDATA%\NyxWhisper\models\
```

## Build developpement

Depuis la racine du depot :

```powershell
cargo check --bin NyxWhisper
cargo fmt
```

Build CPU release :

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-cpu.ps1
```

Build CUDA release :

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-cuda.ps1
```

Build Vulkan release :

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-vulkan.ps1
```

L'executable de developpement est genere ici :

```text
target\release\NyxWhisper.exe
```

## Installeur Windows

Le pipeline complet construit les variantes CUDA et CPU, puis cree l'installeur :

```powershell
powershell -ExecutionPolicy Bypass -File .\installer\build-all.ps1
```

Sortie :

```text
installer\out\NyxWhisper-Setup-0.1.0.exe
```

Les dossiers `dist-*`, `target*`, `installer/out` et les DLL CUDA copiees sont des artefacts generes et ne doivent pas etre commites.

## Utilisation

1. Lancer `NyxWhisper.exe`.
2. Telecharger ou selectionner un modele Whisper.
3. Verifier le raccourci global dans les reglages.
4. Placer le curseur dans l'application cible.
5. Appuyer sur le raccourci pour demarrer la dictee.
6. Appuyer de nouveau pour arreter et transcrire.

Si le mode "Saisie clavier" ne marche pas dans une application protegee ou lancee en admin, utiliser le mode "Presse-papiers" puis coller avec `Ctrl+V`.

## Donnees utilisateur

Configuration :

```text
%APPDATA%\NyxWhisper\config.toml
```

Logs :

```text
%APPDATA%\NyxWhisper\NyxWhisper.log
```

Modeles installes :

```text
%LOCALAPPDATA%\NyxWhisper\models\
```

## Structure du depot

```text
.
├── src/
│   ├── main.rs          # Demarrage eframe/egui
│   ├── app.rs           # UI, hotkey, tray, overlay, modeles
│   ├── worker.rs        # Capture/transcription hors thread UI
│   ├── audio.rs         # Capture micro via cpal
│   ├── transcribe.rs    # Integration whisper-rs
│   ├── output.rs        # Saisie clavier et presse-papiers
│   ├── config.rs        # Config TOML utilisateur
│   ├── tray.rs          # Icone tray et notifications
│   └── icon.rs          # Source de verite de l'icone generee
├── scripts/             # Setup, builds CPU/CUDA/Vulkan, modeles
├── installer/           # Inno Setup et pipeline release
├── models/              # Dossier local ignore pour modeles Whisper
├── Cargo.toml
├── Cargo.lock
└── README.md
```

## Contribution

Voir [CONTRIBUTING.md](CONTRIBUTING.md).

## Securite

Voir [SECURITY.md](SECURITY.md).

## Licence

MIT. Voir [LICENSE](LICENSE).
