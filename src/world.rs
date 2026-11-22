//! Welt-Dateien: Verbindungsdaten, Einstellungen, Trigger, Aliase, Timer, Variablen.
//! Gespeichert als TOML im Konfigurationsordner (`worlds/<name>.toml`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Encoding {
    /// ISO-8859-1 / Windows-1252 – bei vielen deutschen MUDs Standard.
    #[default]
    Latin1,
    Utf8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SendTo {
    /// An den MUD senden
    #[default]
    World,
    /// Als Lua-Code ausführen
    Script,
    /// Nur lokal ausgeben (Notiz)
    Output,
    /// Wie eine Eingabe behandeln (inkl. Alias-Auswertung)
    Execute,
}

impl SendTo {
    pub const ALL: [SendTo; 4] = [SendTo::World, SendTo::Script, SendTo::Output, SendTo::Execute];
    pub fn label(&self) -> &'static str {
        match self {
            SendTo::World => "MUD",
            SendTo::Script => "Skript (Lua)",
            SendTo::Output => "Ausgabe (Notiz)",
            SendTo::Execute => "Ausführen (mit Aliassen)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Trigger {
    pub name: String,
    pub group: String,
    pub enabled: bool,
    /// Muster: Wildcard (`*`) oder regulärer Ausdruck
    pub pattern: String,
    pub regex: bool,
    pub ignore_case: bool,
    /// Text, der gesendet wird. `%0` = ganze Zeile/Treffer, `%1`..`%9` = Wildcards
    pub send: String,
    pub send_to: SendTo,
    /// Name einer Lua-Funktion: `funktion(name, zeile, wildcards)`
    pub script: String,
    pub omit_from_output: bool,
    pub omit_from_log: bool,
    /// Zeile umfärben (Farbname oder #rrggbb), leer = nicht umfärben
    pub color: String,
    /// Niedrigere Zahl = früher ausgewertet
    pub sequence: i32,
    pub keep_evaluating: bool,
    pub one_shot: bool,
}

impl Default for Trigger {
    fn default() -> Self {
        Self {
            name: String::new(),
            group: String::new(),
            enabled: true,
            pattern: String::new(),
            regex: false,
            ignore_case: true,
            send: String::new(),
            send_to: SendTo::World,
            script: String::new(),
            omit_from_output: false,
            omit_from_log: false,
            color: String::new(),
            sequence: 100,
            keep_evaluating: true,
            one_shot: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Alias {
    pub name: String,
    pub group: String,
    pub enabled: bool,
    pub pattern: String,
    pub regex: bool,
    pub ignore_case: bool,
    pub send: String,
    pub send_to: SendTo,
    pub script: String,
    pub sequence: i32,
    pub keep_evaluating: bool,
    /// Das Kommando nicht in der Ausgabe wiederholen
    pub omit_from_output: bool,
}

impl Default for Alias {
    fn default() -> Self {
        Self {
            name: String::new(),
            group: String::new(),
            enabled: true,
            pattern: String::new(),
            regex: false,
            ignore_case: true,
            send: String::new(),
            send_to: SendTo::World,
            script: String::new(),
            sequence: 100,
            keep_evaluating: false,
            omit_from_output: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Timer {
    pub name: String,
    pub group: String,
    pub enabled: bool,
    /// Intervall in Sekunden (Bruchteile erlaubt)
    pub interval: f64,
    pub send: String,
    pub send_to: SendTo,
    pub script: String,
    pub one_shot: bool,
    /// Nur aktiv, solange verbunden
    pub only_when_connected: bool,
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            name: String::new(),
            group: String::new(),
            enabled: true,
            interval: 60.0,
            send: String::new(),
            send_to: SendTo::World,
            script: String::new(),
            one_shot: false,
            only_when_connected: true,
        }
    }
}

/// Eigener Knopf in der Icon-Leiste.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserButton {
    /// Beschriftung (Text oder Symbol)
    pub label: String,
    pub tooltip: String,
    pub send: String,
    pub send_to: SendTo,
}

impl Default for UserButton {
    fn default() -> Self {
        Self {
            label: "Neu".into(),
            tooltip: String::new(),
            send: String::new(),
            send_to: SendTo::Execute,
        }
    }
}

/// Standardbelegung für deutschsprachige MUDs.
pub fn default_keypad() -> BTreeMap<String, String> {
    [
        ("8", "n"),
        ("2", "s"),
        ("4", "w"),
        ("6", "o"),
        ("7", "nw"),
        ("9", "no"),
        ("1", "sw"),
        ("3", "so"),
        ("5", "schau"),
        ("-", "ob"),
        ("+", "u"),
        ("0", ""),
        (".", ""),
        ("/", ""),
        ("*", ""),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct World {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub encoding: Encoding,
    pub mccp: bool,
    pub gmcp: bool,
    pub msdp: bool,
    pub gmcp_supports: Vec<String>,
    /// Befehle, die nach dem Verbinden gesendet werden (z. B. Name). Eine Zeile pro Befehl.
    pub connect_text: String,
    /// Lua-Skriptdatei, die beim Öffnen der Welt geladen wird
    pub script_file: String,
    pub logging: bool,
    pub log_dir: String,
    pub log_input: bool,
    /// Zeichen zum Trennen mehrerer Befehle in einer Eingabe (leer = aus)
    pub command_stack: String,
    /// Präfix für Speedwalks, z. B. `#3n2o` (leer = aus)
    pub speedwalk_prefix: String,
    /// Präfix für direkte Lua-Ausführung in der Eingabezeile, z. B. `/`
    pub script_prefix: String,
    pub echo_input: bool,
    pub max_lines: usize,
    pub font_size: f32,
    pub auto_reconnect: bool,
    /// Beim Öffnen der Welt sofort verbinden
    pub auto_connect: bool,
    /// Mit dem Ziffernblock laufen
    pub keypad_enabled: bool,
    pub keypad: BTreeMap<String, String>,
    pub buttons: Vec<UserButton>,
    /// Kennungen der in dieser Welt aktiven Plugins
    pub plugins: Vec<String>,
    /// Variablen der Plugins (Plugin-Kennung → Name → Wert)
    pub plugin_vars: BTreeMap<String, BTreeMap<String, String>>,
    pub triggers: Vec<Trigger>,
    pub aliases: Vec<Alias>,
    pub timers: Vec<Timer>,
    pub variables: BTreeMap<String, String>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            name: "Neue Welt".into(),
            host: "mg.mud.de".into(),
            port: 4711,
            encoding: Encoding::Latin1,
            mccp: true,
            gmcp: true,
            msdp: true,
            gmcp_supports: vec!["Core 1".into(), "Char 1".into(), "Room 1".into(), "Comm 1".into()],
            connect_text: String::new(),
            script_file: String::new(),
            logging: false,
            log_dir: String::new(),
            log_input: true,
            command_stack: ";".into(),
            speedwalk_prefix: "#".into(),
            script_prefix: "/".into(),
            echo_input: true,
            max_lines: 20_000,
            font_size: 14.0,
            auto_reconnect: false,
            auto_connect: false,
            keypad_enabled: true,
            keypad: default_keypad(),
            buttons: Vec::new(),
            plugins: Vec::new(),
            plugin_vars: BTreeMap::new(),
            triggers: Vec::new(),
            aliases: Vec::new(),
            timers: Vec::new(),
            variables: BTreeMap::new(),
        }
    }
}

impl World {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Dateiname aus dem Weltnamen (ohne Sonderzeichen).
    pub fn file_name(&self) -> String {
        let slug: String = self
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("{}.toml", if slug.is_empty() { "welt".into() } else { slug })
    }
}

/// Pfad, unter dem eine Welt mit diesem Namen gespeichert würde.
pub fn path_for(world: &World) -> PathBuf {
    worlds_dir().join(world.file_name())
}

/// Freier Name für eine Kopie: „Name (Kopie)“, „Name (Kopie 2)“ …
pub fn unique_name(base: &str) -> String {
    let taken: Vec<String> = list_worlds().into_iter().map(|(_, n)| n.to_lowercase()).collect();
    if !taken.contains(&base.to_lowercase()) {
        return base.to_string();
    }
    for i in 1.. {
        let candidate = if i == 1 {
            format!("{base} (Kopie)")
        } else {
            format!("{base} (Kopie {i})")
        };
        if !taken.contains(&candidate.to_lowercase()) {
            return candidate;
        }
    }
    unreachable!()
}

/// Basisordner für Konfiguration (Welten, Logs).
pub fn config_dir() -> PathBuf {
    directories::ProjectDirs::from("de", "areadox", "rustmud")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn worlds_dir() -> PathBuf {
    config_dir().join("worlds")
}

pub fn default_log_dir() -> PathBuf {
    config_dir().join("logs")
}

/// Listet alle gespeicherten Welten.
pub fn list_worlds() -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(worlds_dir()) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "toml") {
                let name = World::load(&p).map(|w| w.name).unwrap_or_else(|_| {
                    p.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
                out.push((p, name));
            }
        }
    }
    out.sort_by_key(|a| a.1.to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut w = World::default();
        w.triggers.push(Trigger {
            name: "t".into(),
            pattern: "* kommt an.".into(),
            send: "winke %1".into(),
            ..Default::default()
        });
        w.variables.insert("ziel".into(), "ork".into());
        let s = toml::to_string_pretty(&w).unwrap();
        let back: World = toml::from_str(&s).unwrap();
        assert_eq!(back.triggers[0].send, "winke %1");
        assert_eq!(back.variables["ziel"], "ork");
    }

    #[test]
    fn partial_file_uses_defaults() {
        let w: World = toml::from_str("name = \"MG\"\nhost = \"mg.mud.de\"\nport = 23\n").unwrap();
        assert_eq!(w.port, 23);
        assert_eq!(w.command_stack, ";");
    }
}
