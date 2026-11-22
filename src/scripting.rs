//! Lua-Scripting mit MUSHclient-ähnlicher API.
//!
//! Lua-Funktionen greifen nicht direkt auf die Sitzung zu, sondern legen
//! Aktionen in eine Warteschlange, die die Sitzung danach abarbeitet.

use crate::ansi::{color_from_name, Color, Line, Style};
use crate::world::SendTo;
use mlua::{Function, Lua, LuaSerdeExt, MultiValue, Table, Value as LuaValue, Variadic};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Inhalt eines Plugin-Fensters.
#[derive(Debug, Clone)]
pub enum WinItem {
    Line(Line),
    Heading(String),
    Gauge {
        label: String,
        value: f64,
        max: f64,
        color: Color,
    },
    Pair(String, String),
    Separator,
}

/// Ergebnis von OnPluginLineReceived.
#[derive(Debug, Clone, PartialEq)]
pub enum LineHook {
    Keep,
    Omit,
    Replace(String),
}

#[derive(Debug, Clone)]
pub enum Action {
    WindowCreate(String, String),
    WindowClear(String),
    WindowItem(String, WinItem),
    WindowShow(String),
    WindowOption(String, String, String),
    /// Zeile in der Ausgabe anzeigen, ohne Log und Trigger (z. B. zurückgehaltene Zeilen)
    EmitLine(Line),
    /// Die nächste Ausgabezeile beginnt eine neue Seite (Seitenansicht)
    PageStart,
    Send(String),
    Execute(String),
    Note(Line),
    Raw(Vec<u8>),
    EnableTrigger(String, bool),
    EnableAlias(String, bool),
    EnableTimer(String, bool),
    EnableGroup(String, bool),
    DoAfter(f64, String, SendTo),
    Connect,
    Disconnect,
}

#[derive(Default)]
pub struct Shared {
    pub actions: Vec<Action>,
    pub variables: BTreeMap<String, String>,
    tell: Line,
}

pub struct ScriptEngine {
    lua: Lua,
    pub shared: Rc<RefCell<Shared>>,
}

pub fn note_style() -> Style {
    Style {
        fg: Color::Rgb(0, 200, 200),
        ..Style::default()
    }
}

pub fn error_style() -> Style {
    Style {
        fg: Color::Rgb(255, 80, 80),
        ..Style::default()
    }
}

impl ScriptEngine {
    pub fn new(variables: BTreeMap<String, String>, world_name: &str) -> Self {
        let lua = Lua::new();
        let shared = Rc::new(RefCell::new(Shared {
            variables,
            ..Default::default()
        }));
        let engine = Self { lua, shared };
        if let Err(e) = engine.register(world_name) {
            engine.push(Action::Note(Line::from_text(
                &format!("Lua-Initialisierung fehlgeschlagen: {e}"),
                error_style(),
            )));
        }
        engine
    }

    fn push(&self, a: Action) {
        self.shared.borrow_mut().actions.push(a);
    }

    /// Nimmt Aktionen, die vor der aktuellen Zeile wirken müssen (EmitLine, PageStart), aus der Warteschlange.
    pub fn take_emits(&self) -> Vec<Action> {
        let mut sh = self.shared.borrow_mut();
        let mut out = Vec::new();
        sh.actions.retain(|a| match a {
            Action::EmitLine(_) | Action::PageStart => {
                out.push(a.clone());
                false
            }
            _ => true,
        });
        out
    }

    pub fn take_actions(&self) -> Vec<Action> {
        std::mem::take(&mut self.shared.borrow_mut().actions)
    }

    fn register(&self, world_name: &str) -> mlua::Result<()> {
        let lua = &self.lua;
        let g = lua.globals();
        let world = lua.create_table()?;

        macro_rules! func {
            ($name:expr, $f:expr) => {{
                let f = lua.create_function($f)?;
                g.set($name, f.clone())?;
                world.set($name, f)?;
            }};
        }

        let s = self.shared.clone();
        func!("Send", move |_, args: Variadic<String>| {
            s.borrow_mut().actions.push(Action::Send(args.concat()));
            Ok(())
        });
        let s = self.shared.clone();
        func!("Execute", move |_, text: String| {
            s.borrow_mut().actions.push(Action::Execute(text));
            Ok(())
        });
        let s = self.shared.clone();
        func!("Note", move |_, args: Variadic<LuaValue>| {
            let text = join_values(&args);
            let mut sh = s.borrow_mut();
            let mut line = std::mem::take(&mut sh.tell);
            line.push_str(&text, note_style());
            sh.actions.push(Action::Note(line));
            Ok(())
        });
        let s = self.shared.clone();
        func!("Tell", move |_, args: Variadic<LuaValue>| {
            let text = join_values(&args);
            s.borrow_mut().tell.push_str(&text, note_style());
            Ok(())
        });
        let s = self.shared.clone();
        func!("ColourNote", move |_, args: Variadic<String>| {
            let mut sh = s.borrow_mut();
            let mut line = std::mem::take(&mut sh.tell);
            push_colour_args(&mut line, &args);
            sh.actions.push(Action::Note(line));
            Ok(())
        });
        let s = self.shared.clone();
        func!("ColourTell", move |_, args: Variadic<String>| {
            push_colour_args(&mut s.borrow_mut().tell, &args);
            Ok(())
        });
        let s = self.shared.clone();
        func!("GetVariable", move |_, name: String| Ok(s
            .borrow()
            .variables
            .get(&name)
            .cloned()));
        let s = self.shared.clone();
        func!("SetVariable", move |_, (name, value): (String, LuaValue)| {
            let v = match value {
                LuaValue::String(s) => s.to_str()?.to_string(),
                LuaValue::Nil => String::new(),
                other => other.to_string()?,
            };
            s.borrow_mut().variables.insert(name, v);
            Ok(())
        });
        let s = self.shared.clone();
        func!("DeleteVariable", move |_, name: String| {
            s.borrow_mut().variables.remove(&name);
            Ok(())
        });
        let s = self.shared.clone();
        func!("GetVariableList", move |lua, ()| {
            let t = lua.create_table()?;
            for (k, v) in &s.borrow().variables {
                t.set(k.as_str(), v.as_str())?;
            }
            Ok(t)
        });
        let s = self.shared.clone();
        func!("EnableTrigger", move |_, (name, on): (String, Option<bool>)| {
            s.borrow_mut()
                .actions
                .push(Action::EnableTrigger(name, on.unwrap_or(true)));
            Ok(())
        });
        let s = self.shared.clone();
        func!("EnableAlias", move |_, (name, on): (String, Option<bool>)| {
            s.borrow_mut()
                .actions
                .push(Action::EnableAlias(name, on.unwrap_or(true)));
            Ok(())
        });
        let s = self.shared.clone();
        func!("EnableTimer", move |_, (name, on): (String, Option<bool>)| {
            s.borrow_mut()
                .actions
                .push(Action::EnableTimer(name, on.unwrap_or(true)));
            Ok(())
        });
        let s = self.shared.clone();
        func!("EnableGroup", move |_, (name, on): (String, Option<bool>)| {
            s.borrow_mut()
                .actions
                .push(Action::EnableGroup(name, on.unwrap_or(true)));
            Ok(())
        });
        let s = self.shared.clone();
        func!("DoAfter", move |_, (secs, text): (f64, String)| {
            s.borrow_mut().actions.push(Action::DoAfter(secs, text, SendTo::World));
            Ok(())
        });
        let s = self.shared.clone();
        func!("DoAfterNote", move |_, (secs, text): (f64, String)| {
            s.borrow_mut().actions.push(Action::DoAfter(secs, text, SendTo::Output));
            Ok(())
        });
        let s = self.shared.clone();
        func!("DoAfterSpecial", move |_, (secs, text, target): (f64, String, i32)| {
            // MUSHclient: 0 = World, 2 = Output, 10 = Execute, 12 = Script
            let to = match target {
                2 => SendTo::Output,
                10 => SendTo::Execute,
                12 => SendTo::Script,
                _ => SendTo::World,
            };
            s.borrow_mut().actions.push(Action::DoAfter(secs, text, to));
            Ok(())
        });
        let s = self.shared.clone();
        func!("SendGMCP", move |_, (package, data): (String, Option<String>)| {
            s.borrow_mut()
                .actions
                .push(Action::Raw(crate::telnet::gmcp_packet(&package, data.as_deref())));
            Ok(())
        });
        let s = self.shared.clone();
        func!("SendMSDP", move |_, (var, vals): (String, Variadic<String>)| {
            let v: Vec<&str> = vals.iter().map(String::as_str).collect();
            s.borrow_mut()
                .actions
                .push(Action::Raw(crate::telnet::msdp_packet(&var, &v)));
            Ok(())
        });
        let s = self.shared.clone();
        func!("Connect", move |_, ()| {
            s.borrow_mut().actions.push(Action::Connect);
            Ok(())
        });
        let s = self.shared.clone();
        func!("Disconnect", move |_, ()| {
            s.borrow_mut().actions.push(Action::Disconnect);
            Ok(())
        });
        let wn = world_name.to_string();
        func!("WorldName", move |_, ()| Ok(wn.clone()));

        // ---- ANSI-Hilfen
        func!("AnsiStrip", |_, s: String| Ok(Line::from_ansi(&s).plain()));
        func!("AnsiLen", |_, s: String| Ok(Line::from_ansi(&s).char_len()));
        func!("AnsiSub", |_, (s, start, len): (String, i64, Option<i64>)| {
            // 1-basierte Zeichenposition wie string.sub, Farben bleiben erhalten
            let line = Line::from_ansi(&s);
            let total = line.char_len() as i64;
            let start = if start < 1 { 1 } else { start };
            let end = match len {
                Some(l) => (start - 1 + l.max(0)).min(total),
                None => total,
            };
            if start > total || end < start {
                return Ok(String::new());
            }
            let (_, right) = line.split_at_char((start - 1) as usize);
            let (mid, _) = right.split_at_char((end - start + 1) as usize);
            Ok(mid.to_ansi())
        });
        let s = self.shared.clone();
        func!("AnsiNote", move |_, text: String| {
            s.borrow_mut().actions.push(Action::Note(Line::from_ansi(&text)));
            Ok(())
        });
        let s = self.shared.clone();
        func!("SetPageStart", move |_, ()| {
            s.borrow_mut().actions.push(Action::PageStart);
            Ok(())
        });
        let s = self.shared.clone();
        func!("EmitLine", move |_, text: String| {
            s.borrow_mut().actions.push(Action::EmitLine(Line::from_ansi(&text)));
            Ok(())
        });

        // ---- Fenster
        let s = self.shared.clone();
        func!("WindowCreate", move |_, (id, title): (String, Option<String>)| {
            let t = title.unwrap_or_else(|| id.clone());
            s.borrow_mut().actions.push(Action::WindowCreate(id, t));
            Ok(())
        });
        let s = self.shared.clone();
        func!("WindowClear", move |_, id: String| {
            s.borrow_mut().actions.push(Action::WindowClear(id));
            Ok(())
        });
        let s = self.shared.clone();
        func!("WindowShow", move |_, id: String| {
            s.borrow_mut().actions.push(Action::WindowShow(id));
            Ok(())
        });
        let s = self.shared.clone();
        func!("WindowAddLine", move |_, (id, text): (String, String)| {
            s.borrow_mut()
                .actions
                .push(Action::WindowItem(id, WinItem::Line(Line::from_ansi(&text))));
            Ok(())
        });
        let s = self.shared.clone();
        func!("WindowHeading", move |_, (id, text): (String, String)| {
            s.borrow_mut()
                .actions
                .push(Action::WindowItem(id, WinItem::Heading(text)));
            Ok(())
        });
        let s = self.shared.clone();
        func!("WindowSeparator", move |_, id: String| {
            s.borrow_mut().actions.push(Action::WindowItem(id, WinItem::Separator));
            Ok(())
        });
        let s = self.shared.clone();
        func!("WindowPair", move |_, (id, k, v): (String, String, LuaValue)| {
            let v = match v {
                LuaValue::String(x) => x.to_string_lossy().to_string(),
                LuaValue::Nil => String::new(),
                other => other.to_string()?,
            };
            s.borrow_mut().actions.push(Action::WindowItem(id, WinItem::Pair(k, v)));
            Ok(())
        });
        let s = self.shared.clone();
        func!("WindowGauge", move |_,
                                   (id, label, value, max, colour): (
            String,
            String,
            f64,
            f64,
            Option<String>
        )| {
            let color = colour
                .as_deref()
                .and_then(color_from_name)
                .unwrap_or(Color::Rgb(90, 160, 90));
            s.borrow_mut().actions.push(Action::WindowItem(
                id,
                WinItem::Gauge {
                    label,
                    value,
                    max,
                    color,
                },
            ));
            Ok(())
        });
        let s = self.shared.clone();
        func!(
            "WindowOption",
            move |_, (id, key, value): (String, String, LuaValue)| {
                let v = match value {
                    LuaValue::String(x) => x.to_string_lossy().to_string(),
                    other => other.to_string()?,
                };
                s.borrow_mut().actions.push(Action::WindowOption(id, key, v));
                Ok(())
            }
        );

        // print() landet als Notiz in der Ausgabe
        let s = self.shared.clone();
        g.set(
            "print",
            lua.create_function(move |_, args: Variadic<LuaValue>| {
                let text = join_values(&args);
                s.borrow_mut()
                    .actions
                    .push(Action::Note(Line::from_text(&text, note_style())));
                Ok(())
            })?,
        )?;

        g.set("world", world)?;
        g.set("gmcp", lua.create_table()?)?;
        g.set("msdp", lua.create_table()?)?;
        Ok(())
    }

    fn report<T>(&self, r: mlua::Result<T>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                for l in format!("Lua-Fehler: {e}").lines() {
                    self.push(Action::Note(Line::from_text(l, error_style())));
                }
                None
            }
        }
    }

    pub fn run(&self, code: &str, chunk_name: &str) {
        let r = self.lua.load(code).set_name(chunk_name).exec();
        self.report(r);
    }

    pub fn load_file(&self, path: &str) {
        match std::fs::read_to_string(path) {
            Ok(code) => self.run(&code, path),
            Err(e) => self.push(Action::Note(Line::from_text(
                &format!("Skript {path} konnte nicht geladen werden: {e}"),
                error_style(),
            ))),
        }
    }

    /// Für Plugins: GetPluginID(), GetPluginName(), GetPluginDirectory().
    pub fn set_plugin_info(&self, id: &str, name: &str, dir: &str) {
        let r = (|| -> mlua::Result<()> {
            let g = self.lua.globals();
            let (id, name, dir) = (id.to_string(), name.to_string(), dir.to_string());
            g.set("GetPluginID", self.lua.create_function(move |_, ()| Ok(id.clone()))?)?;
            g.set(
                "GetPluginName",
                self.lua.create_function(move |_, ()| Ok(name.clone()))?,
            )?;
            g.set(
                "GetPluginDirectory",
                self.lua.create_function(move |_, ()| Ok(dir.clone()))?,
            )?;
            Ok(())
        })();
        self.report(r);
    }

    /// Ruft eine globale Funktion auf und liefert ihren Rückgabewert, falls er ein Wahrheitswert ist.
    /// `None`, wenn die Funktion fehlt, fehlschlägt oder nichts Boolesches zurückgibt.
    pub fn call_bool(&self, name: &str, args: impl mlua::IntoLuaMulti) -> Option<bool> {
        let f = match self.lua.globals().get::<LuaValue>(name) {
            Ok(LuaValue::Function(f)) => f,
            _ => return None,
        };
        match self.report(f.call::<LuaValue>(args))? {
            LuaValue::Boolean(b) => Some(b),
            _ => None,
        }
    }

    /// OnPluginLineReceived(zeile, ansi): false = ausblenden, Text = Zeile ersetzen.
    pub fn call_line_hook(&self, name: &str, plain: &str, ansi: &str) -> LineHook {
        let f = match self.lua.globals().get::<LuaValue>(name) {
            Ok(LuaValue::Function(f)) => f,
            _ => return LineHook::Keep,
        };
        match self.report(f.call::<LuaValue>((plain.to_string(), ansi.to_string()))) {
            Some(LuaValue::Boolean(false)) => LineHook::Omit,
            Some(LuaValue::String(s)) => {
                let s = s.to_string_lossy().to_string();
                if s.is_empty() {
                    LineHook::Omit
                } else {
                    LineHook::Replace(s)
                }
            }
            _ => LineHook::Keep,
        }
    }

    /// Ruft eine globale Funktion auf, falls vorhanden.
    pub fn call_if_exists(&self, name: &str, args: impl mlua::IntoLuaMulti) -> bool {
        let f: mlua::Result<LuaValue> = self.lua.globals().get(name);
        match f {
            Ok(LuaValue::Function(f)) => {
                let r: mlua::Result<MultiValue> = f.call(args);
                self.report(r);
                true
            }
            _ => false,
        }
    }

    /// Ruft eine Trigger-/Alias-Funktion im MUSHclient-Stil auf: f(name, zeile, wildcards).
    pub fn call_handler(
        &self,
        func: &str,
        name: &str,
        line: &str,
        wildcards: &[String],
        named: &BTreeMap<String, String>,
    ) {
        let r = (|| -> mlua::Result<()> {
            let f: Function = match self.lua.globals().get::<LuaValue>(func)? {
                LuaValue::Function(f) => f,
                _ => return Err(mlua::Error::runtime(format!("Funktion „{func}“ nicht gefunden"))),
            };
            let wc = self.wildcard_table(wildcards, named)?;
            f.call::<()>((name, line, wc))
        })();
        self.report(r);
    }

    /// Führt Lua-Code mit gesetzter `matches`/`wildcards`-Tabelle aus.
    pub fn run_with_matches(&self, code: &str, chunk: &str, wildcards: &[String], named: &BTreeMap<String, String>) {
        let r = (|| -> mlua::Result<()> {
            let wc = self.wildcard_table(wildcards, named)?;
            self.lua.globals().set("matches", wc.clone())?;
            self.lua.globals().set("wildcards", wc)?;
            self.lua.load(code).set_name(chunk).exec()
        })();
        self.report(r);
    }

    fn wildcard_table(&self, wildcards: &[String], named: &BTreeMap<String, String>) -> mlua::Result<Table> {
        let t = self.lua.create_table()?;
        for (i, w) in wildcards.iter().enumerate() {
            t.set(i, w.as_str())?;
        }
        for (k, v) in named {
            t.set(k.as_str(), v.as_str())?;
        }
        Ok(t)
    }

    /// Schreibt GMCP-Daten in die verschachtelte Tabelle `gmcp` und ruft `OnGMCP(paket)` auf.
    pub fn on_gmcp(&self, package: &str, data: &serde_json::Value, callback: &str) {
        let r = (|| -> mlua::Result<()> {
            let mut table: Table = self.lua.globals().get("gmcp")?;
            let parts: Vec<&str> = package.split('.').collect();
            for p in &parts[..parts.len().saturating_sub(1)] {
                let next = match table.get::<LuaValue>(*p)? {
                    LuaValue::Table(t) => t,
                    _ => {
                        let t = self.lua.create_table()?;
                        table.set(*p, t.clone())?;
                        t
                    }
                };
                table = next;
            }
            if let Some(last) = parts.last() {
                table.set(*last, self.lua.to_value(data)?)?;
            }
            Ok(())
        })();
        self.report(r);
        self.call_if_exists(callback, (package.to_string(), data.to_string()));
    }

    pub fn on_msdp(&self, var: &str, data: &serde_json::Value, callback: &str) {
        let r = (|| -> mlua::Result<()> {
            let t: Table = self.lua.globals().get("msdp")?;
            t.set(var, self.lua.to_value(data)?)
        })();
        self.report(r);
        self.call_if_exists(callback, var.to_string());
    }
}

fn join_values(args: &[LuaValue]) -> String {
    args.iter()
        .map(|v| match v {
            LuaValue::String(s) => s.to_string_lossy().to_string(),
            LuaValue::Nil => "nil".into(),
            other => other.to_string().unwrap_or_else(|_| format!("{other:?}")),
        })
        .collect::<Vec<_>>()
        .join("")
}

/// ColourNote("red", "black", "Text", "white", "", "mehr Text", …)
fn push_colour_args(line: &mut Line, args: &[String]) {
    for chunk in args.chunks(3) {
        if chunk.len() < 3 {
            break;
        }
        let style = Style {
            fg: color_from_name(&chunk[0]).unwrap_or(Color::Default),
            bg: color_from_name(&chunk[1]).unwrap_or(Color::Default),
            ..Style::default()
        };
        line.push_str(&chunk[2], style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_and_variables() {
        let e = ScriptEngine::new(BTreeMap::new(), "Test");
        e.run(
            r#"SetVariable("ziel", "ork"); Send("toete ", GetVariable("ziel")); world.Note("hi")"#,
            "test",
        );
        let a = e.take_actions();
        assert!(matches!(&a[0], Action::Send(s) if s == "toete ork"));
        assert!(matches!(&a[1], Action::Note(l) if l.plain() == "hi"));
        assert_eq!(e.shared.borrow().variables["ziel"], "ork");
    }

    #[test]
    fn gmcp_table() {
        let e = ScriptEngine::new(BTreeMap::new(), "Test");
        e.run("function OnGMCP(p) Note(p .. '=' .. gmcp.Char.Vitals.hp) end", "t");
        e.on_gmcp("Char.Vitals", &serde_json::json!({"hp": 42}), "OnGMCP");
        let a = e.take_actions();
        assert!(matches!(&a[0], Action::Note(l) if l.plain() == "Char.Vitals=42"));
    }

    #[test]
    fn ansi_helpers_and_windows() {
        let e = ScriptEngine::new(BTreeMap::new(), "Test");
        e.run(
            r#"
            local s = "|\27[31m*\27[0m| Text"
            assert(AnsiStrip(s) == "|*| Text")
            assert(AnsiLen(s) == 8)
            assert(AnsiStrip(AnsiSub(s, 1, 3)) == "|*|")
            assert(AnsiStrip(AnsiSub(s, 5)) == "Text")
            WindowCreate("w", "Titel")
            WindowGauge("w", "TP", 10, 20, "red")
            "#,
            "t",
        );
        let a = e.take_actions();
        assert!(
            matches!(&a[0], Action::WindowCreate(id, t) if id == "w" && t == "Titel"),
            "{a:?}"
        );
        assert!(matches!(&a[1], Action::WindowItem(_, WinItem::Gauge { value, .. }) if *value == 10.0));
    }

    #[test]
    fn line_hook_results() {
        let e = ScriptEngine::new(BTreeMap::new(), "Test");
        e.run(
            "function F(p, a) if p == 'weg' then return false elseif p == 'neu' then return 'anders' end end",
            "t",
        );
        assert_eq!(e.call_line_hook("F", "weg", "weg"), LineHook::Omit);
        assert_eq!(e.call_line_hook("F", "neu", "neu"), LineHook::Replace("anders".into()));
        assert_eq!(e.call_line_hook("F", "x", "x"), LineHook::Keep);
    }

    #[test]
    fn errors_become_notes() {
        let e = ScriptEngine::new(BTreeMap::new(), "Test");
        e.run("this is not lua", "t");
        let a = e.take_actions();
        assert!(matches!(&a[0], Action::Note(l) if l.plain().starts_with("Lua-Fehler")));
    }

    #[test]
    fn handler_gets_wildcards() {
        let e = ScriptEngine::new(BTreeMap::new(), "Test");
        e.run("function f(name, line, wc) Send('winke ' .. wc[1]) end", "t");
        e.call_handler(
            "f",
            "trg",
            "Bob kommt an.",
            &["Bob kommt an.".into(), "Bob".into()],
            &BTreeMap::new(),
        );
        let a = e.take_actions();
        assert!(matches!(&a[0], Action::Send(s) if s == "winke Bob"));
    }
}
