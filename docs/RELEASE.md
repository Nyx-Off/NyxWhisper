# Release Checklist

Cette checklist prepare une release Windows manuelle.

## Verification

```powershell
cargo fmt --check
cargo check --bin NyxWhisper
powershell -ExecutionPolicy Bypass -File .\scripts\build-cpu.ps1
```

Tester manuellement :

- telechargement ou selection d'un modele
- start/stop avec bouton principal
- start/stop avec raccourci global
- raccourci global avec fenetre minimisee
- menu tray : ouvrir, demarrer/arreter, quitter
- notifications Windows
- mode saisie clavier
- mode presse-papiers

## Installeur

```powershell
powershell -ExecutionPolicy Bypass -File .\installer\build-all.ps1
```

Verifier la sortie :

```text
installer\out\NyxWhisper-Setup-0.1.0.exe
```

Ne pas commiter :

- `dist-*`
- `target*`
- `installer/out`
- `models/*.bin`
- DLL CUDA copiees
- logs locaux

## Notes de release

Inclure :

- version
- backend inclus : CPU/CUDA
- commandes de verification
- limites connues
- hash ou taille de l'installeur si publication binaire
