//! Hauptfenster: Icon-Leiste, Welt-Tabs, Ausgabe, Eingabezeile, Menüs,
//! Weltenverwaltung, Update-Hinweis und Statusleiste.

use crate::ansi::{resolve, Line};
use crate::session::{ConnState, Session, Waker};
use crate::ui_editors::{self, EditorState, VarState};
use crate::updater::{self, UpdateState};
use crate::world::{config_dir, list_worlds, path_for, unique_name, Encoding, World};
use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{Button, Color32, FontId, Key, RichText, ScrollArea, Stroke, TextEdit, TextFormat};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const DEFAULT_FG: (u8, u8, u8) = (192, 192, 192);
const DEFAULT_BG: (u8, u8, u8) = (0, 0, 0);
const HISTORY_MAX: usize = 500;

/// Fenster, die im Andockbereich rechts als Tabs erscheinen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Worlds,
    Settings,
    Triggers,
    Aliases,
    Timers,
    Variables,
    Keypad,
    Buttons,
    Plugins,
    Map,
    Gmcp,
    About,
}

impl Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Worlds => "🌍 Welten",
            Panel::Settings => "⚙ Einstellungen",
            Panel::Triggers => "⚡ Trigger",
            Panel::Aliases => "⌨ Aliase",
            Panel::Timers => "⏱ Timer",
            Panel::Variables => "📋 Variablen",
            Panel::Keypad => "🔢 Ziffernblock",
            Panel::Buttons => "🔘 Knöpfe",
            Panel::Plugins => "📦 Plugins",
            Panel::Map => "🗺 Karte",
            Panel::Gmcp => "📡 GMCP",
            Panel::About => "Über",
        }
    }
}

struct View {
    session: Session,
    input: String,
    history: Vec<String>,
    hist_pos: Option<usize>,
    focus_input: bool,
    trg: EditorState,
    ali: EditorState,
    tim: EditorState,
    vars: VarState,
    button_sel: Option<usize>,
    /// Kartenstand, den das Fenster schon kennt (zum automatischen Öffnen)
    map_seen: u64,
}

impl View {
    fn new(session: Session) -> Self {
        Self {
            session,
            input: String::new(),
            history: Vec::new(),
            hist_pos: None,
            focus_input: true,
            trg: EditorState::default(),
            ali: EditorState::default(),
            tim: EditorState::default(),
            vars: VarState::default(),
            button_sel: None,
            map_seen: 0,
        }
    }
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct AppState {
    #[serde(default)]
    open_worlds: Vec<PathBuf>,
    #[serde(default)]
    active: usize,
    #[serde(default = "yes")]
    check_updates: bool,
    #[serde(default = "yes")]
    show_toolbar: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            open_worlds: Vec::new(),
            active: 0,
            check_updates: true,
            show_toolbar: true,
        }
    }
}

fn app_state_path() -> PathBuf {
    config_dir().join("rustmud.toml")
}

/// Eingabefelder und Zustand der Weltenverwaltung.
struct WorldManager {
    worlds: Option<Vec<(PathBuf, World)>>,
    new_name: String,
    new_host: String,
    new_port: u16,
    new_encoding: Encoding,
    rename: Option<(PathBuf, String)>,
    confirm_delete: Option<PathBuf>,
    message: String,
}

impl Default for WorldManager {
    fn default() -> Self {
        Self {
            worlds: None,
            new_name: String::new(),
            new_host: String::new(),
            new_port: 23,
            new_encoding: Encoding::Latin1,
            rename: None,
            confirm_delete: None,
            message: String::new(),
        }
    }
}

enum WmAction {
    Open(PathBuf, bool),
    Edit(PathBuf),
    Create,
    Duplicate(PathBuf),
    Rename(PathBuf, String),
    Delete(PathBuf),
    Import,
    Export(PathBuf),
}

pub struct MudApp {
    views: Vec<View>,
    active: usize,
    waker: Waker,
    ctx: egui::Context,
    wm: WorldManager,
    settings: AppState,
    panels: Vec<Panel>,
    panel: Option<Panel>,
    plugin_ui: crate::ui_editors::PluginUi,
    update: updater::Shared,
    update_dismissed: bool,
    status: String,
}

impl MudApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        let c2 = ctx.clone();
        let waker: Waker = Arc::new(move || c2.request_repaint());
        crate::keypad::install(cc);

        let settings: AppState = std::fs::read_to_string(app_state_path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        let mut app = Self {
            views: Vec::new(),
            active: 0,
            waker,
            ctx: ctx.clone(),
            wm: WorldManager::default(),
            settings,
            panels: Vec::new(),
            panel: None,
            plugin_ui: Default::default(),
            update: Arc::new(Mutex::new(UpdateState::Idle)),
            update_dismissed: false,
            status: String::new(),
        };
        for p in app.settings.open_worlds.clone() {
            if let Ok(w) = World::load(&p) {
                app.open_world(w, Some(p), false);
            }
        }
        app.active = app.settings.active.min(app.views.len().saturating_sub(1));
        if app.views.is_empty() {
            app.open_panel(Panel::Worlds);
        }
        if app.settings.check_updates {
            updater::check(app.update.clone(), ctx);
        }
        app
    }

    fn open_world(&mut self, world: World, path: Option<PathBuf>, connect: bool) {
        if let Some(p) = &path {
            if let Some(i) = self.views.iter().position(|v| v.session.world_path.as_ref() == Some(p)) {
                self.active = i;
                if connect {
                    self.views[i].session.connect();
                }
                return;
            }
        }
        let auto = world.auto_connect || connect;
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
        self.settings.open_worlds = open;
        self.settings.active = self.active;
        self.save_settings();
    }

    fn save_settings(&self) {
        if let Ok(text) = toml::to_string_pretty(&self.settings) {
            let _ = std::fs::create_dir_all(config_dir());
            let _ = std::fs::write(app_state_path(), text);
        }
    }

    fn close_view(&mut self, i: usize, save: bool) {
        let mut v = self.views.remove(i);
        v.session.disconnect();
        if save {
            let _ = v.session.save_world();
        }
        if self.active >= self.views.len() {
            self.active = self.views.len().saturating_sub(1);
        }
    }

    fn open_panel(&mut self, p: Panel) {
        if !self.panels.contains(&p) {
            self.panels.push(p);
        }
        self.panel = Some(p);
        if p == Panel::Worlds {
            self.wm.worlds = None;
        }
    }

    fn close_panel(&mut self, p: Panel) {
        self.panels.retain(|x| *x != p);
        if self.panel == Some(p) {
            self.panel = self.panels.last().copied();
        }
    }

    /// Icon-Leiste: öffnen bzw. nach vorne holen; ist es schon vorne, schließen.
    fn toggle_panel(&mut self, p: Panel) {
        if self.panel == Some(p) && self.panels.contains(&p) {
            self.close_panel(p);
        } else {
            self.open_panel(p);
        }
    }

    fn view_index(&self, path: &PathBuf) -> Option<usize> {
        self.views
            .iter()
            .position(|v| v.session.world_path.as_ref() == Some(path))
    }

    fn save_active(&mut self) {
        if let Some(v) = self.views.get_mut(self.active) {
            self.status = match v.session.save_world() {
                Ok(p) => format!("Gespeichert: {}", p.display()),
                Err(e) => format!("Fehler: {e}"),
            };
            self.wm.worlds = None;
        }
    }

    // ------------------------------------------------------------ Menü

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let mut req: Option<Panel> = None;
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Welt", |ui| {
                if ui.button("🌍 Weltenverwaltung …").clicked() {
                    req = Some(Panel::Worlds);
                    ui.close_menu();
                }
                let has = !self.views.is_empty();
                if ui.add_enabled(has, Button::new("💾 Speichern")).clicked() {
                    self.save_active();
                    ui.close_menu();
                }
                ui.separator();
                if ui.add_enabled(has, Button::new("Verbinden")).clicked() {
                    self.views[self.active].session.connect();
                    ui.close_menu();
                }
                if ui.add_enabled(has, Button::new("Trennen")).clicked() {
                    self.views[self.active].session.disconnect();
                    ui.close_menu();
                }
                ui.separator();
                if ui.add_enabled(has, Button::new("Welt schließen")).clicked() {
                    let a = self.active;
                    self.close_view(a, true);
                    ui.close_menu();
                }
                if ui.button("Beenden").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Konfiguration", |ui| {
                for (p, label) in [
                    (Panel::Settings, "⚙ Welt-Einstellungen …"),
                    (Panel::Triggers, "⚡ Trigger …"),
                    (Panel::Aliases, "⌨ Aliase …"),
                    (Panel::Timers, "⏱ Timer …"),
                    (Panel::Variables, "📋 Variablen …"),
                    (Panel::Keypad, "🔢 Ziffernblock …"),
                    (Panel::Buttons, "🔘 Eigene Knöpfe …"),
                    (Panel::Plugins, "📦 Plugins …"),
                    (Panel::Map, "🗺 Karte …"),
                ] {
                    if ui.button(label).clicked() {
                        req = Some(p);
                        ui.close_menu();
                    }
                }
            });
            if let Some(v) = self.views.get_mut(self.active) {
                ui.menu_button("Skript", |ui| {
                    if ui.button("🔄 Lua neu laden").clicked() {
                        v.session.reload_script();
                        ui.close_menu();
                    }
                    if ui.button("📦 Plugins neu laden").clicked() {
                        v.session.reload_plugins();
                        ui.close_menu();
                    }
                });
            }
            ui.menu_button("Ansicht", |ui| {
                ui.checkbox(&mut self.settings.show_toolbar, "Icon-Leiste");
                if ui.button("📡 GMCP/MSDP-Daten").clicked() {
                    req = Some(Panel::Gmcp);
                    ui.close_menu();
                }
                if ui.button("Alle angedockten Fenster schließen").clicked() {
                    self.panels.clear();
                    self.panel = None;
                    ui.close_menu();
                }
                if let Some(v) = self.views.get_mut(self.active) {
                    if ui.button("Ausgabe leeren").clicked() {
                        v.session.clear_output();
                        ui.close_menu();
                    }
                }
            });
            ui.menu_button("Hilfe", |ui| {
                if ui.button("Nach Updates suchen").clicked() {
                    self.update_dismissed = false;
                    updater::check(self.update.clone(), self.ctx.clone());
                    req = Some(Panel::About);
                    ui.close_menu();
                }
                if ui.button("Projektseite (GitHub)").clicked() {
                    updater::open_url(&format!("https://github.com/{}", updater::REPO));
                    ui.close_menu();
                }
                if ui.button("Über rustmud / Programmeinstellungen …").clicked() {
                    req = Some(Panel::About);
                    ui.close_menu();
                }
            });
        });
        if let Some(p) = req {
            self.open_panel(p);
        }
    }

    // ------------------------------------------------------------ Icon-Leiste

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let big = |s: &str| RichText::new(s).size(18.0);
        let mut toggle: Option<Panel> = None;
        let mut save = false;
        let open_panels = self.panels.clone();
        let is_open = |p: Panel| open_panels.contains(&p);
        ui.horizontal(|ui| {
            if ui
                .selectable_label(is_open(Panel::Worlds), big("🌍"))
                .on_hover_text("Weltenverwaltung")
                .clicked()
            {
                toggle = Some(Panel::Worlds);
            }
            let Some(v) = self.views.get_mut(self.active) else {
                return;
            };
            match v.session.state {
                ConnState::Disconnected => {
                    if ui.button(big("🔌")).on_hover_text("Verbinden").clicked() {
                        v.session.connect();
                    }
                }
                _ => {
                    if ui.button(big("⏏")).on_hover_text("Trennen").clicked() {
                        v.session.disconnect();
                    }
                }
            }
            if ui.button(big("💾")).on_hover_text("Welt speichern").clicked() {
                save = true;
            }
            ui.separator();
            for (p, icon, tip) in [
                (Panel::Settings, "⚙", "Welt-Einstellungen"),
                (Panel::Triggers, "⚡", "Trigger"),
                (Panel::Aliases, "⌨", "Aliase"),
                (Panel::Timers, "⏱", "Timer"),
                (Panel::Variables, "📋", "Variablen"),
                (Panel::Keypad, "🔢", "Ziffernblock"),
                (Panel::Plugins, "📦", "Plugins"),
                (Panel::Map, "🗺", "Karte"),
            ] {
                if ui.selectable_label(is_open(p), big(icon)).on_hover_text(tip).clicked() {
                    toggle = Some(p);
                }
            }
            ui.separator();
            if ui.button(big("🔄")).on_hover_text("Lua-Skript neu laden").clicked() {
                v.session.reload_script();
            }
            let logging = v.session.is_logging();
            if ui
                .selectable_label(logging, big("📝"))
                .on_hover_text(if logging { "Log beenden" } else { "Log starten" })
                .clicked()
            {
                v.session.set_logging(!logging);
            }
            if ui
                .selectable_label(is_open(Panel::Gmcp), big("📡"))
                .on_hover_text("GMCP/MSDP-Daten")
                .clicked()
            {
                toggle = Some(Panel::Gmcp);
            }
            if ui.button(big("🗑")).on_hover_text("Ausgabe leeren").clicked() {
                v.session.clear_output();
            }
            ui.separator();
            // Eigene Knöpfe der Welt
            let mut fire = None;
            for (i, b) in v.session.world.buttons.iter().enumerate() {
                let mut r = ui.button(RichText::new(&b.label).size(15.0));
                if !b.tooltip.is_empty() {
                    r = r.on_hover_text(&b.tooltip);
                }
                if r.clicked() {
                    fire = Some(i);
                }
            }
            if let Some(i) = fire {
                v.session.run_button(i);
            }
            if ui
                .small_button("➕")
                .on_hover_text("Eigene Knöpfe bearbeiten")
                .clicked()
            {
                toggle = Some(Panel::Buttons);
            }
        });
        if save {
            self.save_active();
        }
        if let Some(p) = toggle {
            self.toggle_panel(p);
        }
    }

    // ------------------------------------------------------------ Update

    fn update_banner(&mut self, ctx: &egui::Context) {
        let state = self.update.lock().unwrap().clone();
        let info = match &state {
            UpdateState::Available(i) if !self.update_dismissed => i.clone(),
            UpdateState::Downloading(ver) => {
                egui::TopBottomPanel::top("update").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Lade Version {ver} herunter …"));
                    });
                });
                return;
            }
            UpdateState::Installing => {
                // Setup läuft – Programm schließen, damit Dateien ersetzt werden können
                self.save_state();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            _ => return,
        };
        egui::TopBottomPanel::top("update").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "⬆ rustmud {} ist verfügbar (installiert: {}).",
                        info.version,
                        updater::current_version()
                    ))
                    .color(Color32::LIGHT_GREEN),
                );
                if updater::can_auto_install(&info) {
                    if ui.button("Jetzt aktualisieren").clicked() {
                        self.save_state();
                        updater::download_and_install(self.update.clone(), info.clone(), ctx.clone());
                    }
                } else if ui.button("Download-Seite öffnen").clicked() {
                    updater::open_url(&info.page_url);
                }
                if ui.button("Was ist neu?").clicked() {
                    self.open_panel(Panel::About);
                }
                if ui.button("Später").clicked() {
                    self.update_dismissed = true;
                }
            });
        });
    }

    fn about_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        {
            ui.horizontal(|ui| {
                ui.heading("rustmud");
                ui.label(format!("Version {}", updater::current_version()));
            });
            ui.label("Freier MUD-Client nach dem Vorbild von MUSHclient. MIT-Lizenz.");
            ui.hyperlink_to(
                "github.com/areadox/Rustmud",
                format!("https://github.com/{}", updater::REPO),
            );
            ui.separator();
            ui.heading("Updates");
            if ui
                .checkbox(&mut self.settings.check_updates, "Beim Start nach Updates suchen")
                .changed()
            {
                self.save_settings();
            }
            let state = self.update.lock().unwrap().clone();
            match state {
                UpdateState::Idle => {}
                UpdateState::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Suche nach Updates …");
                    });
                }
                UpdateState::UpToDate => {
                    ui.label(RichText::new("✔ Du hast die neueste Version.").color(Color32::LIGHT_GREEN));
                }
                UpdateState::Available(info) => {
                    ui.label(
                        RichText::new(format!("Neue Version {} verfügbar.", info.version)).color(Color32::LIGHT_GREEN),
                    );
                    if !info.notes.trim().is_empty() {
                        ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            ui.label(&info.notes);
                        });
                    }
                    ui.horizontal(|ui| {
                        if updater::can_auto_install(&info) {
                            if ui.button("Jetzt aktualisieren").clicked() {
                                self.save_state();
                                updater::download_and_install(self.update.clone(), info.clone(), ctx.clone());
                            }
                        } else {
                            ui.label(RichText::new("Automatisches Update nur bei Installation über das Setup.").weak());
                        }
                        if ui.button("Release-Seite").clicked() {
                            updater::open_url(&info.page_url);
                        }
                    });
                }
                UpdateState::Downloading(v) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Lade {v} …"));
                    });
                }
                UpdateState::Installing => {
                    ui.label("Installation läuft …");
                }
                UpdateState::Failed(e) => {
                    ui.label(RichText::new(format!("Update-Prüfung fehlgeschlagen: {e}")).color(Color32::LIGHT_RED));
                }
            }
            if ui.button("Jetzt prüfen").clicked() {
                updater::check(self.update.clone(), ctx.clone());
            }
            ui.separator();
            ui.label(
                RichText::new(format!("Konfiguration: {}", config_dir().display()))
                    .weak()
                    .small(),
            );
        }
    }

    // ------------------------------------------------------------ Weltenverwaltung

    fn world_manager_ui(&mut self, ui: &mut egui::Ui) {
        if self.wm.worlds.is_none() {
            self.wm.worlds = Some(
                list_worlds()
                    .into_iter()
                    .filter_map(|(p, _)| World::load(&p).ok().map(|w| (p, w)))
                    .collect(),
            );
        }
        let mut action: Option<WmAction> = None;
        let open_paths: Vec<PathBuf> = self.views.iter().filter_map(|v| v.session.world_path.clone()).collect();
        {
            let wm = &mut self.wm;
            let worlds = wm.worlds.as_ref().unwrap();
            if worlds.is_empty() {
                ui.label("Noch keine Welten angelegt – unten eine neue Welt anlegen.");
            }
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                egui::Grid::new("worlds")
                    .num_columns(4)
                    .striped(true)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Welt").strong());
                        ui.label(RichText::new("Server").strong());
                        ui.label(RichText::new("Zeichensatz").strong());
                        ui.label("");
                        ui.end_row();
                        for (path, w) in worlds {
                            let is_open = open_paths.contains(path);
                            match &mut wm.rename {
                                Some((p, name)) if p == path => {
                                    ui.horizontal(|ui| {
                                        let r = ui.add(TextEdit::singleline(name).desired_width(150.0));
                                        if ui.button("✔").clicked()
                                            || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                                        {
                                            action = Some(WmAction::Rename(path.clone(), name.clone()));
                                        }
                                    });
                                }
                                _ => {
                                    let mut t = RichText::new(&w.name);
                                    if is_open {
                                        t = t.strong().color(Color32::LIGHT_GREEN);
                                    }
                                    ui.label(t).on_hover_text(path.display().to_string());
                                }
                            }
                            ui.label(format!("{}:{}", w.host, w.port));
                            ui.label(match w.encoding {
                                Encoding::Latin1 => "Latin-1",
                                Encoding::Utf8 => "UTF-8",
                            });
                            ui.horizontal(|ui| {
                                if ui.button("🔌 Verbinden").clicked() {
                                    action = Some(WmAction::Open(path.clone(), true));
                                }
                                if ui.button(if is_open { "Anzeigen" } else { "Öffnen" }).clicked() {
                                    action = Some(WmAction::Open(path.clone(), false));
                                }
                                if ui.button("⚙").on_hover_text("Einstellungen bearbeiten").clicked() {
                                    action = Some(WmAction::Edit(path.clone()));
                                }
                                if ui.button("✏").on_hover_text("Umbenennen").clicked() {
                                    wm.rename = Some((path.clone(), w.name.clone()));
                                }
                                if ui.button("🗐").on_hover_text("Duplizieren").clicked() {
                                    action = Some(WmAction::Duplicate(path.clone()));
                                }
                                if ui.button("📤").on_hover_text("Exportieren").clicked() {
                                    action = Some(WmAction::Export(path.clone()));
                                }
                                if wm.confirm_delete.as_ref() == Some(path) {
                                    if ui
                                        .button(RichText::new("Wirklich löschen?").color(Color32::LIGHT_RED))
                                        .clicked()
                                    {
                                        action = Some(WmAction::Delete(path.clone()));
                                    }
                                    if ui.button("Nein").clicked() {
                                        wm.confirm_delete = None;
                                    }
                                } else if ui.button("🗑").on_hover_text("Löschen").clicked() {
                                    wm.confirm_delete = Some(path.clone());
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
            ui.separator();
            ui.heading("Neue Welt");
            egui::Grid::new("new_world")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.add(TextEdit::singleline(&mut wm.new_name).hint_text("z. B. MorgenGrauen"));
                    ui.end_row();
                    ui.label("Server");
                    ui.horizontal(|ui| {
                        ui.add(
                            TextEdit::singleline(&mut wm.new_host)
                                .hint_text("mg.mud.de")
                                .desired_width(180.0),
                        );
                        ui.label("Port");
                        ui.add(egui::DragValue::new(&mut wm.new_port).range(1..=65535));
                    });
                    ui.end_row();
                    ui.label("Zeichensatz");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut wm.new_encoding, Encoding::Latin1, "Latin-1");
                        ui.radio_value(&mut wm.new_encoding, Encoding::Utf8, "UTF-8");
                    });
                    ui.end_row();
                });
            ui.horizontal(|ui| {
                let ok = !wm.new_name.trim().is_empty() && !wm.new_host.trim().is_empty();
                if ui.add_enabled(ok, Button::new("➕ Anlegen und öffnen")).clicked() {
                    action = Some(WmAction::Create);
                }
                if ui.button("📥 Welt importieren …").clicked() {
                    action = Some(WmAction::Import);
                }
            });
            if !wm.message.is_empty() {
                ui.label(RichText::new(&wm.message).weak());
            }
        }
        if let Some(a) = action {
            self.apply_wm_action(a);
        }
    }

    fn apply_wm_action(&mut self, a: WmAction) {
        self.wm.message.clear();
        let result: Result<(), String> = (|| {
            match a {
                WmAction::Open(p, connect) => {
                    let w = World::load(&p)?;
                    self.open_world(w, Some(p), connect);
                    self.close_panel(Panel::Worlds);
                }
                WmAction::Edit(p) => {
                    if self.view_index(&p).is_none() {
                        let w = World::load(&p)?;
                        self.open_world(w, Some(p.clone()), false);
                    }
                    let i = self.view_index(&p).unwrap();
                    self.active = i;
                    self.open_panel(Panel::Settings);
                }
                WmAction::Create => {
                    let w = World {
                        name: unique_name(self.wm.new_name.trim()),
                        host: self.wm.new_host.trim().to_string(),
                        port: self.wm.new_port,
                        encoding: self.wm.new_encoding,
                        ..World::default()
                    };
                    let p = path_for(&w);
                    w.save(&p)?;
                    self.wm.new_name.clear();
                    self.wm.new_host.clear();
                    self.open_world(w, Some(p), false);
                    self.close_panel(Panel::Worlds);
                }
                WmAction::Duplicate(p) => {
                    let mut w = World::load(&p)?;
                    w.name = unique_name(&w.name);
                    w.save(&path_for(&w))?;
                    self.wm.message = format!("Kopie „{}“ angelegt.", w.name);
                }
                WmAction::Rename(p, new_name) => {
                    let new_name = new_name.trim().to_string();
                    if new_name.is_empty() {
                        return Err("Name darf nicht leer sein.".into());
                    }
                    let mut w = match self.view_index(&p) {
                        Some(i) => {
                            let vars = self.views[i].session.script.shared.borrow().variables.clone();
                            self.views[i].session.world.variables = vars;
                            self.views[i].session.world.clone()
                        }
                        None => World::load(&p)?,
                    };
                    w.name = new_name;
                    let new_path = path_for(&w);
                    if new_path != p && new_path.exists() {
                        return Err("Eine Welt mit diesem Namen gibt es schon.".into());
                    }
                    w.save(&new_path)?;
                    if new_path != p {
                        let _ = std::fs::remove_file(&p);
                    }
                    if let Some(i) = self.view_index(&p) {
                        self.views[i].session.world.name = w.name.clone();
                        self.views[i].session.world_path = Some(new_path);
                    }
                    self.wm.rename = None;
                }
                WmAction::Delete(p) => {
                    if let Some(i) = self.view_index(&p) {
                        self.close_view(i, false);
                    }
                    std::fs::remove_file(&p).map_err(|e| e.to_string())?;
                    self.wm.confirm_delete = None;
                }
                WmAction::Import => {
                    if let Some(src) = rfd::FileDialog::new().add_filter("rustmud-Welt", &["toml"]).pick_file() {
                        let mut w = World::load(&src)?;
                        w.name = unique_name(&w.name);
                        w.save(&path_for(&w))?;
                        self.wm.message = format!("„{}“ importiert.", w.name);
                    }
                }
                WmAction::Export(p) => {
                    if let Some(i) = self.view_index(&p) {
                        self.views[i].session.save_world()?;
                    }
                    let w = World::load(&p)?;
                    if let Some(dst) = rfd::FileDialog::new()
                        .add_filter("rustmud-Welt", &["toml"])
                        .set_file_name(w.file_name())
                        .save_file()
                    {
                        w.save(&dst)?;
                        self.wm.message = format!("Exportiert nach {}", dst.display());
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.wm.message = format!("Fehler: {e}");
        }
        self.wm.worlds = None;
    }
    // ------------------------------------------------------------ Andockbereich

    fn dock(&mut self, ctx: &egui::Context) {
        if self.panels.is_empty() {
            return;
        }
        let current = match self.panel {
            Some(p) if self.panels.contains(&p) => p,
            _ => self.panels[0],
        };
        self.panel = Some(current);
        let panels = self.panels.clone();
        let mut activate = None;
        let mut close = None;
        egui::SidePanel::right("dock")
            .resizable(true)
            .default_width(520.0)
            .min_width(320.0)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for p in &panels {
                        if ui.selectable_label(*p == current, p.title()).clicked() {
                            activate = Some(*p);
                        }
                        if ui.small_button("✖").on_hover_text("Schließen").clicked() {
                            close = Some(*p);
                        }
                        ui.add_space(6.0);
                    }
                });
                ui.separator();
                ScrollArea::vertical()
                    .id_salt(("dock", current.title()))
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.panel_ui(ui, current));
            });
        if let Some(p) = activate {
            self.panel = Some(p);
        }
        if let Some(p) = close {
            self.close_panel(p);
        }
    }

    fn panel_ui(&mut self, ui: &mut egui::Ui, p: Panel) {
        match p {
            Panel::Worlds => return self.world_manager_ui(ui),
            Panel::About => return self.about_ui(ui),
            _ => {}
        }
        let active = self.active;
        let Some(v) = self.views.get_mut(active) else {
            ui.label(RichText::new("Keine Welt geöffnet.").weak());
            return;
        };
        ui.label(RichText::new(format!("Welt: {}", v.session.world.name)).weak());
        let mut save = false;
        match p {
            Panel::Settings => {
                if ui_editors::world_settings(ui, &mut v.session) {
                    v.session.reload_script();
                }
                ui.separator();
                if ui.button("💾 Welt speichern").clicked() {
                    save = true;
                }
            }
            Panel::Triggers => ui_editors::triggers(ui, &mut v.session, &mut v.trg),
            Panel::Aliases => ui_editors::aliases(ui, &mut v.session, &mut v.ali),
            Panel::Timers => ui_editors::timers(ui, &mut v.session, &mut v.tim),
            Panel::Variables => ui_editors::variables(ui, &mut v.session, &mut v.vars),
            Panel::Keypad => ui_editors::keypad(ui, &mut v.session),
            Panel::Buttons => ui_editors::buttons(ui, &mut v.session, &mut v.button_sel),
            Panel::Plugins => ui_editors::plugins(ui, &mut v.session, &mut self.plugin_ui),
            Panel::Map => {
                let w = &mut v.session.world;
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut w.map_capture, "Karte aus der Ausgabe holen");
                    ui.checkbox(&mut w.map_fit, "An Fenster anpassen");
                    ui.add_enabled(
                        !w.map_fit,
                        egui::Slider::new(&mut w.map_font_size, 8.0..=40.0).text("Größe"),
                    );
                });
                if !w.map_capture {
                    ui.label(
                        RichText::new(
                            "Ist das eingeschaltet, wird eine ASCII-Karte am Zeilenanfang (Rahmen +----+ und Zeilen |....|) \
                             aus der Ausgabe herausgeschnitten und hier angezeigt.",
                        )
                        .weak(),
                    );
                }
                // Karte wächst mit der Erfahrung – Schriftgröße so wählen, dass sie ganz hineinpasst
                let cols = v
                    .session
                    .map
                    .iter()
                    .map(|l| l.plain().chars().count())
                    .max()
                    .unwrap_or(1)
                    .max(1) as f32;
                let rows = v.session.map.len().max(1) as f32;
                let size = if w.map_fit {
                    let probe = FontId::monospace(20.0);
                    let (cw, rh) = ui.fonts(|f| (f.glyph_width(&probe, 'M') / 20.0, f.row_height(&probe) / 20.0));
                    let avail = ui.available_size() - egui::vec2(24.0, 24.0);
                    let by_w = avail.x / (cols * cw);
                    let by_h = (avail.y.max(200.0)) / (rows * rh);
                    by_w.min(by_h).clamp(6.0, 60.0)
                } else {
                    w.map_font_size
                };
                let font = FontId::monospace(size);
                if !v.session.map.is_empty() {
                    ui.label(
                        RichText::new(format!("{} × {} Zeichen", cols as usize, rows as usize))
                            .weak()
                            .small(),
                    );
                }
                if v.session.map.is_empty() {
                    ui.label(RichText::new("Noch keine Karte empfangen.").weak());
                } else {
                    let bg = Color32::from_rgb(DEFAULT_BG.0, DEFAULT_BG.1, DEFAULT_BG.2);
                    egui::Frame::new().fill(bg).inner_margin(8.0).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for line in &v.session.map {
                            let mut job = LayoutJob::default();
                            job.wrap.max_width = f32::INFINITY;
                            line_job(line, &font, &mut job);
                            ui.add(egui::Label::new(job).extend());
                        }
                    });
                }
            }
            Panel::Gmcp => {
                ui.heading("GMCP");
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
            }
            Panel::Worlds | Panel::About => {}
        }
        if save {
            self.save_active();
        }
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

fn state_color(s: ConnState) -> Color32 {
    match s {
        ConnState::Connected => Color32::LIGHT_GREEN,
        ConnState::Connecting => Color32::YELLOW,
        ConnState::Disconnected => Color32::from_rgb(230, 90, 90),
    }
}

/// Farbiger Statuspunkt (die Standardschrift hat kein „●“).
fn status_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.5, color);
}

fn status_text(s: &Session) -> RichText {
    let text = match s.state {
        ConnState::Connected => "Verbunden",
        ConnState::Connecting => "Verbinde …",
        ConnState::Disconnected => "Getrennt",
    };
    RichText::new(text).color(state_color(s.state))
}

fn input_id(active: usize) -> egui::Id {
    egui::Id::new(("input", active))
}

impl eframe::App for MudApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let empty = self.views.get(self.active).is_none_or(|v| v.input.is_empty());
        crate::keypad::fallback_filter(raw_input, empty);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut next = std::time::Duration::from_millis(250);
        for v in &mut self.views {
            v.session.poll();
            next = next.min(v.session.tick());
        }
        ctx.request_repaint_after(next);

        // Ziffernblock: nur wenn Eingabezeile oder nichts den Fokus hat
        let focused = ctx.memory(|m| m.focused());
        let keypad_ok = focused.is_none() || focused == Some(input_id(self.active));
        let enabled = self
            .views
            .get(self.active)
            .is_some_and(|v| v.session.world.keypad_enabled);
        crate::keypad::set_active(enabled, keypad_ok);
        let keys = crate::keypad::take();
        if let Some(v) = self.views.get_mut(self.active) {
            for k in keys {
                v.session.keypad(k);
            }
        }

        // Kartenfenster beim ersten Empfang einer Karte automatisch öffnen
        if let Some(v) = self.views.get_mut(self.active) {
            if v.session.map_version > 0 && v.map_seen == 0 {
                v.map_seen = v.session.map_version;
                if !self.panels.contains(&Panel::Map) {
                    self.panels.push(Panel::Map);
                    self.panel.get_or_insert(Panel::Map);
                }
            }
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.menu_bar(ui);
            if self.settings.show_toolbar {
                ui.separator();
                self.toolbar(ui);
            }
            if !self.views.is_empty() {
                ui.separator();
                ui.horizontal(|ui| {
                    let mut close = None;
                    for (i, v) in self.views.iter().enumerate() {
                        status_dot(ui, state_color(v.session.state));
                        if ui.selectable_label(i == self.active, &v.session.world.name).clicked() {
                            self.active = i;
                        }
                        if ui.small_button("✖").on_hover_text("Welt schließen").clicked() {
                            close = Some(i);
                        }
                        ui.separator();
                    }
                    if let Some(i) = close {
                        self.close_view(i, true);
                    }
                });
            }
        });
        self.update_banner(ctx);

        // Statusleiste und Eingabezeile (volle Breite, unter dem Andockbereich)
        if !self.views.is_empty() {
            let v = &mut self.views[self.active];
            let font = FontId::monospace(v.session.world.font_size);
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    status_dot(ui, state_color(v.session.state));
                    ui.label(status_text(&v.session));
                    ui.separator();
                    ui.label(format!("{}:{}", v.session.world.host, v.session.world.port));
                    if v.session.mccp {
                        ui.separator();
                        ui.label("MCCP");
                    }
                    if v.session.gmcp_tree.as_object().is_some_and(|m| !m.is_empty()) {
                        ui.separator();
                        ui.label("GMCP");
                    }
                    if !v.session.msdp_tree.is_empty() {
                        ui.separator();
                        ui.label("MSDP");
                    }
                    if v.session.is_logging() {
                        ui.separator();
                        ui.label("LOG");
                    }
                    ui.separator();
                    ui.label(format!("{} Zeilen", v.session.lines.len()));
                    if !self.status.is_empty() {
                        ui.separator();
                        ui.label(RichText::new(&self.status).weak());
                    }
                });
            });

            // ---------------------------------------------------- Eingabezeile
            egui::TopBottomPanel::bottom("input").show(ctx, |ui| {
                let id = input_id(self.active);
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
                        .hint_text(if v.session.hide_input { "Passwort …" } else { "" })
                        .desired_width(f32::INFINITY)
                        .lock_focus(true),
                );
                if hist_changed {
                    if let Some(mut st) = TextEdit::load_state(ctx, id) {
                        let n = v.input.chars().count();
                        st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(n))));
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
        }

        // Angedockte Fenster rechts
        self.dock(ctx);

        if self.views.is_empty() {
            let mut open_wm = false;
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    if ui
                        .button(RichText::new("🌍 Welt öffnen oder anlegen").size(20.0))
                        .clicked()
                    {
                        open_wm = true;
                    }
                });
            });
            if open_wm {
                self.open_panel(Panel::Worlds);
            }
            return;
        }
        let v = &mut self.views[self.active];
        let font = FontId::monospace(v.session.world.font_size);
        // ---------------------------------------------------- Ausgabe
        let bg = Color32::from_rgb(DEFAULT_BG.0, DEFAULT_BG.1, DEFAULT_BG.2);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg).inner_margin(4.0))
            .show(ctx, |ui| {
                let (row_h, char_w) = ui.fonts(|f| (f.row_height(&font), f.glyph_width(&font, 'M')));
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
                            let line = if i < s.lines.len() { &s.lines[i] } else { &s.current };
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
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_state();
        for v in &mut self.views {
            v.session.disconnect();
        }
    }
}
