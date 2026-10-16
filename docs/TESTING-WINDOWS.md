# Handtest-Checkliste – Windows 11 x64

Erster manueller Test von HEIC Explorer Preview auf echter Hardware oder in einer VM.
Empfehlung: Windows-11-VM mit Snapshot vor Schritt 1, damit Installationen wiederholbar sind.

Legende: ☐ offen · ✅ ok · ❌ Fehler (bitte Notiz + Screenshot/Log)

## 0. Vorbereitung

- ☐ Installer aus dem CI-Artifact (`HEICExplorerPreview-Setup-<version>`) bzw. der Release laden.
- ☐ Zusätzlich das CI-Build-Verzeichnis oder ein lokales `cargo build --release -p thumbnail-probe`
  für `thumbnail-probe.exe` (optional, Diagnose).
- ☐ Testordner `C:\heic-test` anlegen mit:
  - allen Dateien aus `testdata\` dieses Repos
  - mehreren echten iPhone-Fotos: Querformat, **Hochformat**, Selfie, Foto mit vielen roten/
    grünen Farbtönen (P3), ein **HDR**-Foto (neueres iPhone), eine Live-Photo-HEIC, ein Burst-Bild
  - wenn vorhanden: `.hif` von einer Canon/Sony-Kamera, ein großes Foto (48 MP, iPhone 15 Pro+)
  - eine 0-Byte-Datei `leer.heic` und eine umbenannte Textdatei `fake.heic`
  - einen Ordner mit ≥ 200 HEIC-Fotos (Performance)
- ☐ Ausgangszustand notieren (Eingabeaufforderung):

  ```bat
  reg query "HKCU\Software\Classes\.heic\ShellEx\{e357fccd-a995-4576-b01f-234630154e96}"
  reg query "HKLM\Software\Classes\.heic\ShellEx\{e357fccd-a995-4576-b01f-234630154e96}"
  reg query "HKCR\SystemFileAssociations\.heic\ShellEx\{e357fccd-a995-4576-b01f-234630154e96}"
  ```

  Notiz: Ist die Microsoft „HEIF-Bilderweiterungen“ installiert? ☐ ja ☐ nein
  Werden HEIC-Vorschaubilder **vor** der Installation angezeigt? ☐ ja ☐ nein

## 1. Installation (pro Benutzer, GUI)

- ☐ Doppelklick auf den Installer → SmartScreen-Meldung erscheint → „Weitere Informationen“ →
  „Trotzdem ausführen“ funktioniert.
- ☐ Sprachauswahl Deutsch/Englisch angeboten; Texte in der gewählten Sprache.
- ☐ Dialog „Installationsmodus“: „Nur für mich“ ist vorausgewählt, **kein** UAC-Prompt.
- ☐ Lizenzseite zeigt: deutschen/englischen Hinweis, MIT-Lizenz, THIRD-PARTY-Liste, LGPL-3-Volltext,
  HEVC-Patenthinweis.
- ☐ Zielordner Standard: `%LocalAppData%\Programs\HEIC Explorer Preview`.
- ☐ Installation läuft ohne Fehlermeldung durch; kein Neustart verlangt.
- ☐ Ordner enthält `heic_thumbnail.dll`, `heif.dll`, `libde265.dll`, `LICENSE.txt`,
  `THIRD-PARTY-LICENSES.txt`, `licenses\…`, `unins000.exe`.
- ☐ Registry:

  ```bat
  reg query "HKCU\Software\Classes\CLSID\{218C7AE3-FDC4-4381-ABCD-D7385AFB7A4A}\InprocServer32"
  reg query "HKCU\Software\Classes\.heic\ShellEx\{e357fccd-a995-4576-b01f-234630154e96}"
  reg query "HKCU\Software\Classes\.heif\ShellEx\{e357fccd-a995-4576-b01f-234630154e96}"
  reg query "HKCU\Software\Classes\.hif\ShellEx\{e357fccd-a995-4576-b01f-234630154e96}"
  ```

  → Pfad zur DLL und `ThreadingModel = Apartment`; alle drei Endungen zeigen auf `{218C7AE3-…}`.

## 2. Vorschaubilder im Explorer

Falls alte Symbole bleiben: Thumbnail-Cache leeren (siehe README) und Explorer neu starten.

- ☐ `C:\heic-test` in Ansicht **Extra große Symbole**: alle gültigen Dateien zeigen ein Bild.
- ☐ Ansichten **Große Symbole**, **Mittelgroße Symbole**, **Kacheln**, **Inhalt**: Bilder scharf,
  nicht verzerrt.
- ☐ Details-Bereich (Alt+Umschalt+P) zeigt das Vorschaubild der markierten Datei.
- ☐ `quadrants_srgb.heic`: links oben rot, rechts oben grün, links unten blau, rechts unten weiß.
- ☐ `with_thumbnail.heic`: gleiches Muster, korrektes Seitenverhältnis 4:3.
- ☐ `.heif`- und `.hif`-Dateien zeigen ebenfalls Vorschaubilder.
- ☐ Live Photo / Burst: es erscheint das Hauptbild, kein Fehler.

## 3. Hochformat / Rotation

- ☐ Echte iPhone-Hochformat-Fotos stehen **aufrecht** (nicht um 90° gedreht).
- ☐ `quadrants_orientation6.heic` ist hochkant (64x96): oben links **blau**, oben rechts **rot**,
  unten links weiß, unten rechts grün.
- ☐ `with_thumbnail_orientation6.heic`: ebenso hochkant mit Blau oben links.
- ☐ Selfie (Frontkamera, ggf. gespiegelt) wirkt wie in der Fotos-App.

## 4. Farben (Display P3, 10 Bit, HDR)

- ☐ P3-Fotos (kräftiges Rot/Grün) wirken im Explorer ähnlich wie in der Windows-Fotos-App bzw.
  auf dem iPhone, nicht blass/entsättigt.
- ☐ `p3_color.heic`: Mittelpunktfarbe ≈ `#DE1A29` (nicht `#CC3333`):
  `thumbnail-probe.exe C:\heic-test\p3_color.heic 256` → `center=#DE1A29` (±3 je Kanal).
- ☐ `tenbit_p3.heic` zeigt das Vierfarbmuster sauber (keine Streifen/Falschfarben).
- ☐ HDR-iPhone-Foto: normal belichtet, nicht ausgebrannt/zu dunkel.

## 5. Transparenz

- ☐ `alpha.heic` auf hellem und dunklem Explorer-Design: linke Hälfte rot, rechte Hälfte
  halbtransparent blau (Hintergrund scheint durch), keine schwarzen/weißen Ränder.

## 6. Defekte Dateien / Robustheit

- ☐ `corrupt_truncated.heic`, `leer.heic`, `fake.heic`: Standardsymbol, **kein** Absturz, kein
  Hängen des Explorers.
- ☐ Ordner mit ≥ 200 Fotos: zügiges Scrollen; Explorer bleibt bedienbar, Vorschaubilder kommen
  nach und nach. Task-Manager: kein dauerhaft hoher CPU/RAM-Verbrauch durch `dllhost.exe`.
- ☐ 48-MP-Foto: Vorschaubild erscheint (ggf. nach ~1–2 s).
- ☐ Optional: Sysinternals **DebugView** starten → bei defekten Dateien erscheinen Zeilen
  `[HEIC Explorer Preview] decode failed: …`.

## 7. Update / erneute Installation

- ☐ Während Explorer HEIC-Vorschaubilder zeigt, den Installer erneut ausführen → läuft ohne
  „Datei in Verwendung“-Fehler durch. Im Installationsordner darf eine `*.old`-Datei auftauchen.
- ☐ Vorschaubilder funktionieren danach weiterhin.

## 8. Deinstallation inkl. Wiederherstellung des alten Handlers

Vorbereitung (falls kein fremder Handler existiert, einen simulieren – **vor** der Installation,
also ggf. VM-Snapshot zurücksetzen und erst diesen Befehl, dann Schritt 1 ausführen):

```bat
reg add "HKCU\Software\Classes\.heif\ShellEx\{e357fccd-a995-4576-b01f-234630154e96}" /ve /d "{C7657C4A-9F68-40fa-A4DF-96BC08EB3551}" /f
```

- ☐ Nach der Installation zeigt `.heif` auf `{218C7AE3-…}`, und
  `reg query "HKCU\Software\HEIC Explorer Preview\ThumbnailHandlerBackup"` enthält
  `.heif = {C7657C4A-…}`.
- ☐ Deinstallation über „Einstellungen → Apps → Installierte Apps“ läuft fehlerfrei.
- ☐ Danach: `.heif` zeigt wieder auf `{C7657C4A-…}`; `.heic`/`.hif`-Schlüssel (falls vorher nicht
  vorhanden) sind entfernt; `HKCU\Software\Classes\CLSID\{218C7AE3-…}` und
  `HKCU\Software\HEIC Explorer Preview` existieren nicht mehr.
- ☐ Installationsordner ist weg (oder enthält nur noch `*.old`, falls `dllhost.exe` die DLL
  geladen hatte).
- ☐ Mit installierter Microsoft-HEIF-Erweiterung: nach der Deinstallation erzeugt wieder
  Windows/Microsoft die Vorschaubilder (ggf. Cache leeren).

## 9. Silent-Install

Eingabeaufforderung (nicht als Admin):

```bat
HEICExplorerPreview-Setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER /LOG=%TEMP%\heic-setup.log
echo %ERRORLEVEL%
```

- ☐ Kein Fenster, kein Prompt, Exit-Code 0, Log ohne Fehler, Vorschaubilder funktionieren.
- ☐ Silent-Deinstallation:
  `"%LocalAppData%\Programs\HEIC Explorer Preview\unins000.exe" /VERYSILENT /SUPPRESSMSGBOXES`
  → Registry und Ordner wie in Abschnitt 8 bereinigt.

## 10. Installation für alle Benutzer

- ☐ Installer per GUI starten → „Für alle Benutzer installieren“ → UAC-Abfrage → Installation
  nach `C:\Program Files\HEIC Explorer Preview`.
- ☐ Registrierung unter `HKLM\Software\Classes\…` (gleiche Abfragen wie oben mit `HKLM`).
- ☐ Ein zweites Benutzerkonto sieht ebenfalls Vorschaubilder.
- ☐ Admin-Konsole: `HEICExplorerPreview-Setup.exe /VERYSILENT /SUPPRESSMSGBOXES /ALLUSERS` funktioniert.
- ☐ Deinstallation stellt einen vorher in `HKLM` registrierten Handler wieder her.

## 11. Direkter COM-Test (optional)

```bat
thumbnail-probe.exe C:\heic-test\quadrants_orientation6.heic 256 --bmp %TEMP%\probe.bmp
thumbnail-probe.exe --shell C:\heic-test\IMG_0001.HEIC 256
```

- ☐ Erste Zeile: `64x96 32bpp alpha=rgb center=#…` – beweist Registrierung + Laden von
  `heif.dll` aus dem Installationsordner.
- ☐ `--shell` liefert ein Bild (Shell-Pfad wie im Explorer). Wichtig: aus einer **normalen,
  nicht als Administrator gestarteten** Konsole ausführen. Bei elevated Aufrufern ignoriert COM
  die per-user-Registrierung (HKCU) im Thumbnail-Hostprozess („Class not registered“). Im CI
  (elevated Runner) tritt genau das auf. Schlägt es auch nicht-elevated fehl, bitte melden.

## Ergebnis

| Bereich | Ergebnis | Notizen |
|---|---|---|
| 1 Installation | | |
| 2 Vorschaubilder | | |
| 3 Rotation | | |
| 4 Farben | | |
| 5 Transparenz | | |
| 6 Robustheit | | |
| 7 Update | | |
| 8 Deinstallation/Restore | | |
| 9 Silent | | |
| 10 Alle Benutzer | | |

Windows-Version (`winver`): ______ · Installer-Version: ______ · Datum: ______
