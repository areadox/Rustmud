# Changelog

Alle nennenswerten Änderungen an diesem Projekt werden hier festgehalten.
Das Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
die Versionierung [Semantic Versioning](https://semver.org/lang/de/).

## [Unveröffentlicht]

### Hinzugefügt
- Andockbereich: alle Fenster erscheinen als Tabs rechts neben der Ausgabe
- Plugin-Schnittstelle (Lua, eigener Zustand pro Plugin, pro Welt aktivierbar) und Plugin-Manager
- Beispiel-Plugin „LP-Warnung“

## [0.0.1] – 2026-11-22

### Hinzugefügt
- Telnet mit Optionsverhandlung, GA/EOR-Prompts und Passwortmodus
- MCCP2, GMCP, MSDP, NAWS und TTYPE/MTTS
- ANSI-Farben (16, 256, Truecolor), Latin-1 und UTF-8
- Mehrere Welten als Tabs, Welt-Dateien im TOML-Format
- Trigger, Aliase, Timer und Variablen im Stil von MUSHclient
- Lua-5.4-Scripting mit MUSHclient-ähnlicher API
- Befehlstrenner, Speedwalk, Lua in der Eingabezeile, Befehlshistorie
- Logging, GMCP/MSDP-Inspektor, automatisches (Neu-)Verbinden
- Weltenverwaltung im Programm (anlegen, öffnen, bearbeiten, umbenennen, duplizieren, löschen, Import/Export)
- Icon-Leiste mit allen Funktionen und eigenen Knöpfen pro Welt
- Ziffernblock zum Laufen, frei belegbar (unter Windows exakte Erkennung)
- Windows-Installer (NSIS, ohne Adminrechte) und Update-Funktion über GitHub-Releases
- Programmsymbol

[Unveröffentlicht]: https://github.com/areadox/Rustmud/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/areadox/Rustmud/releases/tag/v0.0.1
