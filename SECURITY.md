# Security Policy

NyxWhisper traite de l'audio utilisateur et des transcriptions locales. Aucun audio n'est volontairement envoye vers un service externe par l'application.

## Signaler une vulnerabilite

Quand le depot GitHub sera cree, utiliser les GitHub Security Advisories si elles sont activees. Sinon, contacter le mainteneur du depot par le canal indique sur la page du projet.

Ne pas ouvrir d'issue publique pour une faille exploitable avant coordination.

## Donnees sensibles

Ne jamais joindre dans une issue publique :

- fichiers audio personnels
- transcriptions privees
- fichiers de configuration contenant des chemins ou informations sensibles
- logs complets sans relecture

Les logs applicatifs sont situes dans :

```text
%APPDATA%\NyxWhisper\NyxWhisper.log
```

Avant partage, supprimer toute information personnelle.
