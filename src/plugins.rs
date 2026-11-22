//! Plugins: eigenständige Lua-Erweiterungen mit eigenem Lua-Zustand.
//!
//! Ein Plugin ist ein Ordner im Plugin-Verzeichnis mit einer `plugin.toml`
//! (Name, Version, Autor, Beschreibung) und einer Lua-Hauptdatei (`main.lua`).
//! Plugins werden pro Welt aktiviert und bekommen Ereignisse über Callbacks
//! im Stil von MUSHclient (`OnPluginConnect`, `OnPluginLineReceived` …).

use crate::scripting::ScriptEngine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "plugin.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginMeta {
    /// Eindeutige Kennung (Ordnername), z. B. "lp-warnung"
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    /// Lua-Hauptdatei relativ zum Plugin-Ordner
    pub main: String,
    /// Mindestversion von rustmud (optional)
    pub min_rustmud: String,
    #[serde(skip)]
    pub dir: PathBuf,
}

impl Default for PluginMeta {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: "1.0.0".into(),
            author: String::new(),
            description: String::new(),
            main: "main.lua".into(),
            min_rustmud: String::new(),
            dir: PathBuf::new(),
        }
    }
}

impl PluginMeta {
    pub fn main_path(&self) -> PathBuf {
        self.dir.join(&self.main)
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join(MANIFEST);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut m: PluginMeta = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        m.dir = dir.to_path_buf();
        if m.id.trim().is_empty() {
            m.id = dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
        }
        if m.name.trim().is_empty() {
            m.name = m.id.clone();
        }
        Ok(m)
    }

    fn save(&self) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(self.dir.join(MANIFEST), text).map_err(|e| e.to_string())
    }
}

/// Plugins, die mit rustmud ausgeliefert werden (Kennung, plugin.toml, main.lua).
pub const BUNDLED: &[(&str, &str, &str)] = &[
    (
        "karte",
        include_str!("../plugins/karte/plugin.toml"),
        include_str!("../plugins/karte/main.lua"),
    ),
    (
        "spielstand",
        include_str!("../plugins/spielstand/plugin.toml"),
        include_str!("../plugins/spielstand/main.lua"),
    ),
    (
        "lp-warnung",
        include_str!("../plugins/lp-warnung/plugin.toml"),
        include_str!("../plugins/lp-warnung/main.lua"),
    ),
];

pub fn is_bundled(id: &str) -> bool {
    BUNDLED.iter().any(|(b, _, _)| *b == id)
}

/// Datei mit den Kennungen mitgelieferter Plugins, die der Benutzer gelöscht hat.
fn removed_file() -> PathBuf {
    plugins_dir().join(".entfernt")
}

fn removed_bundled() -> Vec<String> {
    std::fs::read_to_string(removed_file())
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Installiert fehlende mitgelieferte Plugins und aktualisiert ältere Versionen.
/// Vom Benutzer gelöschte mitgelieferte Plugins werden nicht wiederhergestellt.
pub fn install_bundled() -> Vec<String> {
    let mut changed = Vec::new();
    let removed = removed_bundled();
    for (id, manifest, main) in BUNDLED {
        if removed.iter().any(|r| r == id) {
            continue;
        }
        let dir = plugins_dir().join(id);
        let bundled: PluginMeta = match toml::from_str(manifest) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let install = match PluginMeta::load(&dir) {
            Ok(installed) => crate::updater::is_newer(&bundled.version, &installed.version),
            Err(_) => true,
        };
        if install
            && std::fs::create_dir_all(&dir).is_ok()
            && std::fs::write(dir.join(MANIFEST), manifest).is_ok()
            && std::fs::write(dir.join("main.lua"), main).is_ok()
        {
            changed.push(bundled.name.clone());
        }
    }
    changed
}

/// Mitgeliefertes Plugin auf den Auslieferungszustand zurücksetzen.
pub fn restore_bundled(id: &str) -> Result<(), String> {
    let (_, manifest, main) = BUNDLED
        .iter()
        .find(|(b, _, _)| *b == id)
        .ok_or("kein mitgeliefertes Plugin")?;
    let dir = plugins_dir().join(id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(MANIFEST), manifest).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("main.lua"), main).map_err(|e| e.to_string())?;
    let rest: Vec<String> = removed_bundled().into_iter().filter(|r| r != id).collect();
    let _ = std::fs::write(removed_file(), rest.join("\n"));
    Ok(())
}

pub fn plugins_dir() -> PathBuf {
    crate::world::config_dir().join("plugins")
}

/// Alle installierten Plugins sowie Fehlermeldungen zu defekten Plugin-Ordnern.
pub fn list_plugins() -> (Vec<PluginMeta>, Vec<String>) {
    let mut ok = Vec::new();
    let mut errors = Vec::new();
    if let Ok(rd) = std::fs::read_dir(plugins_dir()) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                match PluginMeta::load(&p) {
                    Ok(m) => ok.push(m),
                    Err(e) => errors.push(e),
                }
            }
        }
    }
    ok.sort_by_key(|m| m.name.to_lowercase());
    (ok, errors)
}

/// Wandelt einen Namen in eine Ordner-Kennung um („LP-Warnung“ → „lp-warnung“).
pub fn slug(name: &str) -> String {
    let s: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'ä' => 'a',
            'ö' => 'o',
            'ü' => 'u',
            'ß' => 's',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() {
        "plugin".into()
    } else {
        s
    }
}

fn free_dir(id: &str) -> PathBuf {
    let base = plugins_dir();
    let mut dir = base.join(id);
    let mut i = 2;
    while dir.exists() {
        dir = base.join(format!("{id}-{i}"));
        i += 1;
    }
    dir
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let target = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// Installiert einen Plugin-Ordner (mit `plugin.toml`) durch Kopieren.
pub fn install_from_dir(src: &Path) -> Result<PluginMeta, String> {
    let meta = PluginMeta::load(src)?;
    if !meta.main_path().exists() {
        return Err(format!("Hauptdatei {} fehlt", meta.main));
    }
    let dst = free_dir(&slug(&meta.id));
    copy_dir(src, &dst).map_err(|e| e.to_string())?;
    let mut installed = PluginMeta::load(&dst)?;
    // Ordnername ist die Kennung
    installed.id = dst.file_name().unwrap().to_string_lossy().to_string();
    installed.save()?;
    Ok(installed)
}

/// Installiert eine einzelne Lua-Datei als Plugin.
pub fn install_from_lua(file: &Path) -> Result<PluginMeta, String> {
    let name = file
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Plugin".into());
    let dir = free_dir(&slug(&name));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::copy(file, dir.join("main.lua")).map_err(|e| e.to_string())?;
    let meta = PluginMeta {
        id: dir.file_name().unwrap().to_string_lossy().to_string(),
        name,
        description: format!("Importiert aus {}", file.display()),
        dir,
        ..Default::default()
    };
    meta.save()?;
    Ok(meta)
}

const TEMPLATE: &str = r#"-- Plugin für rustmud
-- Alle Funktionen der Lua-API (Send, Note, ColourNote, GetVariable, …) stehen zur Verfügung.
-- Variablen (SetVariable/GetVariable) sind pro Plugin und Welt getrennt gespeichert.

-- Wird aufgerufen, wenn das Plugin geladen wird
function OnPluginInstall()
  Note(GetPluginName() .. " geladen.")
end

-- Nach dem Verbinden bzw. Trennen
function OnPluginConnect() end
function OnPluginDisconnect() end

-- Jede empfangene Zeile (ohne Farben). „return false“ blendet die Zeile aus.
function OnPluginLineReceived(zeile)
  return true
end

-- Jeder Befehl vor dem Senden. „return false“ verhindert das Senden.
function OnPluginCommand(befehl)
  return true
end

-- GMCP-/MSDP-Daten (zusätzlich in den Tabellen gmcp und msdp)
function OnPluginGMCP(paket, json) end
function OnPluginMSDP(variable) end
"#;

/// Legt ein neues Plugin aus einer Vorlage an.
pub fn create_template(name: &str, author: &str) -> Result<PluginMeta, String> {
    let dir = free_dir(&slug(name));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("main.lua"), TEMPLATE).map_err(|e| e.to_string())?;
    let meta = PluginMeta {
        id: dir.file_name().unwrap().to_string_lossy().to_string(),
        name: name.trim().to_string(),
        author: author.trim().to_string(),
        description: "Beschreibung des Plugins".into(),
        dir,
        ..Default::default()
    };
    meta.save()?;
    Ok(meta)
}

pub fn delete(meta: &PluginMeta) -> Result<(), String> {
    if !meta.dir.starts_with(plugins_dir()) {
        return Err("Plugin liegt nicht im Plugin-Ordner".into());
    }
    std::fs::remove_dir_all(&meta.dir).map_err(|e| e.to_string())?;
    if is_bundled(&meta.id) {
        let mut removed = removed_bundled();
        removed.push(meta.id.clone());
        let _ = std::fs::write(removed_file(), removed.join("\n"));
    }
    Ok(())
}

/// Ein für eine Welt geladenes Plugin.
pub struct PluginInstance {
    pub meta: PluginMeta,
    pub engine: ScriptEngine,
}

impl PluginInstance {
    pub fn load(meta: PluginMeta, variables: BTreeMap<String, String>, world_name: &str) -> Self {
        let engine = ScriptEngine::new(variables, world_name);
        engine.set_plugin_info(&meta.id, &meta.name, &meta.dir.display().to_string());
        engine.load_file(&meta.main_path().display().to_string());
        engine.call_if_exists("OnPluginInstall", ());
        Self { meta, engine }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::world::World;
    use std::sync::Arc;

    /// Lädt ein mitgeliefertes Plugin direkt aus dem Quelltext in eine Testsitzung.
    fn session_with(ids: &[&str]) -> (Session, Vec<PathBuf>) {
        let mut s = Session::new(World::default(), None, Arc::new(|| {}));
        let mut dirs = Vec::new();
        for id in ids {
            let (_, manifest, main) = BUNDLED.iter().find(|(b, _, _)| b == id).unwrap();
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("rustmud-bundled-{}-{n}-{id}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("main.lua"), main).unwrap();
            let mut meta: PluginMeta = toml::from_str(manifest).unwrap();
            meta.dir = dir.clone();
            s.world.plugins.push(meta.id.clone());
            s.load_plugin_for_test(meta);
            dirs.push(dir);
        }
        (s, dirs)
    }

    fn feed(s: &mut Session, text: &str) {
        s.feed_for_test(text.replace('\n', "\r\n").as_bytes());
    }

    fn window_lines(s: &Session, id: &str) -> Vec<String> {
        let all: Vec<String> = s.lines.iter().map(|l| l.plain()).collect();
        s.windows
            .get(id)
            .unwrap_or_else(|| panic!("Fenster {id} fehlt: {all:?}"))
            .items
            .iter()
            .filter_map(|i| match i {
                crate::scripting::WinItem::Line(l) => Some(l.plain()),
                _ => None,
            })
            .collect()
    }

    const ROOM: &str = "\
+-------------+ Hier bist Du nun also am Eingang der
|*************| Dunkelelfenstadt \"Corona\".
|#(N)-[X]-( )-| In unregelmaessigen Abstaenden
|******|*o***&| 
+-------------+ von Wasser.
   Ausgaenge: Norden und Osten.
+----+----+
";

    /// Landschaftskarte mit Bäumen „oOo“ (Buchstaben in der Karte) und Rahmen nur unten mit Text daneben
    #[test]
    fn karte_plugin_with_letter_terrain() {
        let (mut s, dirs) = session_with(&["karte"]);
        let before = s.lines.len();
        feed(
            &mut s,
            "\
+-----------------+ Du befindest Dich auf einem breiten Weg entlang, der
|oOo|.~.|....&oooO| durch sanfte Huegel und Wiesen fuehrt. Der Weg verlaeuft
|oOo|.~.|....&o&oo| von Westen nach Osten.
|&oo|.~.|....&o&oo| Es ist Nacht.
|&o|.~.||.......&o| 
|o|.~.||#X######..| 
|#|.~..|**~~~**...| 
+-----------------+ 
",
        );
        let out: Vec<String> = s.lines.iter().skip(before).map(|l| l.plain()).collect();
        assert_eq!(
            out,
            vec![
                "Du befindest Dich auf einem breiten Weg entlang, der",
                "durch sanfte Huegel und Wiesen fuehrt. Der Weg verlaeuft",
                "von Westen nach Osten.",
                "Es ist Nacht.",
            ]
        );
        let map = window_lines(&s, "karte");
        assert_eq!(map.len(), 8);
        assert_eq!(map[1], "|oOo|.~.|....&oooO|");
        for d in dirs {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn karte_plugin_cuts_map() {
        let (mut s, dirs) = session_with(&["karte"]);
        let before = s.lines.len();
        feed(&mut s, ROOM);
        let out: Vec<String> = s.lines.iter().skip(before).map(|l| l.plain()).collect();
        assert_eq!(
            out,
            vec![
                "Hier bist Du nun also am Eingang der",
                "Dunkelelfenstadt \"Corona\".",
                "In unregelmaessigen Abstaenden",
                "von Wasser.",
                "   Ausgaenge: Norden und Osten.",
                "+----+----+",
            ]
        );
        let map = window_lines(&s, "karte");
        assert_eq!(map.len(), 5);
        assert_eq!(map[2], "|#(N)-[X]-( )-|");
        assert!(s.window_requests.contains(&"karte".to_string()));
        // Seitenansicht: der Raum beginnt mit der ersten Textzeile neben der Karte
        let a = s.page_start_index().expect("Seitenbeginn gesetzt");
        assert_eq!(s.lines[a].plain(), "Hier bist Du nun also am Eingang der");
        // größere Karte
        let big = format!(
            "+{0}+ Text\n{1}+{0}+\n",
            "-".repeat(25),
            (0..9)
                .map(|i| format!("|{}| Zeile {i}\n", "*".repeat(25)))
                .collect::<String>()
        );
        feed(&mut s, &big);
        assert_eq!(window_lines(&s, "karte").len(), 11);
        for d in dirs {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    const SCORE: &str = "\
+-------------------------------------------------------------------------+
|        Dunkelelfenbengel Gast1, der unerfahrene Avalon-Besucher         |
+-------------------------------------------------------------------------+
|  Staerke.........:   1   Ausdauer........:   1   Geschicklichkeit:   1  |
|  Intelligenz.....:   1   Weisheit........:   1   Aura............:   1  |
+-------------------------------------------------------------------------+
|                             0%       25%        50%       75%     100%  |
|  Trefferpunkte:   54 (  54) ##########################################  |
|  Manapunkte:    2500 (9999) ###########                                 |
|  Tragkraft             92%  #######################################     |
+-------------------------------------+-----------------------------------+
|  Erfahrungspunkte:               0  |  Flucht bei:          (25 TP) an  |
+-----------------------------------------------------+-------------------+
|  Alter:  20 Minuten 20 Sekunden                     |         Stufe S1  |
+-----------------------------------------------------+-------------------+
Du hast ausreichend gegessen.
";

    #[test]
    fn spielstand_plugin_parses_score() {
        let (mut s, dirs) = session_with(&["karte", "spielstand"]);
        let before = s.lines.len();
        feed(&mut s, SCORE);
        let out: Vec<String> = s.lines.iter().skip(before).map(|l| l.plain()).collect();
        assert_eq!(out, vec!["Du hast ausreichend gegessen."]);
        let items = &s.windows["spielstand"].items;
        use crate::scripting::WinItem::*;
        assert!(items
            .iter()
            .any(|i| matches!(i, Heading(h) if h.starts_with("Dunkelelfenbengel Gast1"))));
        assert!(items.iter().any(
            |i| matches!(i, Gauge { label, value, max, .. } if label == "Trefferpunkte" && *value == 54.0 && *max == 54.0)
        ));
        assert!(items
            .iter()
            .any(|i| matches!(i, Gauge { label, value, .. } if label == "Tragkraft" && *value == 92.0)));
        assert!(items
            .iter()
            .any(|i| matches!(i, Pair(k, v) if k == "Geschicklichkeit" && v == "1")));
        assert!(items
            .iter()
            .any(|i| matches!(i, Pair(k, v) if k == "Flucht bei" && v == "(25 TP) an")));
        assert!(items
            .iter()
            .any(|i| matches!(i, Pair(k, v) if k == "Stufe" && v == "S1")));
        // andere Tabellen bleiben in der Ausgabe
        let before = s.lines.len();
        feed(
            &mut s,
            "+------------------------------+\n| Wer ist online?              |\n+------------------------------+\n> ",
        );
        s.feed_prompt_for_test();
        let out: Vec<String> = s.lines.iter().skip(before).map(|l| l.plain()).collect();
        assert_eq!(
            out,
            vec![
                "+------------------------------+",
                "| Wer ist online?              |",
                "+------------------------------+",
                "> "
            ]
        );
        // Karte und Spielstand zusammen: Karte wird trotzdem erkannt
        feed(&mut s, ROOM);
        assert_eq!(window_lines(&s, "karte").len(), 5);
        for d in dirs {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("LP-Warnung für Kämpfe!"), "lp-warnung-fur-kampfe");
        assert_eq!(slug("   "), "plugin");
    }

    #[test]
    fn template_is_valid_lua_with_callbacks() {
        let dir = std::env::temp_dir().join(format!("rustmud-plugin-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.lua"), TEMPLATE).unwrap();
        let meta = PluginMeta {
            id: "test".into(),
            name: "Test".into(),
            dir: dir.clone(),
            ..Default::default()
        };
        let p = PluginInstance::load(meta, BTreeMap::new(), "Welt");
        let actions = p.engine.take_actions();
        assert!(actions
            .iter()
            .any(|a| matches!(a, crate::scripting::Action::Note(l) if l.plain() == "Test geladen.")));
        assert_eq!(p.engine.call_bool("OnPluginLineReceived", "x".to_string()), Some(true));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
