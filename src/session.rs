//! Eine Sitzung = eine geöffnete Welt: verbindet Netzwerk, Zeichensatz,
//! ANSI-Ausgabe, Trigger/Aliase/Timer, Lua und Logging.

use crate::ansi::{color_from_name, AnsiEvent, AnsiParser, Color, Line, Style};
use crate::automation::{parse_speedwalk, split_command_stack, substitute, Automation, Match};
use crate::logger::Logger;
use crate::net::{Connection, NetEvent};
use crate::plugins::{PluginInstance, PluginMeta};
use crate::scripting::{error_style, note_style, Action, LineHook, ScriptEngine, WinItem};
use crate::telnet::{escape_iac, naws_packet, TelnetConfig, TelnetEvent};
use crate::world::{default_log_dir, worlds_dir, Encoding, SendTo, World};
use encoding_rs::{Decoder, UTF_8, WINDOWS_1252};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_DEPTH: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    Disconnected,
    Connecting,
    Connected,
}

pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// Ein Fenster, das ein Plugin per Lua befüllt.
#[derive(Debug, Clone)]
pub struct PluginWindow {
    pub title: String,
    pub items: Vec<WinItem>,
    /// Inhalt an die Fenstergröße anpassen (z. B. Karte)
    pub fit: bool,
    pub font_size: f32,
    /// Plugin-Kennung, die das Fenster angelegt hat (None = Welt-Skript)
    pub owner: Option<String>,
}

pub struct Session {
    pub world: World,
    pub world_path: Option<PathBuf>,
    pub lines: VecDeque<Line>,
    /// Aktuelle, noch nicht abgeschlossene Zeile (z. B. Prompt ohne Zeilenumbruch)
    pub current: Line,
    pub state: ConnState,
    pub hide_input: bool,
    pub mccp: bool,
    pub gmcp_tree: Value,
    pub msdp_tree: Map<String, Value>,
    pub output_version: u64,
    pub term_size: (u16, u16),
    pub script: ScriptEngine,
    pub plugins: Vec<PluginInstance>,
    /// Fenster, die Plugins (oder das Welt-Skript) angelegt haben
    pub windows: BTreeMap<String, PluginWindow>,
    /// Fenster, die angezeigt werden sollen (WindowShow) – holt die Oberfläche ab
    pub window_requests: Vec<String>,
    conn: Option<Connection>,
    decoder: Decoder,
    ansi: AnsiParser,
    automation: Automation,
    logger: Option<Logger>,
    naws: bool,
    delayed: Vec<(Instant, String, SendTo)>,
    reconnect_at: Option<Instant>,
    user_disconnect: bool,
    waker: Waker,
}

fn make_decoder(enc: Encoding) -> Decoder {
    match enc {
        Encoding::Latin1 => WINDOWS_1252.new_decoder_without_bom_handling(),
        Encoding::Utf8 => UTF_8.new_decoder_without_bom_handling(),
    }
}

fn echo_style() -> Style {
    Style {
        fg: Color::Rgb(220, 220, 120),
        ..Style::default()
    }
}

impl Session {
    pub fn new(world: World, world_path: Option<PathBuf>, waker: Waker) -> Self {
        let script = ScriptEngine::new(world.variables.clone(), &world.name);
        let mut s = Self {
            decoder: make_decoder(world.encoding),
            world,
            world_path,
            lines: VecDeque::new(),
            current: Line::default(),
            state: ConnState::Disconnected,
            hide_input: false,
            mccp: false,
            gmcp_tree: Value::Object(Map::new()),
            msdp_tree: Map::new(),
            output_version: 0,
            term_size: (80, 24),
            script,
            plugins: Vec::new(),
            windows: BTreeMap::new(),
            window_requests: Vec::new(),
            conn: None,
            ansi: AnsiParser::new(),
            automation: Automation::default(),
            logger: None,
            naws: false,
            delayed: Vec::new(),
            reconnect_at: None,
            user_disconnect: false,
            waker,
        };
        s.rebuild_automation();
        s.note(&format!(
            "Welt „{}“ geöffnet ({}:{}).",
            s.world.name, s.world.host, s.world.port
        ));
        s.load_script();
        s.reload_plugins();
        s
    }

    // ------------------------------------------------------------ Ausgabe

    pub fn note(&mut self, text: &str) {
        self.note_styled(text, note_style());
    }

    pub fn error(&mut self, text: &str) {
        self.note_styled(text, error_style());
    }

    fn note_styled(&mut self, text: &str, style: Style) {
        for l in text.split('\n') {
            self.note_line(Line::from_text(l, style));
        }
    }

    /// Notizen werden vor einem offenen Prompt eingefügt, damit dieser unten bleibt.
    fn note_line(&mut self, line: Line) {
        if let Some(l) = self.logger.as_mut() {
            l.line(&line.plain());
        }
        self.push_output(line);
    }

    fn push_output(&mut self, line: Line) {
        self.lines.push_back(line);
        let max = self.world.max_lines.max(100);
        while self.lines.len() > max {
            self.lines.pop_front();
        }
        self.output_version += 1;
    }

    pub fn clear_output(&mut self) {
        self.lines.clear();
        self.output_version += 1;
    }

    // ------------------------------------------------------------ Einstellungen

    pub fn rebuild_automation(&mut self) {
        let w = &self.world;
        self.automation.rebuild(&w.triggers, &w.aliases, &w.timers);
        let errs = self.automation.errors.clone();
        for e in errs {
            self.error(&e);
        }
    }

    /// Neu kompilieren ohne Fehlermeldungen in der Ausgabe (für Editoren, die Fehler selbst anzeigen).
    pub fn rebuild_automation_quiet(&mut self) {
        let w = &self.world;
        self.automation.rebuild(&w.triggers, &w.aliases, &w.timers);
    }

    /// Nach Änderungen an den Welteinstellungen aufrufen.
    pub fn apply_settings(&mut self) {
        self.decoder = make_decoder(self.world.encoding);
        self.rebuild_automation();
    }

    pub fn load_script(&mut self) {
        let path = self.world.script_file.trim().to_string();
        if path.is_empty() {
            return;
        }
        self.script.load_file(&path);
        self.note(&format!("Skript geladen: {path}"));
        self.drain_actions(0);
    }

    /// Lua komplett neu starten (Variablen bleiben erhalten).
    pub fn reload_script(&mut self) {
        let vars = self.script.shared.borrow().variables.clone();
        self.script = ScriptEngine::new(vars, &self.world.name);
        self.load_script();
    }

    // ------------------------------------------------------------ Plugins

    #[cfg(test)]
    pub fn load_plugin_for_test(&mut self, meta: PluginMeta) {
        self.load_plugin(meta);
    }

    #[cfg(test)]
    pub fn feed_for_test(&mut self, data: &[u8]) {
        self.on_data(data);
    }

    #[cfg(test)]
    pub fn feed_prompt_for_test(&mut self) {
        self.on_telnet(TelnetEvent::Prompt);
    }

    #[cfg(test)]
    pub fn save_world_vars_for_test(&mut self) {
        self.store_plugin_vars();
    }

    fn store_plugin_vars(&mut self) {
        for p in &self.plugins {
            let vars = p.engine.shared.borrow().variables.clone();
            if vars.is_empty() {
                self.world.plugin_vars.remove(&p.meta.id);
            } else {
                self.world.plugin_vars.insert(p.meta.id.clone(), vars);
            }
        }
    }

    /// Lädt alle in dieser Welt aktiven Plugins (neu).
    pub fn reload_plugins(&mut self) {
        self.store_plugin_vars();
        self.plugins.clear();
        self.windows.retain(|_, w| w.owner.is_none());
        let (installed, _) = crate::plugins::list_plugins();
        for id in self.world.plugins.clone() {
            match installed.iter().find(|m| m.id == id) {
                Some(meta) => self.load_plugin(meta.clone()),
                None => self.error(&format!("Plugin „{id}“ ist aktiviert, aber nicht installiert.")),
            }
        }
    }

    fn load_plugin(&mut self, meta: PluginMeta) {
        let vars = self.world.plugin_vars.get(&meta.id).cloned().unwrap_or_default();
        let name = meta.name.clone();
        let p = PluginInstance::load(meta, vars, &self.world.name);
        self.plugins.push(p);
        self.note(&format!("Plugin geladen: {name}"));
        self.drain_actions(0);
        if self.is_connected() {
            if let Some(p) = self.plugins.last() {
                p.engine.call_if_exists("OnPluginConnect", ());
            }
            self.drain_actions(0);
        }
    }

    pub fn plugin_enabled(&self, id: &str) -> bool {
        self.world.plugins.iter().any(|p| p == id)
    }

    /// Plugin in dieser Welt ein- oder ausschalten.
    pub fn set_plugin_enabled(&mut self, meta: &PluginMeta, on: bool) {
        if on && !self.plugin_enabled(&meta.id) {
            self.world.plugins.push(meta.id.clone());
            self.load_plugin(meta.clone());
        } else if !on && self.plugin_enabled(&meta.id) {
            self.store_plugin_vars();
            if let Some(p) = self.plugins.iter().find(|p| p.meta.id == meta.id) {
                p.engine.call_if_exists("OnPluginDisable", ());
            }
            self.drain_actions(0);
            self.world.plugins.retain(|p| p != &meta.id);
            self.plugins.retain(|p| p.meta.id != meta.id);
            self.windows.retain(|_, w| w.owner.as_deref() != Some(meta.id.as_str()));
            self.note(&format!("Plugin deaktiviert: {}", meta.name));
        }
    }

    /// Ruft einen Callback in allen Plugins auf.
    fn plugins_call(&mut self, name: &str) {
        for p in &self.plugins {
            p.engine.call_if_exists(name, ());
        }
        self.drain_actions(0);
    }

    /// Fragt alle Plugins; liefert `false`, sobald eines `false` zurückgibt.
    /// Aktionen (Send, Note …) der Plugins bleiben in der Warteschlange, bis `drain_actions` läuft.
    fn plugins_allow(&mut self, callback: &str, arg: &str) -> bool {
        let mut allow = true;
        for p in &self.plugins {
            if p.engine.call_bool(callback, arg.to_string()) == Some(false) {
                allow = false;
            }
        }
        allow
    }

    pub fn save_world(&mut self) -> Result<PathBuf, String> {
        self.store_plugin_vars();
        self.world.variables = self.script.shared.borrow().variables.clone();
        let path = self
            .world_path
            .clone()
            .unwrap_or_else(|| worlds_dir().join(self.world.file_name()));
        self.world.save(&path)?;
        self.world_path = Some(path.clone());
        Ok(path)
    }

    // ------------------------------------------------------------ Verbindung

    pub fn is_connected(&self) -> bool {
        self.state == ConnState::Connected
    }

    pub fn connect(&mut self) {
        if self.state != ConnState::Disconnected {
            return;
        }
        self.user_disconnect = false;
        self.reconnect_at = None;
        self.state = ConnState::Connecting;
        self.note(&format!("Verbinde mit {}:{} …", self.world.host, self.world.port));
        let cfg = TelnetConfig {
            mccp: self.world.mccp,
            gmcp: self.world.gmcp,
            msdp: self.world.msdp,
            utf8: self.world.encoding == Encoding::Utf8,
            gmcp_supports: self.world.gmcp_supports.clone(),
            ..TelnetConfig::default()
        };
        let waker = self.waker.clone();
        self.conn = Some(Connection::open(
            self.world.host.clone(),
            self.world.port,
            cfg,
            move || waker(),
        ));
    }

    pub fn disconnect(&mut self) {
        self.user_disconnect = true;
        self.reconnect_at = None;
        if let Some(c) = &self.conn {
            c.close();
        }
    }

    /// Verarbeitet alle anstehenden Netzwerk-Ereignisse.
    pub fn poll(&mut self) {
        while let Some(ev) = self.conn.as_ref().and_then(|c| c.rx.try_recv().ok()) {
            match ev {
                NetEvent::Connected(peer) => self.on_connected(peer),
                NetEvent::Telnet(t) => self.on_telnet(t),
                NetEvent::Disconnected(reason) => {
                    self.on_disconnected(reason);
                    break;
                }
            }
        }
    }

    fn on_connected(&mut self, peer: String) {
        self.state = ConnState::Connected;
        self.hide_input = false;
        self.naws = false;
        self.gmcp_tree = Value::Object(Map::new());
        self.msdp_tree.clear();
        self.decoder = make_decoder(self.world.encoding);
        self.ansi = AnsiParser::new();
        self.note(&format!("Verbunden ({peer})."));
        if self.world.logging && self.logger.is_none() {
            self.open_logger();
        }
        self.automation.reset_timers(&self.world.timers);
        let connect_text = self.world.connect_text.clone();
        for line in connect_text.lines().filter(|l| !l.trim().is_empty()) {
            let text = {
                let sh = self.script.shared.borrow();
                substitute(line, &Match::default(), &sh.variables)
            };
            self.send_world(&text, false);
        }
        self.script.call_if_exists("OnConnect", ());
        self.drain_actions(0);
        self.plugins_call("OnPluginConnect");
        self.drain_actions(0);
    }

    fn on_disconnected(&mut self, reason: String) {
        if !self.current.is_empty() {
            self.finish_line(true);
        }
        let was_connected = self.state == ConnState::Connected;
        self.state = ConnState::Disconnected;
        self.conn = None;
        self.mccp = false;
        self.hide_input = false;
        self.error(&reason);
        if was_connected {
            self.script.call_if_exists("OnDisconnect", ());
            self.plugins_call("OnPluginDisconnect");
            self.drain_actions(0);
        }
        self.logger = None;
        if self.world.auto_reconnect && !self.user_disconnect {
            self.note("Automatisches Neuverbinden in 10 Sekunden …");
            self.reconnect_at = Some(Instant::now() + Duration::from_secs(10));
        }
    }

    fn on_telnet(&mut self, ev: TelnetEvent) {
        match ev {
            TelnetEvent::Data(bytes) => self.on_data(&bytes),
            TelnetEvent::Prompt => {
                if !self.current.is_empty() {
                    self.current.is_prompt = true;
                    self.finish_line(true);
                }
            }
            TelnetEvent::HideInput(h) => self.hide_input = h,
            TelnetEvent::Gmcp { package, data } => {
                insert_path(&mut self.gmcp_tree, &package, data.clone());
                self.script.on_gmcp(&package, &data, "OnGMCP");
                for p in &self.plugins {
                    p.engine.on_gmcp(&package, &data, "OnPluginGMCP");
                }
                self.drain_actions(0);
            }
            TelnetEvent::Msdp(vars) => {
                for (k, v) in vars {
                    self.msdp_tree.insert(k.clone(), v.clone());
                    self.script.on_msdp(&k, &v, "OnMSDP");
                    for p in &self.plugins {
                        p.engine.on_msdp(&k, &v, "OnPluginMSDP");
                    }
                }
                self.drain_actions(0);
            }
            TelnetEvent::CompressionStarted => self.mccp = true,
            TelnetEvent::CompressionEnded => self.mccp = false,
            TelnetEvent::NawsEnabled => {
                self.naws = true;
                self.send_naws();
            }
            TelnetEvent::Error(e) => self.error(&e),
        }
    }

    fn on_data(&mut self, bytes: &[u8]) {
        // decode_to_string schreibt nur in vorhandene Kapazität, daher vorher reservieren
        let cap = self
            .decoder
            .max_utf8_buffer_length(bytes.len())
            .unwrap_or(bytes.len() * 3 + 16);
        let mut text = String::with_capacity(cap);
        let _ = self.decoder.decode_to_string(bytes, &mut text, false);
        let mut events = Vec::new();
        self.ansi.feed(&text, &mut events);
        for e in events {
            match e {
                AnsiEvent::Char(c, st) => self.current.push(c, st),
                AnsiEvent::Newline => self.finish_line(true),
            }
        }
        self.output_version += 1;
    }

    /// Schließt die aktuelle Zeile ab, wertet Trigger aus und gibt sie aus.
    /// Wie bei MUSHclient erscheint zuerst die Zeile, danach die Ausgaben der Trigger.
    fn finish_line(&mut self, run_triggers: bool) {
        let mut line = std::mem::take(&mut self.current);
        let original = line.plain();
        // Plugins dürfen Zeilen ausblenden oder umschreiben (OnPluginLineReceived)
        let mut plugin_omit = false;
        if run_triggers && !self.plugins.is_empty() {
            for i in 0..self.plugins.len() {
                let (plain, ansi) = (line.plain(), line.to_ansi());
                match self.plugins[i]
                    .engine
                    .call_line_hook("OnPluginLineReceived", &plain, &ansi)
                {
                    LineHook::Keep => {}
                    LineHook::Omit => {
                        plugin_omit = true;
                        break;
                    }
                    LineHook::Replace(text) => {
                        let prompt = line.is_prompt;
                        line = Line::from_ansi(&text);
                        line.is_prompt = prompt;
                    }
                }
            }
        }
        // Von Plugins zurückgehaltene Zeilen (EmitLine) erscheinen vor der aktuellen Zeile
        if !self.plugins.is_empty() {
            let emits: Vec<Line> = self.plugins.iter().flat_map(|p| p.engine.take_emits()).collect();
            for l in emits {
                self.push_output(l);
            }
        }
        let plain = line.plain();
        let fired = if run_triggers && !plugin_omit {
            self.match_triggers(&mut line, &plain)
        } else {
            Vec::new()
        };
        let omit = plugin_omit || fired.iter().any(|(t, _)| t.omit_from_output);
        let omit_log = fired.iter().any(|(t, _)| t.omit_from_log);
        if !omit_log {
            if let Some(l) = self.logger.as_mut() {
                l.line(&original);
            }
        }
        if !omit {
            self.push_output(line);
        }
        if !self.plugins.is_empty() {
            self.drain_actions(0);
        }
        for (t, m) in fired {
            if !t.send.is_empty() {
                self.run_send(&t.send, t.send_to, &m, 0, &format!("Trigger {}", t.name));
            }
            if !t.script.trim().is_empty() {
                self.script
                    .call_handler(t.script.trim(), &t.name, &plain, &m.wildcards, &m.named);
            }
            self.drain_actions(0);
        }
    }

    /// Ermittelt passende Trigger und färbt die Zeile ggf. um.
    fn match_triggers(&mut self, line: &mut Line, plain: &str) -> Vec<(crate::world::Trigger, Match)> {
        let matches = self.automation.match_triggers(&self.world.triggers, plain);
        let mut fired = Vec::with_capacity(matches.len());
        for (idx, m) in matches {
            let t = self.world.triggers[idx].clone();
            if let Some(c) = color_from_name(&t.color).filter(|_| !t.color.trim().is_empty()) {
                for span in &mut line.spans {
                    span.style.fg = c;
                }
            }
            if t.one_shot {
                self.world.triggers[idx].enabled = false;
            }
            fired.push((t, m));
        }
        fired
    }

    // ------------------------------------------------------------ Eingabe

    /// Verarbeitet eine Eingabe des Benutzers aus der Eingabezeile.
    pub fn user_input(&mut self, input: &str) {
        if input.is_empty() {
            self.send_world("", true);
            return;
        }
        for line in input.split('\n') {
            self.execute(line.trim_end_matches('\r'), 0);
        }
    }

    /// Wie eine Eingabe: Skriptpräfix, Befehlstrenner, Speedwalk, Aliase.
    pub fn execute(&mut self, text: &str, depth: usize) {
        if depth > MAX_DEPTH {
            self.error("Zu tiefe Verschachtelung (Alias-Schleife?) – abgebrochen.");
            return;
        }
        let sp = self.world.script_prefix.clone();
        if !sp.is_empty() {
            if let Some(code) = text.strip_prefix(sp.as_str()) {
                self.script.run(code, "Eingabe");
                self.drain_actions(depth + 1);
                return;
            }
        }
        let sep = self.world.command_stack.chars().next();
        for cmd in split_command_stack(text, sep) {
            self.execute_single(&cmd, depth);
        }
    }

    fn execute_single(&mut self, cmd: &str, depth: usize) {
        // Plugins dürfen Befehle abfangen (OnPluginCommand → false)
        if !self.plugins.is_empty() {
            let allow = self.plugins_allow("OnPluginCommand", cmd);
            self.drain_actions(depth + 1);
            if !allow {
                return;
            }
        }
        let swp = self.world.speedwalk_prefix.clone();
        if !swp.is_empty() {
            if let Some(walk) = cmd.strip_prefix(swp.as_str()) {
                match parse_speedwalk(walk) {
                    Ok(dirs) => {
                        for d in dirs {
                            self.send_world(&d, true);
                        }
                    }
                    Err(e) => self.error(&format!("Speedwalk: {e}")),
                }
                return;
            }
        }
        let matches = self.automation.match_aliases(&self.world.aliases, cmd);
        if matches.is_empty() {
            self.send_world(cmd, true);
            return;
        }
        for (idx, m) in matches {
            let a = self.world.aliases[idx].clone();
            if !a.send.is_empty() {
                let to = a.send_to;
                if to == SendTo::World && a.omit_from_output {
                    let text = self.expand(&a.send, &m);
                    for l in text.lines() {
                        self.send_world(l, false);
                    }
                } else {
                    self.run_send(&a.send, to, &m, depth, &format!("Alias {}", a.name));
                }
            }
            if !a.script.trim().is_empty() {
                self.script
                    .call_handler(a.script.trim(), &a.name, cmd, &m.wildcards, &m.named);
            }
            self.drain_actions(depth + 1);
        }
    }

    fn expand(&self, template: &str, m: &Match) -> String {
        let sh = self.script.shared.borrow();
        substitute(template, m, &sh.variables)
    }

    fn run_send(&mut self, template: &str, to: SendTo, m: &Match, depth: usize, chunk: &str) {
        let text = self.expand(template, m);
        match to {
            SendTo::World => {
                for l in text.lines() {
                    self.send_world(l, true);
                }
            }
            SendTo::Output => self.note(&text),
            SendTo::Execute => {
                for l in text.lines() {
                    self.execute(l, depth + 1);
                }
            }
            SendTo::Script => {
                self.script.run_with_matches(&text, chunk, &m.wildcards, &m.named);
                self.drain_actions(depth + 1);
            }
        }
    }

    /// Sendet eine Zeile an den MUD.
    pub fn send_world(&mut self, text: &str, echo: bool) {
        let Some(conn) = &self.conn else {
            self.error("Nicht verbunden.");
            return;
        };
        if self.state != ConnState::Connected {
            self.error("Verbindung wird noch aufgebaut.");
            return;
        }
        let mut bytes = match self.world.encoding {
            Encoding::Latin1 => WINDOWS_1252.encode(text).0.into_owned(),
            Encoding::Utf8 => text.as_bytes().to_vec(),
        };
        bytes = escape_iac(&bytes);
        bytes.extend_from_slice(b"\r\n");
        if let Err(e) = conn.send_raw(&bytes) {
            self.error(&format!("Senden fehlgeschlagen: {e}"));
            return;
        }
        if self.hide_input {
            // Passwort nie anzeigen oder loggen, aber Prompt-Zeile abschließen
            if !self.current.is_empty() {
                self.finish_line(false);
            }
        } else if echo && self.world.echo_input {
            self.current.push_str(text, echo_style());
            let line = std::mem::take(&mut self.current);
            if self.world.log_input || line.plain() != text {
                if let Some(l) = self.logger.as_mut() {
                    l.line(&line.plain());
                }
            }
            self.push_output(line);
        } else if !self.current.is_empty() {
            self.finish_line(false);
        }
    }

    pub fn send_raw(&mut self, data: &[u8]) {
        if let Some(c) = &self.conn {
            if let Err(e) = c.send_raw(data) {
                self.error(&format!("Senden fehlgeschlagen: {e}"));
            }
        }
    }

    pub fn set_term_size(&mut self, w: u16, h: u16) {
        if (w, h) != self.term_size {
            self.term_size = (w, h);
            self.send_naws();
        }
    }

    fn send_naws(&mut self) {
        if self.naws && self.is_connected() {
            let p = naws_packet(self.term_size.0, self.term_size.1);
            self.send_raw(&p);
        }
    }

    /// Ziffernblock-Taste ("8", "+", …) gemäß Belegung der Welt ausführen.
    pub fn keypad(&mut self, key: &str) {
        if !self.world.keypad_enabled {
            return;
        }
        if let Some(cmd) = self.world.keypad.get(key).cloned() {
            if !cmd.trim().is_empty() {
                self.execute(&cmd, 0);
            }
        }
    }

    /// Eigenen Knopf aus der Icon-Leiste auslösen.
    pub fn run_button(&mut self, idx: usize) {
        if let Some(b) = self.world.buttons.get(idx).cloned() {
            if !b.send.is_empty() {
                self.run_send(&b.send, b.send_to, &Match::default(), 0, &format!("Knopf {}", b.label));
            }
        }
    }

    pub fn is_logging(&self) -> bool {
        self.logger.is_some()
    }

    /// Log sofort ein- oder ausschalten (und als Welteinstellung merken).
    pub fn set_logging(&mut self, on: bool) {
        self.world.logging = on;
        if on && self.logger.is_none() {
            self.open_logger();
        } else if !on && self.logger.is_some() {
            self.logger = None;
            self.note("Log beendet.");
        }
    }

    fn open_logger(&mut self) {
        let dir = if self.world.log_dir.trim().is_empty() {
            default_log_dir()
        } else {
            PathBuf::from(self.world.log_dir.trim())
        };
        match Logger::open(&dir, &self.world.name) {
            Ok(l) => {
                self.note(&format!("Log: {}", l.path.display()));
                self.logger = Some(l);
            }
            Err(e) => self.error(&format!("Log konnte nicht geöffnet werden: {e}")),
        }
    }

    // ------------------------------------------------------------ Aktionen & Timer

    fn drain_actions(&mut self, depth: usize) {
        for _ in 0..1000 {
            let mut actions: Vec<(Option<String>, Action)> =
                self.script.take_actions().into_iter().map(|a| (None, a)).collect();
            for p in &self.plugins {
                actions.extend(
                    p.engine
                        .take_actions()
                        .into_iter()
                        .map(|a| (Some(p.meta.id.clone()), a)),
                );
            }
            if actions.is_empty() {
                return;
            }
            for (owner, a) in actions {
                match a {
                    Action::WindowCreate(id, title) => {
                        let w = self.windows.entry(id).or_insert_with(|| PluginWindow {
                            title: title.clone(),
                            items: Vec::new(),
                            fit: false,
                            font_size: 14.0,
                            owner: owner.clone(),
                        });
                        w.title = title;
                        w.owner = owner;
                        self.output_version += 1;
                    }
                    Action::WindowClear(id) => {
                        if let Some(w) = self.windows.get_mut(&id) {
                            w.items.clear();
                        }
                    }
                    Action::WindowItem(id, item) => {
                        if let Some(w) = self.windows.get_mut(&id) {
                            w.items.push(item);
                            self.output_version += 1;
                        }
                    }
                    Action::WindowShow(id) => {
                        if self.windows.contains_key(&id) && !self.window_requests.contains(&id) {
                            self.window_requests.push(id);
                        }
                    }
                    Action::WindowOption(id, key, value) => {
                        if let Some(w) = self.windows.get_mut(&id) {
                            match key.as_str() {
                                "fit" => w.fit = value == "true" || value == "1",
                                "fontsize" => w.font_size = value.parse().unwrap_or(w.font_size).clamp(6.0, 60.0),
                                _ => {}
                            }
                        }
                    }
                    Action::EmitLine(l) => self.push_output(l),
                    Action::Send(t) => {
                        for l in t.lines() {
                            self.send_world(l, true);
                        }
                    }
                    Action::Execute(t) => self.execute(&t, depth + 1),
                    Action::Note(l) => self.note_line(l),
                    Action::Raw(b) => self.send_raw(&b),
                    Action::EnableTrigger(n, on) => self
                        .world
                        .triggers
                        .iter_mut()
                        .filter(|t| t.name == n)
                        .for_each(|t| t.enabled = on),
                    Action::EnableAlias(n, on) => self
                        .world
                        .aliases
                        .iter_mut()
                        .filter(|a| a.name == n)
                        .for_each(|a| a.enabled = on),
                    Action::EnableTimer(n, on) => self
                        .world
                        .timers
                        .iter_mut()
                        .filter(|t| t.name == n)
                        .for_each(|t| t.enabled = on),
                    Action::EnableGroup(g, on) => {
                        self.world
                            .triggers
                            .iter_mut()
                            .filter(|t| t.group == g)
                            .for_each(|t| t.enabled = on);
                        self.world
                            .aliases
                            .iter_mut()
                            .filter(|a| a.group == g)
                            .for_each(|a| a.enabled = on);
                        self.world
                            .timers
                            .iter_mut()
                            .filter(|t| t.group == g)
                            .for_each(|t| t.enabled = on);
                    }
                    Action::DoAfter(secs, text, to) => {
                        self.delayed
                            .push((Instant::now() + Duration::from_secs_f64(secs.max(0.0)), text, to));
                    }
                    Action::Connect => self.connect(),
                    Action::Disconnect => self.disconnect(),
                }
            }
        }
        self.error("Zu viele Skriptaktionen – abgebrochen.");
    }

    /// Regelmäßig aufrufen (Timer, DoAfter, Neuverbinden). Liefert die Zeit bis zum nächsten Aufruf.
    pub fn tick(&mut self) -> Duration {
        let connected = self.is_connected();
        let due = self.automation.due_timers(&mut self.world.timers, connected);
        for i in due {
            let t = self.world.timers[i].clone();
            if !t.send.is_empty() {
                self.run_send(&t.send, t.send_to, &Match::default(), 0, &format!("Timer {}", t.name));
            }
            if !t.script.trim().is_empty() {
                self.script
                    .call_handler(t.script.trim(), &t.name, "", &[], &Default::default());
                self.drain_actions(0);
            }
        }
        if !self.plugins.is_empty() {
            for p in &self.plugins {
                p.engine.call_if_exists("OnPluginTick", ());
            }
            self.drain_actions(0);
        }
        let now = Instant::now();
        let mut ready = Vec::new();
        self.delayed.retain(|(at, text, to)| {
            if *at <= now {
                ready.push((text.clone(), *to));
                false
            } else {
                true
            }
        });
        for (text, to) in ready {
            self.run_send(&text, to, &Match::default(), 0, "DoAfter");
        }
        if let Some(at) = self.reconnect_at {
            if at <= now {
                self.reconnect_at = None;
                self.connect();
            }
        }
        if let Some(l) = self.logger.as_mut() {
            l.flush();
        }
        Duration::from_millis(100)
    }
}

/// Fügt `data` unter dem Punkt-Pfad (`Char.Vitals`) in ein JSON-Objekt ein.
fn insert_path(root: &mut Value, path: &str, data: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut cur = root;
    for p in &parts[..parts.len().saturating_sub(1)] {
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        cur = cur
            .as_object_mut()
            .unwrap()
            .entry(p.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    if !cur.is_object() {
        *cur = Value::Object(Map::new());
    }
    if let Some(last) = parts.last() {
        cur.as_object_mut().unwrap().insert(last.to_string(), data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_gmcp_path() {
        let mut v = Value::Object(Map::new());
        insert_path(&mut v, "Char.Vitals", serde_json::json!({"hp": 1}));
        insert_path(&mut v, "Char.Status", serde_json::json!("ok"));
        assert_eq!(v["Char"]["Vitals"]["hp"], 1);
        assert_eq!(v["Char"]["Status"], "ok");
    }

    #[test]
    fn latin1_decoding_and_triggers() {
        let mut w = World::default();
        w.triggers.push(crate::world::Trigger {
            pattern: "* grüßt dich.".into(),
            send: "Note('Gruß von %1')".into(),
            send_to: SendTo::Script,
            ..Default::default()
        });
        let mut s = Session::new(w, None, Arc::new(|| {}));
        let before = s.lines.len();
        // "Bob grüßt dich.\r\n" in Latin-1
        s.on_data(b"Bob gr\xfc\xdft dich.\r\n");
        let texts: Vec<String> = s.lines.iter().skip(before).map(|l| l.plain()).collect();
        assert_eq!(texts, vec!["Bob grüßt dich.", "Gruß von Bob"]);
    }

    #[test]
    fn plugin_hooks() {
        let dir = std::env::temp_dir().join(format!("rustmud-plg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("main.lua"),
            r#"
            function OnPluginLineReceived(z)
              if z:find("Werbung") then return false end
              if z:find("LP: (%d+)") and tonumber(z:match("LP: (%d+)")) < 30 then Note("Achtung, LP niedrig!") end
              return true
            end
            function OnPluginCommand(c)
              if c == "zaehle" then SetVariable("n", (tonumber(GetVariable("n")) or 0) + 1); Note("gezählt"); return false end
              return true
            end
            "#,
        )
        .unwrap();
        let meta = crate::plugins::PluginMeta {
            id: "t".into(),
            name: "T".into(),
            dir: dir.clone(),
            ..Default::default()
        };
        let mut s = Session::new(World::default(), None, Arc::new(|| {}));
        s.world.plugins.push("t".into());
        s.load_plugin(meta);
        let before = s.lines.len();
        s.on_data(b"Werbung: kauft Schwerter!\r\nLP: 12\r\n");
        s.user_input("zaehle");
        let texts: Vec<String> = s.lines.iter().skip(before).map(|l| l.plain()).collect();
        assert_eq!(texts, vec!["LP: 12", "Achtung, LP niedrig!", "gezählt"]);
        s.save_world_vars_for_test();
        assert_eq!(s.world.plugin_vars["t"]["n"], "1");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn utf8_split_multibyte() {
        let w = World {
            encoding: Encoding::Utf8,
            ..World::default()
        };
        let mut s = Session::new(w, None, Arc::new(|| {}));
        let bytes = "Tür\n".as_bytes();
        s.on_data(&bytes[..2]);
        s.on_data(&bytes[2..]);
        assert_eq!(s.lines.back().unwrap().plain(), "Tür");
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::telnet::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Kompletter Durchlauf gegen einen Fake-MUD auf localhost:
    /// Verhandlung, GMCP, MCCP, Latin-1, Prompt, Alias und Trigger.
    #[test]
    fn fake_mud_roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut c, _) = listener.accept().unwrap();
            c.write_all(&[IAC, WILL, OPT_GMCP, IAC, WILL, OPT_MCCP2, IAC, DO, OPT_NAWS])
                .unwrap();
            std::thread::sleep(Duration::from_millis(150));
            let mut gmcp = vec![IAC, SB, OPT_GMCP];
            gmcp.extend_from_slice(br#"Char.Vitals {"lp": 100}"#);
            gmcp.extend_from_slice(&[IAC, SE, IAC, SB, OPT_MCCP2, IAC, SE]);
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
            enc.write_all(b"Willkommen in M\xfcnchen!\r\nEin Ork kommt herein.\r\nWas nun? ")
                .unwrap();
            enc.write_all(&[IAC, GA]).unwrap();
            enc.flush().unwrap();
            gmcp.extend_from_slice(enc.get_ref());
            c.write_all(&gmcp).unwrap();
            // Alles lesen, was der Client schickt, bis er „ende“ sendet
            c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 1024];
            while !String::from_utf8_lossy(&got).contains("ende") {
                match c.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            }
            got
        });

        let mut w = World {
            host: "127.0.0.1".into(),
            port,
            ..World::default()
        };
        w.triggers.push(crate::world::Trigger {
            pattern: "Ein * kommt herein.".into(),
            send: "toete %1".into(),
            ..Default::default()
        });
        w.aliases.push(crate::world::Alias {
            pattern: "gr *".into(),
            send: "sage Grüße, %1!".into(),
            ..Default::default()
        });
        let mut s = Session::new(w, None, Arc::new(|| {}));
        s.connect();
        let start = Instant::now();
        while !s.lines.iter().any(|l| l.plain().starts_with("Was nun?")) {
            s.poll();
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "Zeitüberschreitung: {:?}",
                s.lines
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(s.mccp, "MCCP sollte aktiv sein");
        assert_eq!(s.gmcp_tree["Char"]["Vitals"]["lp"], 100);
        let texts: Vec<String> = s.lines.iter().map(|l| l.plain()).collect();
        assert!(texts.contains(&"Willkommen in München!".to_string()), "{texts:?}");
        s.user_input("gr Ork;ende");
        std::thread::sleep(Duration::from_millis(200));
        s.disconnect();
        let got = server.join().unwrap();
        // GMCP-Hello, DO MCCP, WILL NAWS, NAWS-Größe, Trigger, Alias (Latin-1), Befehlstrenner
        assert!(got.windows(3).any(|w| w == [IAC, DO, OPT_MCCP2]));
        assert!(got.windows(3).any(|w| w == [IAC, WILL, OPT_NAWS]));
        let text = encoding_rs::WINDOWS_1252.decode(&got).0.to_string();
        assert!(text.contains("Core.Hello"), "{text}");
        assert!(text.contains("toete Ork\r\n"), "{text}");
        assert!(text.contains("sage Grüße, Ork!\r\n"), "{text}");
        assert!(text.contains("ende\r\n"), "{text}");
    }
}
