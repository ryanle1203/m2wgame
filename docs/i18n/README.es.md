# M2WGame

[English](../../README.md) · [Tiếng Việt](README.vi.md) · [简体中文](README.zh-CN.md) · **Español** · [日本語](README.ja.md) · [한국어](README.ko.md) · [Português (BR)](README.pt-BR.md) · [Français](README.fr.md) · [Deutsch](README.de.md)

M2W = Mac a Windows (juegos). Una app para **ejecutar juegos de Windows en Macs con Apple Silicon**: gestiona «botellas» de Wine (entornos de Windows independientes), motores Wine, Steam para Windows y tu biblioteca de juegos, y explica los problemas en lenguaje sencillo en lugar de dejarte con una pared de registros.

> **Estado: alfa temprana.** Funciona con algunos juegos y con otros no. Lee las «Limitaciones» antes de perder tiempo con un juego. La interfaz de la app está por ahora **solo en vietnamita**; las traducciones son bienvenidas. El README en inglés es la referencia y las traducciones pueden ir con retraso.

> **Alcance:** M2WGame se centra en un único problema: ejecutar **juegos de Windows comprados en Steam**. Otras tiendas y lanzadores (Epic, GOG, EA, Ubisoft, Battle.net, itch.io…) aún no son compatibles. Puedes añadir a mano un `.exe` suelto, pero esa vía no está probada.

M2WGame **no está afiliado** a Apple, Valve, CodeWeavers ni al proyecto Wine. No incluye, descarga ni evade ningún juego, DRM o comprobación de licencia: debes ser propietario de tus juegos.

## Qué hace

- **Biblioteca de juegos**: Jugar / Detener, ajustes por juego, registros y pistas en lenguaje sencillo sobre qué falló.
- **Entornos y motores**: crea entornos, elige el motor Wine de cada uno e instala Steam para Windows con un clic.
- **Steam sin la ventana de Steam** (se ve negra con Wine en macOS): añade con un clic los juegos que ya están en la biblioteca de Steam; descarga juegos nuevos con SteamCMD dentro de la app (la contraseña y el código de Steam Guard van directos a SteamCMD y nunca se guardan); muestra dentro de la app los diálogos ocultos de Steam (EULA, sincronización en la nube) para que los respondas tú; avisa de los juegos de 32 bits; desinstala juegos para liberar espacio.
- **Comprobación del sistema y de la red**: Rosetta, versión de macOS, RAM, espacio en disco y si los servidores de Steam son accesibles.

## Requisitos

Mac con Apple Silicon, macOS 14 o posterior, Rosetta 2, Xcode Command Line Tools y [Rust](https://rustup.rs). Debes aportar tu propio motor Wine (no se incluye); consulta «Get an engine» en el README en inglés.

## Descarga

Descarga el `.dmg` desde [Releases](https://github.com/ryanle1203/m2wgame/releases) y arrástralo a Aplicaciones. La app no está notarizada por Apple: la primera vez, clic derecho > Abrir (detalles en la sección «Install» del README en inglés).

## Compilar

```bash
git clone https://github.com/ryanle1203/m2wgame
cd m2wgame
./scripts/build-swift.sh
open target/M2WGame.app
```

## Limitaciones

- **Los juegos de Windows de 32 bits suelen no funcionar** (se cierran al iniciar); M2WGame te avisa.
- La interfaz de Steam no es utilizable con Wine en macOS; el proyecto la rodea como se describe arriba.
- Los juegos con DirectX 12 o antitrampas quedan casi siempre fuera de alcance.
- Solo probado en macOS 27 con un M1 Pro; se agradecen informes de otros Macs.

## Contribuir y licencia

Lee [CONTRIBUTING.md](../../CONTRIBUTING.md) (incluye reglas firmes sobre contraseñas y DRM) y [SECURITY.md](../../SECURITY.md). Licencia: [MIT](../../LICENSE).
