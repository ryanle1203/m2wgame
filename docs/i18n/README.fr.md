# M2WGame

[English](../../README.md) · [Tiếng Việt](README.vi.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Português (BR)](README.pt-BR.md) · **Français** · [Deutsch](README.de.md)

M2W = Mac vers Windows (jeux). Une application pour **faire tourner des jeux Windows sur les Mac Apple Silicon** : elle gère des « bouteilles » Wine (environnements Windows indépendants), des moteurs Wine, Steam pour Windows et votre bibliothèque de jeux, et explique les problèmes en langage clair au lieu de vous laisser face à un mur de journaux.

> **État : alpha précoce.** Cela fonctionne avec certains jeux et pas avec d'autres. Lisez les « Limites » avant de perdre du temps avec un jeu. L'interface de l'application est pour l'instant **uniquement en vietnamien** ; les traductions sont les bienvenues. Le README anglais fait référence, les traductions peuvent avoir du retard.

M2WGame **n'est pas affilié** à Apple, Valve, CodeWeavers ni au projet Wine. Il n'inclut, ne télécharge et ne contourne aucun jeu, DRM ou vérification de licence : vous devez posséder vos jeux.

## Ce qu'il fait

- **Bibliothèque de jeux** : Jouer / Arrêter, réglages par jeu, journaux et indices en langage clair sur la cause d'un échec.
- **Environnements et moteurs** : créez des environnements, choisissez le moteur Wine de chacun et installez Steam pour Windows en un clic.
- **Steam sans la fenêtre de Steam** (elle reste noire sous Wine sur macOS) : ajoutez en un clic les jeux déjà présents dans la bibliothèque Steam ; téléchargez de nouveaux jeux avec SteamCMD dans l'application (le mot de passe et le code Steam Guard vont directement à SteamCMD et ne sont jamais conservés) ; affichez dans l'application les boîtes de dialogue cachées de Steam (EULA, synchronisation cloud) pour y répondre vous-même ; avertissement pour les jeux 32 bits ; désinstallation pour libérer de l'espace disque.
- **Vérification du système et du réseau** : Rosetta, version de macOS, RAM, espace disque, et accessibilité des serveurs Steam.

## Prérequis

Mac Apple Silicon, macOS 14 ou plus récent, Rosetta 2, Xcode Command Line Tools et [Rust](https://rustup.rs). Vous devez fournir votre propre moteur Wine (non inclus) ; voir « Get an engine » dans le README anglais.

## Compilation

```bash
git clone https://github.com/ryanle1203/m2wgame
cd m2wgame
./scripts/build-swift.sh
open target/M2WGame.app
```

## Limites

- **Les jeux Windows 32 bits ne fonctionnent généralement pas** (plantage au démarrage) ; M2WGame vous en avertit.
- L'interface de Steam est inutilisable sous Wine sur macOS ; le projet la contourne comme décrit ci-dessus.
- Les jeux DirectX 12 ou avec anti-triche sont pour la plupart hors de portée.
- Testé uniquement sur macOS 27 avec un M1 Pro ; les retours d'autres Mac sont très appréciés.

## Contribuer et licence

Lisez [CONTRIBUTING.md](../../CONTRIBUTING.md) (avec des règles strictes sur les mots de passe et les DRM) et [SECURITY.md](../../SECURITY.md). Licence : [MIT](../../LICENSE).
