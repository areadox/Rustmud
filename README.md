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
- **Mehrere Welten** gleichzeitig als Tabs; jede Welt wird als TOML-Datei gespeichert
- **Trigger** mit Wildcard (`*`) oder Regex, Umfärben, Zeile unterdrücken, Einmal-Triggern, Reihenfolge, Gruppen und Testfeld im Editor
- **Aliase** und **Timer** (auch einmalig oder nur bei bestehender Verbindung)
- **Variablen** (`@name` in Triggern und Aliassen, `GetVariable`/`SetVariable` in Lua)
- **Befehlstrenner** (`;`), **Speedwalk** (`#3n2o(nw)`) und **Lua direkt in der Eingabezeile** (`/Note("hallo")`)
- **Logging** in eine Datei pro Welt und Tag
- GMCP/MSDP-Inspektor als Seitenleiste
- Befehlshistorie mit ↑/↓, automatisches Verbinden beim Öffnen und automatisches Neuverbinden

## Download

Fertige Programme für Windows, Linux und macOS gibt es unter [Releases](https://github.com/areadox/Rustmud/releases).
Archiv entpacken und `rustmud` bzw. `rustmud.exe` starten – eine Installation ist nicht nötig.

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

1. Programm starten → **Welt öffnen / Neu …** → **Neue Welt**
2. Name, Host und Port eintragen, z. B. MorgenGrauen: `mg.mud.de`, Port `4711`
3. Zeichensatz wählen (Latin-1 für die meisten deutschen MUDs)
4. **Verbinden** unten rechts

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
| `app.rs`, `ui_editors.rs` | Oberfläche |

## Geplant

- Mini-Windows (eigene Zeichenflächen aus Lua, wie `WindowCreate` bei MUSHclient)
- MXP, Plugins und Import von MUSHclient-Welten (`.mcl`)
- Ziffernblock für Richtungen, Suche in der Ausgabe

Ideen und Fehler gerne als [Issue](https://github.com/areadox/Rustmud/issues) melden.

## Mitmachen

Beiträge sind willkommen! Wie man das Projekt baut, testet und Pull Requests einreicht, steht in
[CONTRIBUTING.md](CONTRIBUTING.md). Es gilt der [Verhaltenskodex](CODE_OF_CONDUCT.md).
Sicherheitslücken bitte vertraulich melden, siehe [SECURITY.md](SECURITY.md).
Änderungen zwischen den Versionen stehen in der [CHANGELOG.md](CHANGELOG.md).

## Lizenz

rustmud steht unter der [MIT-Lizenz](LICENSE) – du darfst es frei benutzen, verändern und weitergeben.

Die verwendeten Bibliotheken (u. a. egui, mlua mit Lua 5.4, regex, flate2) stehen unter MIT-kompatiblen
Lizenzen (MIT, Apache-2.0, Zlib, BSD u. a.); das wird in der CI mit [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) geprüft.

rustmud ist ein eigenständiges Projekt und steht in keiner Verbindung zu MUSHclient oder dessen Autor Nick Gammon.
Es wurde kein Code aus MUSHclient übernommen; lediglich Bedienkonzept und Namen der Lua-Funktionen sind angelehnt.
