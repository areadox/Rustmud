//! Hauptfenster: Welt-Tabs, Ausgabe, Eingabezeile, Menüs, Statusleiste.

use crate::ansi::{resolve, Line};
use crate::session::{ConnState, Session, Waker};
use crate::ui_editors::{self, EditorState, VarState};
use crate::world::{config_dir, list_worlds, World};
use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{Color32, FontId, Key, RichText, ScrollArea, Stroke, TextEdit, TextFormat};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_FG: (u8, u8, u8) = (192, 192, 192);
const DEFAULT_BG: (u8, u8, u8) = (0, 0, 0);
const HISTORY_MAX: usize = 500;

struct View {
    session: Session,
    input: String,
    history: Vec<String>,
    hist_pos: Option<usize>,
    focus_input: bool,
    show_settings: bool,
    show_triggers: bool,
    show_aliases: bool,
    show_timers: bool,
    show_vars: bool,
    show_gmcp: bool,
    trg: EditorState,
    ali: EditorState,
    tim: EditorState,
    vars: VarState,
}

impl View {
    fn new(session: Session) -> Self {
        Self {
            session,
            input: String::new(),
            history: Vec::new(),
            hist_pos: None,
            focus_input: true,
            show_settings: false,
            show_triggers: false,
            show_aliases: false,
            show_timers: false,
            show_vars: false,
            show_gmcp: false,
            trg: EditorState::default(),
            ali: EditorState::default(),
            tim: EditorState::default(),
            vars: VarState::default(),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct AppState {
    open_worlds: Vec<PathBuf>,
    #[serde(default)]
    active: usize,
}

fn app_state_path() -> PathBuf {
    config_dir().join("rustmud.toml")
}

pub struct MudApp {
    views: Vec<View>,
    active: usize,
    waker: Waker,
    show_open: bool,
    status: String,
}

impl MudApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        let waker: Waker = Arc::new(move || ctx.request_repaint());
        let mut app = Self {
            views: Vec::new(),
            active: 0,
            waker,
            show_open: false,
            status: String::new(),
        };

        let state: AppState = std::fs::read_to_string(app_state_path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        for p in &state.open_worlds {
            if let Ok(w) = World::load(p) {
                app.open_world(w, Some(p.clone()));
            }
        }
        app.active = state.active.min(app.views.len().saturating_sub(1));
        if app.views.is_empty() {
            app.show_open = true;
        }
        app
    }

    fn open_world(&mut self, world: World, path: Option<PathBuf>) {
        let auto = world.auto_connect;
        let mut session = Session::new(world, path, self.waker.clone());
        if auto {
            session.connect();
        }
        self.views.push(View::new(session));
        self.active = self.views.len() - 1;
    }

    fn save_state(&mut self) {
        let mut open = Vec::new();
        for v in &mut self.views {
            match v.session.save_world() {
                Ok(p) => open.push(p),
                Err(e) => eprintln!("Speichern fehlgeschlagen: {e}"),
            }
        }
        let st = AppState {
            open_worlds: open,
            active: self.active,
        };
        if let Ok(text) = toml::to_string_pretty(&st) {
            let _ = std::fs::create_dir_all(config_dir());
            let _ = std::fs::write(app_state_path(), text);
        }
    }

    fn close_view(&mut self, i: usize) {
        let mut v = self.views.remove(i);
        v.session.disconnect();
        let _ = v.session.save_world();
        if self.active >= self.views.len() {
            self.active = self.views.len().saturating_sub(1);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Welt", |ui| {
                if ui.button("Öffnen / Neu …").clicked() {
                    self.show_open = true;
                    ui.close_menu();
                }
                let has = !self.views.is_empty();
                if ui
                    .add_enabled(has, egui::Button::new("Speichern"))
                    .clicked()
                {
                    let v = &mut self.views[self.active];
                    self.status = match v.session.save_world() {
                        Ok(p) => format!("Gespeichert: {}", p.display()),
                        Err(e) => format!("Fehler: {e}"),
                    };
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .add_enabled(has, egui::Button::new("Verbinden"))
                    .clicked()
                {
                    self.views[self.active].session.connect();
                    ui.close_menu();
                }
                if ui.add_enabled(has, egui::Button::new("Trennen")).clicked() {
                    self.views[self.active].session.disconnect();
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .add_enabled(has, egui::Button::new("Welt schließen"))
                    .clicked()
                {
                    let a = self.active;
                    self.close_view(a);
                    ui.close_menu();
                }
            });
            let Some(v) = self.views.get_mut(self.active) else {
                return;
            };
            ui.menu_button("Konfiguration", |ui| {
                if ui.button("Welt-Einstellungen …").clicked() {
                    v.show_settings = true;
                    ui.close_menu();
                }
                if ui.button("Trigger …").clicked() {
                    v.show_triggers = true;
                    ui.close_menu();
                }
                if ui.button("Aliase …").clicked() {
                    v.show_aliases = true;
                    ui.close_menu();
                }
                if ui.button("Timer …").clicked() {
                    v.show_timers = true;
                    ui.close_menu();
                }
                if ui.button("Variablen …").clicked() {
                    v.show_vars = true;
                    ui.close_menu();
                }
            });
            ui.menu_button("Skript", |ui| {
                if ui.button("Lua neu laden").clicked() {
                    v.session.reload_script();
                    ui.close_menu();
                }
            });
            ui.menu_button("Ansicht", |ui| {
                ui.checkbox(&mut v.show_gmcp, "GMCP/MSDP-Daten");
                if ui.button("Ausgabe leeren").clicked() {
                    v.session.clear_output();
                    ui.close_menu();
                }
            });
        });
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_open;
        let mut chosen: Option<(World, Option<PathBuf>)> = None;
        egui::Window::new("Welt öffnen")
            .open(&mut open)
            .collapsible(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                let worlds = list_worlds();
                if worlds.is_empty() {
                    ui.label("Noch keine Welten gespeichert.");
                }
                for (path, name) in worlds {
                    if ui
                        .button(&name)
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        match World::load(&path) {
                            Ok(w) => chosen = Some((w, Some(path))),
                            Err(e) => self.status = e,
                        }
                    }
                }
                ui.separator();
                if ui.button("➕ Neue Welt").clicked() {
                    chosen = Some((World::default(), None));
                }
                ui.label(
                    RichText::new(format!(
                        "Welten liegen in {}",
                        crate::world::worlds_dir().display()
                    ))
                    .weak()
                    .small(),
                );
            });
        if let Some((w, p)) = chosen {
            let is_new = p.is_none();
            self.open_world(w, p);
            if is_new {
                self.views[self.active].show_settings = true;
            }
            open = false;
        }
        self.show_open = open;
    }
}

fn line_job(line: &Line, font: &FontId, job: &mut LayoutJob) {
    for span in &line.spans {
        let (fg, bg) = resolve(&span.style, DEFAULT_FG, DEFAULT_BG);
        let mut fmt = TextFormat {
            font_id: font.clone(),
            color: Color32::from_rgb(fg.0, fg.1, fg.2),
            italics: span.style.italic,
            ..Default::default()
        };
        if let Some(bg) = bg {
            fmt.background = Color32::from_rgb(bg.0, bg.1, bg.2);
        }
        if span.style.underline {
            fmt.underline = Stroke::new(1.0_f32, fmt.color);
        }
        job.append(&span.text, 0.0, fmt);
    }
}

fn status_text(s: &Session) -> RichText {
    match s.state {
        ConnState::Connected => RichText::new("● Verbunden").color(Color32::LIGHT_GREEN),
        ConnState::Connecting => RichText::new("● Verbinde …").color(Color32::YELLOW),
        ConnState::Disconnected => RichText::new("● Getrennt").color(Color32::LIGHT_RED),
    }
}

impl eframe::App for MudApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut next = std::time::Duration::from_millis(250);
        for v in &mut self.views {
            v.session.poll();
            next = next.min(v.session.tick());
        }
        ctx.request_repaint_after(next);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.menu_bar(ui);
            if !self.views.is_empty() {
                ui.horizontal(|ui| {
                    let mut close = None;
                    for (i, v) in self.views.iter().enumerate() {
                        let color = match v.session.state {
                            ConnState::Connected => Color32::LIGHT_GREEN,
                            ConnState::Connecting => Color32::YELLOW,
                            ConnState::Disconnected => Color32::GRAY,
                        };
                        ui.label(RichText::new("●").color(color));
                        if ui
                            .selectable_label(i == self.active, &v.session.world.name)
                            .clicked()
                        {
                            self.active = i;
                        }
                        if ui
                            .small_button("✖")
                            .on_hover_text("Welt schließen")
                            .clicked()
                        {
                            close = Some(i);
                        }
                        ui.separator();
                    }
                    if let Some(i) = close {
                        self.close_view(i);
                    }
                });
            }
        });

        if self.show_open || self.views.is_empty() {
            if self.views.is_empty() {
                self.show_open = true;
            }
            self.open_dialog(ctx);
        }
        let Some(v) = self.views.get_mut(self.active) else {
            return;
        };
        let font = FontId::monospace(v.session.world.font_size);

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(status_text(&v.session));
                ui.separator();
                ui.label(format!("{}:{}", v.session.world.host, v.session.world.port));
                if v.session.mccp {
                    ui.separator();
                    ui.label("MCCP");
                }
                if v.session
                    .gmcp_tree
                    .as_object()
                    .is_some_and(|m| !m.is_empty())
                {
                    ui.separator();
                    ui.label("GMCP");
                }
                if !v.session.msdp_tree.is_empty() {
                    ui.separator();
                    ui.label("MSDP");
                }
                ui.separator();
                ui.label(format!("{} Zeilen", v.session.lines.len()));
                if !self.status.is_empty() {
                    ui.separator();
                    ui.label(RichText::new(&self.status).weak());
                }
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| match v.session.state {
                        ConnState::Disconnected => {
                            if ui.button("Verbinden").clicked() {
                                v.session.connect();
                            }
                        }
                        _ => {
                            if ui.button("Trennen").clicked() {
                                v.session.disconnect();
                            }
                        }
                    },
                );
            });
        });

        // ---------------------------------------------------- Eingabezeile
        egui::TopBottomPanel::bottom("input").show(ctx, |ui| {
            let id = egui::Id::new(("input", self.active));
            let focused = ctx.memory(|m| m.has_focus(id));
            let mut hist_changed = false;
            if focused && !v.history.is_empty() {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)) {
                    let p = match v.hist_pos {
                        None => v.history.len() - 1,
                        Some(p) => p.saturating_sub(1),
                    };
                    v.hist_pos = Some(p);
                    v.input = v.history[p].clone();
                    hist_changed = true;
                }
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)) {
                    match v.hist_pos {
                        Some(p) if p + 1 < v.history.len() => {
                            v.hist_pos = Some(p + 1);
                            v.input = v.history[p + 1].clone();
                        }
                        _ => {
                            v.hist_pos = None;
                            v.input.clear();
                        }
                    }
                    hist_changed = true;
                }
            }
            let resp = ui.add(
                TextEdit::singleline(&mut v.input)
                    .id(id)
                    .font(font.clone())
                    .password(v.session.hide_input)
                    .hint_text(if v.session.hide_input {
                        "Passwort …"
                    } else {
                        ""
                    })
                    .desired_width(f32::INFINITY)
                    .lock_focus(true),
            );
            if hist_changed {
                if let Some(mut st) = TextEdit::load_state(ctx, id) {
                    let n = v.input.chars().count();
                    st.cursor
                        .set_char_range(Some(CCursorRange::one(CCursor::new(n))));
                    st.store(ctx, id);
                }
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let text = std::mem::take(&mut v.input);
                if !v.session.hide_input && !text.is_empty() && v.history.last() != Some(&text) {
                    v.history.push(text.clone());
                    if v.history.len() > HISTORY_MAX {
                        v.history.remove(0);
                    }
                }
                v.hist_pos = None;
                v.session.user_input(&text);
                v.focus_input = true;
            }
            if v.focus_input {
                resp.request_focus();
                v.focus_input = false;
            }
        });

        // ---------------------------------------------------- GMCP-Inspektor
        if v.show_gmcp {
            egui::SidePanel::right("gmcp")
                .default_width(280.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("GMCP");
                    ScrollArea::vertical()
                        .id_salt("gmcp_scroll")
                        .show(ui, |ui| {
                            if let Some(map) = v.session.gmcp_tree.as_object() {
                                if map.is_empty() {
                                    ui.label(RichText::new("noch keine Daten").weak());
                                }
                                for (k, val) in map {
                                    ui_editors::json_tree(ui, k, val, &format!("g.{k}"));
                                }
                            }
                            ui.separator();
                            ui.heading("MSDP");
                            if v.session.msdp_tree.is_empty() {
                                ui.label(RichText::new("noch keine Daten").weak());
                            }
                            for (k, val) in &v.session.msdp_tree {
                                ui_editors::json_tree(ui, k, val, &format!("m.{k}"));
                            }
                        });
                });
        }

        // ---------------------------------------------------- Ausgabe
        let bg = Color32::from_rgb(DEFAULT_BG.0, DEFAULT_BG.1, DEFAULT_BG.2);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg).inner_margin(4.0))
            .show(ctx, |ui| {
                let (row_h, char_w) =
                    ui.fonts(|f| (f.row_height(&font), f.glyph_width(&font, 'M')));
                let avail = ui.available_size();
                let cols = (avail.x / char_w.max(1.0)).floor().clamp(20.0, 500.0) as u16;
                let rows = (avail.y / row_h.max(1.0)).floor().clamp(5.0, 300.0) as u16;
                v.session.set_term_size(cols, rows);

                let s = &v.session;
                let has_current = !s.current.is_empty();
                let total = s.lines.len() + usize::from(has_current);
                ui.spacing_mut().item_spacing.y = 0.0;
                ScrollArea::both()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show_rows(ui, row_h, total, |ui, range| {
                        for i in range {
                            let line = if i < s.lines.len() {
                                &s.lines[i]
                            } else {
                                &s.current
                            };
                            let mut job = LayoutJob::default();
                            job.wrap.max_width = f32::INFINITY;
                            if line.spans.is_empty() {
                                job.append(
                                    " ",
                                    0.0,
                                    TextFormat {
                                        font_id: font.clone(),
                                        ..Default::default()
                                    },
                                );
                            } else {
                                line_job(line, &font, &mut job);
                            }
                            ui.add(egui::Label::new(job).extend());
                        }
                    });
            });

        // Hat nichts den Fokus (z. B. nach Klick in die Ausgabe), bekommt ihn die Eingabezeile zurück
        if ctx.memory(|m| m.focused().is_none()) {
            v.focus_input = true;
            ctx.request_repaint();
        }

        // ---------------------------------------------------- Dialoge
        let title = v.session.world.name.clone();
        let mut reload = false;
        egui::Window::new(format!("Einstellungen – {title}"))
            .id(egui::Id::new(("settings", self.active)))
            .open(&mut v.show_settings)
            .default_width(460.0)
            .show(ctx, |ui| {
                reload = ui_editors::world_settings(ui, &mut v.session);
                ui.separator();
                if ui.button("💾 Welt speichern").clicked() {
                    self.status = match v.session.save_world() {
                        Ok(p) => format!("Gespeichert: {}", p.display()),
                        Err(e) => format!("Fehler: {e}"),
                    };
                }
            });
        if reload {
            v.session.reload_script();
        }
        egui::Window::new(format!("Trigger – {title}"))
            .id(egui::Id::new(("triggers", self.active)))
            .open(&mut v.show_triggers)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui_editors::triggers(ui, &mut v.session, &mut v.trg)
            });
        egui::Window::new(format!("Aliase – {title}"))
            .id(egui::Id::new(("aliases", self.active)))
            .open(&mut v.show_aliases)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui_editors::aliases(ui, &mut v.session, &mut v.ali)
            });
        egui::Window::new(format!("Timer – {title}"))
            .id(egui::Id::new(("timers", self.active)))
            .open(&mut v.show_timers)
            .default_width(480.0)
            .show(ctx, |ui| ui_editors::timers(ui, &mut v.session, &mut v.tim));
        egui::Window::new(format!("Variablen – {title}"))
            .id(egui::Id::new(("vars", self.active)))
            .open(&mut v.show_vars)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui_editors::variables(ui, &mut v.session, &mut v.vars)
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_state();
        for v in &mut self.views {
            v.session.disconnect();
        }
    }
}
