# rustmud

[![CI](https://github.com/areadox/Rustmud/actions/workflows/ci.yml/badge.svg)](https://github.com/areadox/Rustmud/actions/workflows/ci.yml)
[![Lizenz: MIT](https://img.shields.io/badge/Lizenz-MIT-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/areadox/Rustmud?include_prereleases)](https://github.com/areadox/Rustmud/releases)

Ein freier, quelloffener MUD-Client in Rust nach dem Vorbild von [MUSHclient](https://www.gammon.com.au/mushclient/).
Oberfläche mit [egui](https://github.com/emilk/egui), Scripting mit Lua 5.4 und eine MUSHclient-ähnliche Lua-API.
Läuft unter Windows, Linux und macOS.

> **English:** rustmud is a free and open source MUD client written in Rust, inspired by MUSHclient.
> It supports MCCP, GMCP, MSDP, ANSI colours, triggers, aliases, timers and Lua scripting.
> The UI and documentation are currently in German; contributions in English are welcome.

## Funktionen

- **Telnet** mit Optionsverhandlung, Prompt-Erkennung über GA/EOR und Passwortmodus (Server-ECHO)
- **MCCP2** (Kompression), **GMCP** und **MSDP**, NAWS (Fenstergröße) und TTYPE/MTTS
- **ANSI-Farben**: 16 Farben, 256 Farben und Truecolor sowie fett, kursiv, unterstrichen und invers
- **Zeichensatz** pro Welt: Latin-1 (Standard, passend für die meisten deutschen MUDs) oder UTF-8
- **Weltenverwaltung im Programm**: Welten anlegen, öffnen, verbinden, bearbeiten, umbenennen, duplizieren,
  löschen, importieren und exportieren; mehrere Welten gleichzeitig als Tabs
- **Icon-Leiste** für alle wichtigen Funktionen plus **eigene Knöpfe** pro Welt (z. B. „⚔ Angriff“ → `toete @ziel`)
- **Ziffernblock zum Laufen**, pro Welt frei belegbar (Standard: deutsche Richtungen n, no, o, so, s, sw, w, nw, ob, u)
- **Installer für Windows** und **Update-Funktion**: rustmud prüft beim Start auf neue Versionen und aktualisiert sich auf Wunsch selbst
- **Trigger** mit Wildcard (`*`) oder Regex, Umfärben, Zeile unterdrücken, Einmal-Triggern, Reihenfolge, Gruppen und Testfeld im Editor
- **Aliase** und **Timer** (auch einmalig oder nur bei bestehender Verbindung)
- **Variablen** (`@name` in Triggern und Aliassen, `GetVariable`/`SetVariable` in Lua)
- **Befehlstrenner** (`;`), **Speedwalk** (`#3n2o(nw)`) und **Lua direkt in der Eingabezeile** (`/Note("hallo")`)
- **Logging** in eine Datei pro Welt und Tag
- GMCP/MSDP-Inspektor als Seitenleiste
- Befehlshistorie mit ↑/↓, automatisches Verbinden beim Öffnen und automatisches Neuverbinden

## Download und Installation

Fertige Programme gibt es unter [Releases](https://github.com/areadox/Rustmud/releases):

| Datei | Beschreibung |
|-------|--------------|
| `rustmud-setup-x64.exe` | **Windows-Installer** (empfohlen) – installiert ohne Adminrechte nach `%LOCALAPPDATA%\Programs\rustmud`, mit Startmenü-Eintrag, optionaler Desktop-Verknüpfung und Deinstallation über „Apps & Features“ |
| `rustmud-windows-x64.zip` | Windows ohne Installation (portabel) |
| `rustmud-linux-x64.tar.gz` | Linux |
| `rustmud-macos-arm64.tar.gz` | macOS (Apple Silicon) |

### Updates

rustmud prüft beim Start, ob es auf GitHub eine neuere Version gibt (abschaltbar unter **Hilfe → Über rustmud**).
Bei einer Installation über den Windows-Installer genügt dann ein Klick auf **„Jetzt aktualisieren“**:
Das neue Setup wird heruntergeladen, still installiert und rustmud danach neu gestartet.
Bei der portablen Version und unter Linux/macOS öffnet sich stattdessen die Download-Seite.
Welten, Skripte und Logs bleiben bei Updates und bei der Deinstallation erhalten.

## Selbst bauen

```sh
cargo build --release
```

Die fertige Datei liegt unter `target/release/rustmud` (unter Windows `rustmud.exe`).
Lua wird mitkompiliert, daher wird ein C-Compiler benötigt (unter Windows die MSVC Build Tools).

Tests ausführen (inklusive eines Durchlaufs gegen einen Fake-MUD auf localhost):

```sh
cargo test
```

## Erste Schritte

1. rustmud starten – die **Weltenverwaltung** (🌍) öffnet sich automatisch
2. Unter „Neue Welt“ Name, Server und Port eintragen, z. B. MorgenGrauen: `mg.mud.de`, Port `4711`
3. Zeichensatz wählen (Latin-1 für die meisten deutschen MUDs) und **Anlegen und öffnen**
4. In der Icon-Leiste auf 🔌 **Verbinden** klicken

## Bedienung

### Icon-Leiste

| Symbol | Funktion |
|--------|----------|
| 🌍 | Weltenverwaltung |
| 🔌 / ⏏ | Verbinden / Trennen |
| 💾 | Welt speichern |
| ⚙ ⚡ ⌨ ⏱ 📋 🔢 | Einstellungen, Trigger, Aliase, Timer, Variablen, Ziffernblock |
| 🔄 | Lua-Skript neu laden |
| 📝 | Log starten/beenden |
| 📡 | GMCP/MSDP-Daten anzeigen |
| 🗑 | Ausgabe leeren |
| ➕ | eigene Knöpfe anlegen und bearbeiten |

Eigene Knöpfe erscheinen rechts daneben. Sie können einen Befehl senden, wie eine Eingabe ausgeführt werden
(mit Aliassen, `;` und Speedwalk), Lua-Code ausführen oder eine Notiz ausgeben.
Die Leiste lässt sich unter **Ansicht → Icon-Leiste** ausblenden.

### Ziffernblock

Unter 🔢 **Ziffernblock** wird jede Taste des Ziffernblocks mit einem Befehl belegt (auch mehrere mit `;`).

| | | | |
|---|---|---|---|
| | `/` | `*` | `-` ob |
| `7` nw | `8` n | `9` no | `+` u |
| `4` w | `5` schau | `6` o | |
| `1` sw | `2` s | `3` so | |
| `0` | | `.` | |

Unter **Windows** werden die Ziffernblock-Tasten exakt erkannt (NumLock an oder aus) – die normalen Zifferntasten
schreiben weiterhin Zahlen. Auf Linux/macOS lässt sich der Ziffernblock nicht unterscheiden; dort wirkt die Belegung
für 0–9 und `+`, solange die Eingabezeile leer ist. Beim Tippen in Dialogfeldern ist der Ziffernblock nicht aktiv.

Welten liegen im Konfigurationsordner:

| System  | Ordner |
|---------|--------|
| Windows | `%APPDATA%\areadox\rustmud\config\worlds\` |
| Linux   | `~/.config/rustmud/worlds/` |
| macOS   | `~/Library/Application Support/de.areadox.rustmud/worlds/` |

Beim Beenden werden alle offenen Welten gespeichert und beim nächsten Start wieder geöffnet.

## Trigger, Aliase, Timer

Muster sind standardmäßig MUSHclient-Wildcards: `*` steht für beliebigen Text, und das Muster muss die ganze Zeile treffen.
Mit „Regulärer Ausdruck“ gilt die Rust-Regex-Syntax.

Im Sendetext:

| Platzhalter | Bedeutung |
|-------------|-----------|
| `%0`        | gesamter Treffer |
| `%1` … `%9` | Wildcards / Regex-Gruppen |
| `%<name>`   | benannte Regex-Gruppe |
| `@name`     | Variable |
| `%%`, `@@`  | wörtliches `%` bzw. `@` |

„Senden an“ kann sein: **MUD**, **Skript (Lua)**, **Ausgabe (Notiz)** oder **Ausführen** (wie eine Eingabe, also mit Aliassen).

Beispiel einer Welt-Datei:

```toml
name = "MorgenGrauen"
host = "mg.mud.de"
port = 4711
encoding = "latin1"
script_file = "C:/mud/mg.lua"

[[triggers]]
name = "ork"
pattern = "Ein * kommt herein."
color = "orange"
send = "toete %1"

[[aliases]]
pattern = "gr *"
send = "sage Hallo %1!"

[[timers]]
interval = 300
send = "speichern"
```

## Lua-API

Die Funktionen gibt es global und zusätzlich als `world.Funktion` (wie bei MUSHclient).

| Funktion | Beschreibung |
|----------|--------------|
| `Send(text, …)` | an den MUD senden |
| `Execute(text)` | wie eine Eingabe ausführen (Aliase, `;`, Speedwalk) |
| `Note(text)`, `Tell(text)` | lokale Ausgabe (Tell ohne Zeilenumbruch) |
| `ColourNote(vg, hg, text, …)`, `ColourTell(…)` | farbige Ausgabe, Farben als Name oder `#rrggbb` |
| `print(…)` | wie `Note` |
| `GetVariable(n)`, `SetVariable(n, w)`, `DeleteVariable(n)`, `GetVariableList()` | Variablen |
| `EnableTrigger(n, an)`, `EnableAlias`, `EnableTimer`, `EnableGroup` | ein-/ausschalten |
| `DoAfter(sek, text)`, `DoAfterNote(sek, text)`, `DoAfterSpecial(sek, text, ziel)` | verzögert ausführen |
| `SendGMCP(paket, json)`, `SendMSDP(var, …)` | Protokolldaten senden |
| `Connect()`, `Disconnect()`, `WorldName()` | Verbindung und Weltname |

Callbacks, falls im Skript definiert: `OnConnect()`, `OnDisconnect()`, `OnGMCP(paket, json)`, `OnMSDP(variable)`.
GMCP-Daten stehen zusätzlich als Tabelle bereit (`gmcp.Char.Vitals.lp`), MSDP unter `msdp.VARIABLE`.

Trigger und Aliase können eine Lua-Funktion aufrufen: `funktion(name, zeile, wildcards)`.
Bei „Senden an: Skript“ stehen die Treffer in den Tabellen `matches` und `wildcards`.

Ein Beispielskript liegt unter [`beispiele/beispiel.lua`](beispiele/beispiel.lua).

## Aufbau

| Datei | Inhalt |
|-------|--------|
| `telnet.rs` | Telnet-Zustandsmaschine, MCCP, GMCP, MSDP, NAWS, TTYPE |
| `ansi.rs` | ANSI-Parser, Farbpalette, Farbnamen |
| `net.rs` | Netzwerk-Thread pro Verbindung |
| `session.rs` | eine offene Welt: Ausgabe, Eingabe, Trigger, Aliase, Timer, Log |
| `automation.rs` | Mustervergleich, Platzhalter, Speedwalk, Befehlstrenner |
| `scripting.rs` | Lua-Einbindung (mlua) und API |
| `world.rs` | Welt-Dateien (TOML) |
| `keypad.rs` | Ziffernblock-Erkennung (Windows: Fenster-Subclassing) |
| `updater.rs` | Update-Prüfung und -Installation über GitHub-Releases |
| `app.rs`, `ui_editors.rs` | Oberfläche, Icon-Leiste, Weltenverwaltung, Dialoge |
| `installer/rustmud.nsi` | Windows-Installer (NSIS) |

## Geplant

- Mini-Windows (eigene Zeichenflächen aus Lua, wie `WindowCreate` bei MUSHclient)
- MXP, Plugins und Import von MUSHclient-Welten (`.mcl`)
- Suche in der Ausgabe, mehrere Ausgabefenster

Ideen und Fehler gerne als [Issue](https://github.com/areadox/Rustmud/issues) melden.

## Mitmachen

Beiträge sind willkommen! Wie man das Projekt baut, testet und Pull Requests einreicht, steht in
[CONTRIBUTING.md](CONTRIBUTING.md). Es gilt der [Verhaltenskodex](CODE_OF_CONDUCT.md).
Sicherheitslücken bitte vertraulich melden, siehe [SECURITY.md](SECURITY.md).
Änderungen zwischen den Versionen stehen in der [CHANGELOG.md](CHANGELOG.md).

## Lizenz

rustmud steht unter der [MIT-Lizenz](LICENSE) – du darfst es frei benutzen, verändern und weitergeben.

Die verwendeten Bibliotheken (u. a. egui, mlua mit Lua 5.4, regex, flate2) stehen unter MIT-kompatiblen
Lizenzen (MIT, Apache-2.0, Zlib, BSD, CDLA-Permissive u. a.); das wird in der CI mit [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) geprüft.

rustmud ist ein eigenständiges Projekt und steht in keiner Verbindung zu MUSHclient oder dessen Autor Nick Gammon.
Es wurde kein Code aus MUSHclient übernommen; lediglich Bedienkonzept und Namen der Lua-Funktionen sind angelehnt.
