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
- **Frei anordnbare Fenster**: Ausgabe, Karte, Spielstand, Trigger, Einstellungen usw. lassen sich per Drag & Drop
  nebeneinander, untereinander, als Tabs oder als frei schwebende Fenster anordnen; die Anordnung wird gespeichert
- **Seitenansicht**: in der Ausgabe steht immer nur der aktuelle Raum oben im Fenster, der Verlauf erscheint beim
  Hochscrollen
- **Mitgelieferte Plugins**: **Karte** (ASCII-Karte neben dem Raumtext in ein eigenes Fenster) und **Spielstand**
  (Spielstand-Tabelle als übersichtliches Fenster mit Balken), dazu das Beispiel **LP-Warnung**
- **Plugins**: eigenständige Lua-Erweiterungen mit eigenem Zustand, pro Welt aktivierbar, mit **Plugin-Manager**
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
| ⚙ ⚡ ⌨ ⏱ 📋 🔢 📦 | Einstellungen, Trigger, Aliase, Timer, Variablen, Ziffernblock, Plugins |
| 🔄 | Lua-Skript neu laden |
| 📝 | Log starten/beenden |
| 📡 | GMCP/MSDP-Daten anzeigen |
| 🗑 | Ausgabe leeren |
| ➕ | eigene Knöpfe anlegen und bearbeiten |

Eigene Knöpfe erscheinen rechts daneben. Sie können einen Befehl senden, wie eine Eingabe ausgeführt werden
(mit Aliassen, `;` und Speedwalk), Lua-Code ausführen oder eine Notiz ausgeben.
Die Leiste lässt sich unter **Ansicht → Icon-Leiste** ausblenden.

### Fenster anordnen

Die Spielausgabe und alle anderen Fenster (Einstellungen, Trigger, Weltenverwaltung, Plugin-Fenster wie Karte und
Spielstand …) liegen in einem frei einteilbaren Fensterbereich:

- **Tab an der Titelleiste ziehen** und auf eine der eingeblendeten Zielflächen fallen lassen: links, rechts, oben,
  unten oder in die Mitte (als weiterer Tab).
- **Tab aus dem Fensterbereich herausziehen** erzeugt ein frei schwebendes Fenster, das man beliebig verschieben und
  in der Größe ändern kann.
- Die **Trennlinien** zwischen den Bereichen lassen sich mit der Maus verschieben.
- ✖ am Tab schließt ein Fenster, ein Klick auf das Symbol in der Icon-Leiste öffnet oder schließt es ebenfalls.
- Die Anordnung wird beim Beenden gespeichert (`layout.json` im Konfigurationsordner) und beim nächsten Start
  wiederhergestellt. **Ansicht → Fensteranordnung zurücksetzen** stellt den Ausgangszustand wieder her.

Die Inhalte der Fenster beziehen sich immer auf die oben gewählte Welt.

### Seitenansicht

Damit die Ausgabe nicht ständig durchrollt, beginnt mit jedem neuen Raum eine neue „Seite“: Die Raumbeschreibung
steht oben im Ausgabefenster, alles Vorherige liegt darüber und erscheint erst beim **Hochscrollen**. Was danach noch
kommt (Gespräche, Kampf …), läuft unter dem Raum weiter; reicht der Platz nicht, scrollt die Ausgabe wie gewohnt mit.
Scrollt man hoch, bleibt die Ansicht stehen, bis man wieder nach unten scrollt oder einen Befehl eingibt.

Einstellbar unter **Einstellungen → Ausgabe → Seitenansicht**:

| Modus | Neue Seite beginnt … |
|-------|----------------------|
| Neue Seite bei jedem Raum (Standard) | wenn ein Plugin einen neuen Raum meldet – das mitgelieferte Plugin **Karte** tut das bei jeder Karte |
| Neue Seite bei jedem Befehl | mit jedem eingegebenen Befehl |
| Aus | nie – die Ausgabe scrollt klassisch immer ans Ende |

Eigene Plugins und Skripte können mit `SetPageStart()` selbst einen Seitenbeginn setzen.

### Mitgelieferte Plugins: Karte und Spielstand

rustmud bringt einige Plugins gleich mit. Sie werden beim ersten Start installiert und lassen sich im
**Plugin-Manager** (📦) **pro Welt per Haken einschalten**.

**🗺 Karte** – Manche MUDs zeigen links neben der Raumbeschreibung eine kleine ASCII-Karte:

```
+-------------+ Hier bist Du nun also am Eingang der legendaeren
|*************| Dunkelelfenstadt "Corona" und ihr Anblick raubt Dir fast den
|#(N)-[X]-( )-| Atem. Der Tunnel, der vor Dir liegt, wurde mit grossem
+-------------+ von Wasser.
```

Das Plugin schneidet den Kartenblock heraus, zeigt ihn farbig im Fenster „Karte“ und lässt nur den Text in der
Ausgabe (auch Trigger sehen nur den Text). Breite und Höhe werden bei jeder Karte neu aus dem Rahmen bestimmt, eine
mit der Zeit wachsende Karte wird also automatisch erkannt und an das Fenster angepasst. Kästen und Tabellen mit
Wörtern darin bleiben in der Ausgabe; Buchstaben als Kartenzeichen (z. B. Bäume `oOo`, Orte `(N)`) sind erlaubt. Befehle: `karte` (Fenster zeigen), `karte breite <zahl>` (maximale
Kartenbreite, Standard 60).

**📊 Spielstand** – Erkennt die Spielstand-Tabelle (z. B. nach `sp`/`score`) und zeigt sie als Fenster mit farbigen
Balken für Treffer-, Aktions-, Zauber- und Manapunkte sowie Tragkraft, darunter Eigenschaften und Details. In der
Ausgabe verschwindet die Tabelle, andere Tabellen bleiben unverändert. Sendet der MUD GMCP-Werte (z. B.
`Char.Vitals` mit `hp`/`maxhp`), erscheinen sie zusätzlich als „Live“-Balken. Befehle: `spielstand` (Fenster zeigen
und `sp` senden), `spielstand auto <sek>` (regelmäßig automatisch `sp`, `0` = aus), `spielstand ausgabe an|aus`.

**LP-Warnung** – Beispiel-Plugin: warnt farbig bei niedrigen Lebenspunkten (`lpwarnung <zahl>`).

Mitgelieferte Plugins kann man wie alle anderen bearbeiten (✏ im Plugin-Manager). Mit ↺ lassen sie sich auf den
Auslieferungszustand zurücksetzen; neue Versionen werden bei Programm-Updates automatisch eingespielt.

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

## Plugins

Plugins erweitern rustmud um eigene Funktionen, ohne das Welt-Skript anzufassen. Jedes Plugin läuft in einem eigenen
Lua-Zustand, hat eigene Variablen und wird **pro Welt** im **Plugin-Manager** (📦) ein- und ausgeschaltet.

Im Plugin-Manager kann man:
- installierte Plugins sehen und per Haken in der aktuellen Welt aktivieren
- Plugins aus einem Ordner oder aus einer einzelnen `.lua`-Datei installieren
- ein neues Plugin aus einer Vorlage anlegen (öffnet die `main.lua` im Editor)
- Plugins neu laden, ihren Ordner öffnen oder sie löschen

Plugins liegen im Konfigurationsordner unter `plugins/<kennung>/`:

```
plugins/
└── mein-plugin/
    ├── plugin.toml
    └── main.lua
```

```toml
# plugin.toml
id = "mein-plugin"
name = "Mein Plugin"
version = "1.0.0"
author = "areadox"
description = "Warnt, wenn die Lebenspunkte unter eine Grenze fallen."
main = "main.lua"
```

Plugins können die komplette [Lua-API](#lua-api) nutzen und folgende Callbacks definieren:

| Callback | Wann |
|----------|------|
| `OnPluginInstall()` | beim Laden des Plugins |
| `OnPluginConnect()`, `OnPluginDisconnect()` | nach dem Verbinden / Trennen |
| `OnPluginLineReceived(zeile, ansi)` | für jede Zeile (ohne bzw. mit Farbcodes) – `return false` blendet sie aus, ein zurückgegebener Text ersetzt sie |
| `OnPluginCommand(befehl)` | für jeden Befehl vor dem Senden – `return false` verhindert das Senden |
| `OnPluginGMCP(paket, json)`, `OnPluginMSDP(variable)` | bei Protokolldaten (zusätzlich in `gmcp`/`msdp`) |
| `OnPluginDisable()` | beim Ausschalten |
| `OnPluginTick()` | etwa zehnmal pro Sekunde |

Zusätzliche Funktionen in Plugins: `GetPluginID()`, `GetPluginName()`, `GetPluginDirectory()`.
### Eigene Fenster und Zeilen umschreiben

Plugins können eigene Fenster anlegen, die wie alle anderen frei angeordnet werden:

| Funktion | Beschreibung |
|----------|--------------|
| `WindowCreate(id, titel)` | Fenster anlegen (erscheint erst nach `WindowShow`) |
| `WindowShow(id)` | Fenster öffnen |
| `WindowClear(id)` | Inhalt leeren |
| `WindowAddLine(id, text)` | Textzeile, ANSI-Farbcodes erlaubt (dunkler Block in Festbreitenschrift) |
| `WindowHeading(id, text)` | Überschrift |
| `WindowGauge(id, name, wert, max, farbe)` | farbiger Balken, Farbe als Name oder `#rrggbb` |
| `WindowPair(id, name, wert)` | Wertepaar, aufeinanderfolgende Paare werden als Tabelle angezeigt |
| `WindowSeparator(id)` | Trennlinie |
| `WindowOption(id, "fit", true)` | Textzeilen an die Fenstergröße anpassen; `"fontsize"` = feste Schriftgröße |
| `AnsiStrip(text)`, `AnsiLen(text)`, `AnsiSub(text, start, länge)` | Farbcodes entfernen, Länge, Teilstück mit Farben |
| `AnsiNote(text)` | farbige Notiz in der Ausgabe |
| `EmitLine(text)` | zurückgehaltene Zeile in der Ausgabe anzeigen (ohne Log/Trigger) |
| `SetPageStart()` | die nächste Zeile beginnt eine neue Seite (Seitenansicht, z. B. bei Raumbeginn) |

Die mitgelieferten Plugins in [`plugins/`](plugins) sind vollständige Beispiele:
[`karte`](plugins/karte/main.lua) schreibt Zeilen um und füllt ein Fenster,
[`spielstand`](plugins/spielstand/main.lua) hält Tabellen zurück und zeichnet Balken,
[`lp-warnung`](plugins/lp-warnung/main.lua) reagiert auf GMCP und eigene Befehle.

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
| `plugins.rs` | Plugin-Verwaltung (Manifest, Installation, Vorlage) |
| `keypad.rs` | Ziffernblock-Erkennung (Windows: Fenster-Subclassing) |
| `updater.rs` | Update-Prüfung und -Installation über GitHub-Releases |
| `app.rs`, `ui_editors.rs` | Oberfläche, Icon-Leiste, Fensterbereich (egui_dock), Weltenverwaltung, Plugin-Manager |
| `ui_widgets.rs` | Darstellung der Plugin-Fenster (Text, Balken, Wertepaare) |
| `plugins/` | mitgelieferte Plugins (werden ins Programm eingebettet) |
| `installer/rustmud.nsi` | Windows-Installer (NSIS) |

## Geplant

- Mini-Windows (eigene Zeichenflächen aus Lua, wie `WindowCreate` bei MUSHclient)
- MXP, Import von MUSHclient-Welten (`.mcl`) und MUSHclient-Plugins (`.xml`)
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
