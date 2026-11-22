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
    std::fs::remove_dir_all(&meta.dir).map_err(|e| e.to_string())
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
