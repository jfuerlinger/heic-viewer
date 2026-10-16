# HEIC Explorer Preview

Thumbnails for HEIC/HEIF photos (`.heic`, `.heif`, `.hif`) in Windows Explorer – Windows 10/11 x64.

[Deutsch](#deutsch) · [English](#english)

---

## Deutsch

### Was macht das?

Eine Explorer-Erweiterung (Thumbnail-Handler), die für HEIC/HEIF-Fotos, z. B. vom iPhone,
Vorschaubilder in der Explorer-Ansicht erzeugt:

- nutzt das in der Datei eingebettete Vorschaubild, wenn es groß genug ist, sonst wird das
  Hauptbild decodiert und verkleinert
- Drehung/Spiegelung (HEIF `irot`/`imir`, so speichert das iPhone die Ausrichtung) wird angewendet
- Farbprofile (z. B. Display P3) werden nach sRGB umgerechnet, 10-Bit/HDR auf 8 Bit
- Transparenz wird unterstützt
- Schutz vor defekten oder riesigen Dateien: Limit 100 Megapixel, 20 s Timeout, Fehler
  führen nur zu einem fehlenden Vorschaubild, nie zu einem Absturz des Explorers

Nicht enthalten (v1): Vorschaufenster (Alt+P), AVIF, Bildserien/Live Photos (nur das
Hauptbild wird verwendet), Gain-Maps/Tiefenkarten.

### Installation

1. `HEICExplorerPreview-Setup.exe` von der Releases-Seite herunterladen und starten.
2. **SmartScreen-Hinweis:** Der Installer ist nicht code-signiert. Windows zeigt daher
   „Der Computer wurde durch Windows geschützt“. Auf **„Weitere Informationen“** klicken und
   dann **„Trotzdem ausführen“**.
3. Installationsart wählen:
   - **Nur für mich** (Standard, keine Administratorrechte, Registrierung unter `HKCU`)
   - **Für alle Benutzer** (fragt nach Administratorrechten, Registrierung unter `HKLM`)
4. Lizenz bestätigen, installieren. Explorer-Fenster ggf. neu öffnen.

Ist bereits ein anderer Thumbnail-Handler für diese Endungen registriert (z. B. von der
Microsoft „HEIF-Bilderweiterungen“), wird er überschrieben. Der alte Eintrag wird gesichert
und bei der Deinstallation wiederhergestellt.

**Unbeaufsichtigte Installation** (Inno Setup, `/S` gibt es nicht – stattdessen):

```bat
HEICExplorerPreview-Setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER
HEICExplorerPreview-Setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /ALLUSERS   (aus einer Admin-Konsole)
```

Updates: Es gibt keine automatischen Updates. Neue Version = neuen Installer ausführen, die
alte Version wird dabei ersetzt.

### Vorschaubilder erscheinen nicht / alte Vorschaubilder bleiben

Windows speichert Vorschaubilder in einem Cache. Bereits gecachte (oder als „kein
Vorschaubild“ gemerkte) Dateien zeigen nach der Installation evtl. noch das alte Bild.

- **Einfach:** Datenträgerbereinigung (`cleanmgr`) → Laufwerk C: → nur **„Miniaturansichten“**
  anhaken → OK.
- **Manuell** (Eingabeaufforderung):

  ```bat
  taskkill /f /im explorer.exe
  del /f /a "%LocalAppData%\Microsoft\Windows\Explorer\thumbcache_*.db"
  start explorer.exe
  ```

Außerdem in den Ordneroptionen prüfen, dass „Immer Symbole statt Miniaturansichten anzeigen“
**nicht** aktiviert ist.

### Deinstallation

„Einstellungen → Apps → Installierte Apps → HEIC Explorer Preview → Deinstallieren“
oder still: `"%LocalAppData%\Programs\HEIC Explorer Preview\unins000.exe" /VERYSILENT`
(bei Installation für alle Benutzer liegt er unter `C:\Program Files\HEIC Explorer Preview`).

Der vorher registrierte Thumbnail-Handler wird wiederhergestellt. Falls Explorer die DLL
gerade geladen hat, kann im Installationsordner eine `*.old`-Datei übrig bleiben; sie wird
bei der nächsten Installation oder nach Ab-/Anmelden manuell löschbar.

### Rechtliches

- Eigener Code: **MIT-Lizenz** (`LICENSE`).
- **libheif** und **libde265** werden als separate DLLs (`heif.dll`, `libde265.dll`)
  mitgeliefert und stehen unter der **LGPL-3.0**. Die Lizenztexte werden im Installer
  angezeigt und in den Ordner `licenses` installiert; Details und Quellcode-Links in
  `THIRD-PARTY-LICENSES.txt`. Jede Release enthält zusätzlich `third-party-sources.zip`
  mit den exakt verwendeten Quellen.
- **HEVC-Patente:** HEIC nutzt HEVC (H.265), das in einigen Ländern patentrechtlich geschützt
  ist. Mit dieser Software wird keine Patentlizenz erteilt; Nutzer sind selbst dafür
  verantwortlich, ggf. nötige HEVC-Lizenzen zu beschaffen.

---

## English

### What it does

An Explorer extension (thumbnail handler) that renders thumbnails for HEIC/HEIF photos:

- uses the embedded thumbnail when it is large enough, otherwise decodes and scales the
  primary image
- applies rotation/mirroring (HEIF `irot`/`imir`, which is how iPhones store orientation)
- converts color profiles (e.g. Display P3) to sRGB, 10-bit/HDR down to 8-bit
- supports transparency
- hardened against broken/huge files: 100 megapixel limit, 20 s timeout; failures only mean
  "no thumbnail", never an Explorer crash

Not in v1: preview pane (Alt+P), AVIF, bursts/Live Photos (primary image only), gain maps,
depth maps.

### Installation

1. Download `HEICExplorerPreview-Setup.exe` from the releases page and run it.
2. **SmartScreen:** the installer is not code-signed, so Windows shows "Windows protected
   your PC". Click **"More info"**, then **"Run anyway"**.
3. Choose **"Install for me only"** (default, no admin, `HKCU`) or **"Install for all users"**
   (asks for admin rights, `HKLM`).
4. Accept the license and install. Reopen Explorer windows if needed.

If another thumbnail handler is already registered for these extensions (e.g. Microsoft's
"HEIF Image Extensions"), it is overridden; the previous entry is backed up and restored on
uninstall.

**Silent install** (Inno Setup does not support `/S`; use):

```bat
HEICExplorerPreview-Setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER
HEICExplorerPreview-Setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /ALLUSERS   (from an elevated prompt)
```

There are no automatic updates: run the newer installer to upgrade.

### Thumbnails do not show up / old thumbnails stay

Windows caches thumbnails. Clear the cache with Disk Cleanup (`cleanmgr`, tick only
**"Thumbnails"**) or manually:

```bat
taskkill /f /im explorer.exe
del /f /a "%LocalAppData%\Microsoft\Windows\Explorer\thumbcache_*.db"
start explorer.exe
```

Also make sure "Always show icons, never thumbnails" is **off** in Folder Options.

### Uninstall

Settings → Apps → Installed apps → HEIC Explorer Preview → Uninstall, or silently:
`"%LocalAppData%\Programs\HEIC Explorer Preview\unins000.exe" /VERYSILENT`
(`C:\Program Files\HEIC Explorer Preview` for all-users installs). The previously registered
handler is restored. If Explorer still had the DLL loaded, a `*.old` file may remain in the
install folder until the next install or until you sign out and delete it.

### Legal

- Own code: **MIT** (`LICENSE`).
- **libheif** and **libde265** are shipped as separate DLLs under the **LGPL-3.0**. The
  license texts are shown by the installer and installed into `licenses`; see
  `THIRD-PARTY-LICENSES.txt` for details and source links. Every release also attaches
  `third-party-sources.zip` with the exact sources used.
- **HEVC patents:** HEIC uses HEVC (H.265), which is patent-encumbered in some countries. No
  patent license is granted; users are responsible for obtaining any required HEVC license.

---

## Development

### Layout

```
crates/heic-decoder     platform-independent core: bytes in -> 8-bit sRGB RGBA out
                        (libheif FFI, thumbnail selection, scaling, ICC/nclx -> sRGB, limits, timeout)
crates/heic-thumbnail   Windows COM DLL (IThumbnailProvider + IInitializeWithStream), cdylib
crates/thumbnail-probe  Windows diagnostic tool: fetch a thumbnail via COM or via the shell
installer/              Inno Setup script (German + English)
scripts/                CI helpers: stage dist, verify DLLs, installer smoke test
testdata/               small HEIC fixtures (see testdata/README.md)
docs/                   manual test checklist
```

`heic-decoder` knows nothing about Windows, so a later preview handler (Alt+P) can reuse
`heic_decoder::decode_image` / `decode_thumbnail`.

### Why vcpkg for libheif/libde265

- vcpkg builds libheif/libde265 from the upstream release tags with MSVC, reproducibly pinned
  via `builtin-baseline` + `overrides` in `vcpkg.json` (libheif 1.23.5, libde265 1.1.3).
- `default-features: false` matters: the vcpkg default feature `hevc` adds the **x265
  encoder (GPL)**. Without it libheif only gets HEVC decoding via libde265.
- A custom triplet (`triplets/x64-windows-staticcrt.cmake`) builds DLLs (LGPL-friendly) with
  the static C runtime, so nothing depends on the VC++ Redistributable.
- Prebuilt third-party binaries were rejected: no trustworthy, versioned MSVC builds of
  libheif exist, and we would not control their features/licensing.

`heif.dll` is **delay-loaded** and preloaded by full path from the DLL's own folder
(`crates/heic-thumbnail/src/loader.rs`), because Explorer hosts thumbnail handlers in
`dllhost.exe`, whose DLL search path does not include our install folder.

### Building locally

Windows (Visual Studio Build Tools, Rust, a vcpkg checkout):

```powershell
vcpkg install --triplet x64-windows-staticcrt --overlay-triplets=triplets
$env:HEIF_LIB_DIR = "$PWD\vcpkg_installed\x64-windows-staticcrt\lib"
$env:PATH = "$PWD\vcpkg_installed\x64-windows-staticcrt\bin;$env:PATH"
cargo test --workspace
cargo build --release
./scripts/stage-dist.ps1
iscc /DDistDir=$PWD\dist installer\heic-explorer-preview.iss
```

macOS/Linux (decoder core only, needs libheif ≥ 1.17):

```sh
HEIF_LIB_DIR=/path/to/libheif/lib cargo test -p heic-decoder
cargo check --target x86_64-pc-windows-msvc --workspace --all-targets   # type-check the Windows code
```

Diagnostics on Windows: `thumbnail-probe.exe <file.heic> [size] [--bmp out.bmp]` (direct COM
activation) or `thumbnail-probe.exe --shell <file.heic>` (through the shell like Explorer).
The DLL writes errors via `OutputDebugString` (visible in Sysinternals DebugView).

CI (`.github/workflows/build.yml`, `windows-latest`): vcpkg build → fmt/clippy → tests →
release build → DLL checks (exports, delay-load, static CRT, no x265) → Inno Setup →
silent install/uninstall smoke test → artifact. Tags `v*` create a GitHub release with
`HEICExplorerPreview-Setup.exe` and `third-party-sources.zip`.
