# Mitmachen bei rustmud

Danke, dass du rustmud verbessern möchtest! Beiträge jeder Art sind willkommen:
Fehlerberichte, Ideen, Dokumentation und Code. Englisch ist ebenfalls in Ordnung.

## Fehler melden und Ideen vorschlagen

- Suche zuerst in den [Issues](https://github.com/areadox/Rustmud/issues), ob es das Thema schon gibt.
- Nutze die Vorlagen für Fehlerberichte und Funktionswünsche.
- Bei Verbindungsproblemen hilft die Angabe des MUDs (Host/Port) und des Zeichensatzes.
- Sicherheitslücken bitte **nicht** öffentlich melden, sondern wie in [SECURITY.md](SECURITY.md) beschrieben.

## Entwicklung

Voraussetzung ist eine aktuelle stabile Rust-Version (über [rustup](https://rustup.rs/)).
Lua wird über `mlua` mitgebaut, ein C-Compiler muss daher vorhanden sein.

```sh
git clone https://github.com/areadox/Rustmud.git
cd Rustmud
cargo run
```

Vor einem Pull Request bitte lokal prüfen – dasselbe macht auch die CI:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Pull Requests

1. Forke das Repository und lege einen Branch an (`feature/mini-windows`, `fix/mccp-abbruch` …).
2. Halte Änderungen möglichst klein und thematisch zusammenhängend.
3. Schreibe Tests für neue Logik (Protokolle, Trigger, Lua-API lassen sich ohne Oberfläche testen).
4. Passe die `README.md` an, wenn sich Funktionen oder Bedienung ändern, und trage die Änderung unter
   „Unveröffentlicht“ in der `CHANGELOG.md` ein.
5. Beschreibe im Pull Request, was sich ändert und warum.

## Lizenz deiner Beiträge

Mit dem Einreichen eines Beitrags erklärst du dich einverstanden, dass er unter der
[MIT-Lizenz](LICENSE) des Projekts veröffentlicht wird.

## Verhaltenskodex

Für alle Beteiligten gilt der [Verhaltenskodex](CODE_OF_CONDUCT.md).
