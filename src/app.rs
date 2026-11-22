//! Hauptfenster: Icon-Leiste, Welt-Tabs, Ausgabe, Eingabezeile, Menüs,
//! Weltenverwaltung, Update-Hinweis und Statusleiste.

use crate::session::{ConnState, Session, Waker};
use crate::ui_editors::{self, EditorState, VarState};
use crate::updater::{self, UpdateState};
use crate::world::{config_dir, list_worlds, path_for, unique_name, Encoding, World};
use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{Button, Color32, FontId, Key, RichText, ScrollArea, Stroke, TextEdit, TextFormat};
use egui_dock::{DockArea, DockState};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::ui_widgets::{line_job, DEFAULT_BG};
const HISTORY_MAX: usize = 500;

/// Alles, was im frei anordnbaren Fensterbereich erscheint – auch die Ausgabe selbst.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Tab {
    Output,
    Worlds,
    Settings,
    Triggers,
    Aliases,
    Timers,
    Variables,
    Keypad,
    Buttons,
    Plugins,
    Gmcp,
    About,
    /// Fenster eines Plugins (Kennung des Fensters)
    Plugin(String),
}

impl Tab {
    fn title(&self) -> String {
        match self {
            Tab::Output => "📜 Ausgabe",
            Tab::Worlds => "🌍 Welten",
            Tab::Settings => "⚙ Einstellungen",
            Tab::Triggers => "⚡ Trigger",
            Tab::Aliases => "⌨ Aliase",
            Tab::Timers => "⏱ Timer",
            Tab::Variables => "📋 Variablen",
            Tab::Keypad => "🔢 Ziffernblock",
            Tab::Buttons => "🔘 Knöpfe",
            Tab::Plugins => "📦 Plugins",
            Tab::Gmcp => "📡 GMCP",
            Tab::About => "Über rustmud",
            Tab::Plugin(id) => return id.clone(),
        }
        .to_string()
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
    /// Ausgabe folgt neuen Zeilen/Seiten (false, solange der Benutzer hochgescrollt hat)
    follow: bool,
    seen_page_version: u64,
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
            follow: true,
            seen_page_version: 0,
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
    /// Fensteranordnung (wird beim Anzeigen kurz herausgenommen)
    dock: Option<DockState<Tab>>,
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
        apply_theme(&cc.egui_ctx);

        let installed = crate::plugins::install_bundled();
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
            dock: Some(load_layout()),
            plugin_ui: Default::default(),
            update: Arc::new(Mutex::new(UpdateState::Idle)),
            update_dismissed: false,
            status: if installed.is_empty() {
                String::new()
            } else {
                format!("Mitgelieferte Plugins installiert: {}", installed.join(", "))
            },
        };
        for p in app.settings.open_worlds.clone() {
            if let Ok(w) = World::load(&p) {
                app.open_world(w, Some(p), false);
            }
        }
        app.active = app.settings.active.min(app.views.len().saturating_sub(1));
        if app.views.is_empty() {
            app.open_tab(Tab::Worlds);
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
        if let Some(d) = &self.dock {
            save_layout(d);
        }
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

    fn dock_mut(&mut self) -> &mut DockState<Tab> {
        self.dock.get_or_insert_with(default_layout)
    }

    fn is_open(&self, t: &Tab) -> bool {
        self.dock.as_ref().is_some_and(|d| d.find_tab(t).is_some())
    }

    /// Fenster öffnen bzw. nach vorne holen. Neue Fenster kommen rechts neben die Ausgabe.
    fn open_tab(&mut self, t: Tab) {
        if t == Tab::Worlds {
            self.wm.worlds = None;
        }
        let dock = self.dock_mut();
        if let Some(loc) = dock.find_tab(&t) {
            dock.set_active_tab(loc);
            dock.set_focused_node_and_surface((loc.0, loc.1));
            return;
        }
        // Gibt es im Hauptbereich schon einen Bereich neben der Ausgabe? Dann dorthin.
        let output = dock.find_main_surface_tab(&Tab::Output).map(|(n, _)| n);
        let side = dock
            .iter_all_tabs()
            .find(|((surface, node), tab)| **tab != Tab::Output && surface.is_main() && Some(*node) != output)
            .map(|(loc, _)| loc);
        // Plugin-Fenster (Karte, Spielstand …) untereinander statt als Tabs hintereinander
        let plugin_leaf = dock
            .iter_all_tabs()
            .find(|((surface, _), tab)| surface.is_main() && matches!(tab, Tab::Plugin(_)))
            .map(|((_, node), _)| node);
        if let (Tab::Plugin(_), Some(node)) = (&t, plugin_leaf) {
            dock.main_surface_mut().split_below(node, 0.45, vec![t]);
            return;
        }
        match (side, output) {
            (Some(loc), _) => {
                dock.set_focused_node_and_surface(loc);
                dock.push_to_focused_leaf(t);
            }
            (None, Some(out)) => {
                dock.main_surface_mut().split_right(out, 0.62, vec![t]);
            }
            (None, None) => dock.push_to_first_leaf(t),
        }
    }

    fn close_tab(&mut self, t: Tab) {
        let dock = self.dock_mut();
        if let Some(loc) = dock.find_tab(&t) {
            dock.remove_tab(loc);
        }
    }

    /// Icon-Leiste: geöffnetes Fenster schließen, sonst öffnen.
    fn toggle_tab(&mut self, t: Tab) {
        if self.is_open(&t) {
            self.close_tab(t);
        } else {
            self.open_tab(t);
        }
    }

    fn reset_layout(&mut self) {
        self.dock = Some(default_layout());
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
        let mut req: Option<Tab> = None;
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Welt", |ui| {
                if ui.button("🌍 Weltenverwaltung …").clicked() {
                    req = Some(Tab::Worlds);
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
                    (Tab::Settings, "⚙ Welt-Einstellungen …"),
                    (Tab::Triggers, "⚡ Trigger …"),
                    (Tab::Aliases, "⌨ Aliase …"),
                    (Tab::Timers, "⏱ Timer …"),
                    (Tab::Variables, "📋 Variablen …"),
                    (Tab::Keypad, "🔢 Ziffernblock …"),
                    (Tab::Buttons, "🔘 Eigene Knöpfe …"),
                    (Tab::Plugins, "📦 Plugins …"),
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
                    req = Some(Tab::Gmcp);
                    ui.close_menu();
                }
                if ui.button("Fensteranordnung zurücksetzen").clicked() {
                    self.reset_layout();
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
                    req = Some(Tab::About);
                    ui.close_menu();
                }
                if ui.button("Projektseite (GitHub)").clicked() {
                    updater::open_url(&format!("https://github.com/{}", updater::REPO));
                    ui.close_menu();
                }
                if ui.button("Über rustmud / Programmeinstellungen …").clicked() {
                    req = Some(Tab::About);
                    ui.close_menu();
                }
            });
        });
        if let Some(p) = req {
            self.open_tab(p);
        }
    }

    // ------------------------------------------------------------ Icon-Leiste

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let big = |s: &str| RichText::new(s).size(18.0);
        let mut toggle: Option<Tab> = None;
        let mut save = false;
        let open_tabs: Vec<Tab> = self
            .dock
            .as_ref()
            .map(|d| d.iter_all_tabs().map(|(_, t)| t.clone()).collect())
            .unwrap_or_default();
        let is_open = |p: &Tab| open_tabs.contains(p);
        ui.horizontal(|ui| {
            if ui
                .selectable_label(is_open(&Tab::Worlds), big("🌍"))
                .on_hover_text("Weltenverwaltung")
                .clicked()
            {
                toggle = Some(Tab::Worlds);
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
                (Tab::Settings, "⚙", "Welt-Einstellungen"),
                (Tab::Triggers, "⚡", "Trigger"),
                (Tab::Aliases, "⌨", "Aliase"),
                (Tab::Timers, "⏱", "Timer"),
                (Tab::Variables, "📋", "Variablen"),
                (Tab::Keypad, "🔢", "Ziffernblock"),
                (Tab::Plugins, "📦", "Plugins"),
            ] {
                if ui.selectable_label(is_open(&p), big(icon)).on_hover_text(tip).clicked() {
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
                .selectable_label(is_open(&Tab::Gmcp), big("📡"))
                .on_hover_text("GMCP/MSDP-Daten")
                .clicked()
            {
                toggle = Some(Tab::Gmcp);
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
                toggle = Some(Tab::Buttons);
            }
        });
        if save {
            self.save_active();
        }
        if let Some(p) = toggle {
            self.toggle_tab(p);
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
                    self.open_tab(Tab::About);
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
                    self.close_tab(Tab::Worlds);
                }
                WmAction::Edit(p) => {
                    if self.view_index(&p).is_none() {
                        let w = World::load(&p)?;
                        self.open_world(w, Some(p.clone()), false);
                    }
                    let i = self.view_index(&p).unwrap();
                    self.active = i;
                    self.open_tab(Tab::Settings);
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
                    self.close_tab(Tab::Worlds);
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
    // ------------------------------------------------------------ Fensterinhalte

    fn panel_ui(&mut self, ui: &mut egui::Ui, p: Tab) {
        match p {
            Tab::Worlds => return self.world_manager_ui(ui),
            Tab::About => return self.about_ui(ui),
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
            Tab::Settings => {
                if ui_editors::world_settings(ui, &mut v.session) {
                    v.session.reload_script();
                }
                ui.separator();
                if ui.button("💾 Welt speichern").clicked() {
                    save = true;
                }
            }
            Tab::Triggers => ui_editors::triggers(ui, &mut v.session, &mut v.trg),
            Tab::Aliases => ui_editors::aliases(ui, &mut v.session, &mut v.ali),
            Tab::Timers => ui_editors::timers(ui, &mut v.session, &mut v.tim),
            Tab::Variables => ui_editors::variables(ui, &mut v.session, &mut v.vars),
            Tab::Keypad => ui_editors::keypad(ui, &mut v.session),
            Tab::Buttons => ui_editors::buttons(ui, &mut v.session, &mut v.button_sel),
            Tab::Plugins => ui_editors::plugins(ui, &mut v.session, &mut self.plugin_ui),
            Tab::Gmcp => {
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
            Tab::Worlds | Tab::About | Tab::Output | Tab::Plugin(_) => {}
        }
        if save {
            self.save_active();
        }
    }

    /// Die Spielausgabe der gewählten Welt.
    fn output_ui(&mut self, ui: &mut egui::Ui) {
        let Some(v) = self.views.get_mut(self.active) else {
            let mut open_wm = false;
            ui.centered_and_justified(|ui| {
                if ui
                    .button(RichText::new("🌍 Welt öffnen oder anlegen").size(20.0))
                    .clicked()
                {
                    open_wm = true;
                }
            });
            if open_wm {
                self.open_tab(Tab::Worlds);
            }
            return;
        };
        let font = FontId::monospace(v.session.world.font_size);
        let bg = Color32::from_rgb(DEFAULT_BG.0, DEFAULT_BG.1, DEFAULT_BG.2);
        egui::Frame::new().fill(bg).inner_margin(6.0).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            let (row_h, char_w) = ui.fonts(|f| (f.row_height(&font), f.glyph_width(&font, 'M')));
            let avail = ui.available_size();
            let cols = (avail.x / char_w.max(1.0)).floor().clamp(20.0, 500.0) as u16;
            let rows = (avail.y / row_h.max(1.0)).floor().clamp(5.0, 300.0) as u16;
            v.session.set_term_size(cols, rows);

            // Seitenansicht: ab Seitenbeginn (z. B. neuer Raum) steht die Ausgabe oben im Fenster,
            // der Verlauf liegt darüber und erscheint beim Hochscrollen.
            if v.session.page_version != v.seen_page_version {
                v.seen_page_version = v.session.page_version;
                v.follow = true;
            }
            let s = &v.session;
            let has_current = !s.current.is_empty();
            let n = s.lines.len() + usize::from(has_current);
            let anchor = s.page_start_index();
            let vis_rows = (avail.y / row_h.max(1.0)).ceil() as usize + 1;
            // Leerzeilen unten, damit der Seitenbeginn ganz nach oben gescrollt werden kann
            let total = match anchor {
                Some(a) => n.max(a + vis_rows),
                None => n,
            };
            let bottom = (n as f32 * row_h - avail.y).max(0.0);
            let target = match anchor {
                Some(a) => (a as f32 * row_h).max(bottom),
                None => bottom,
            };
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut area = ScrollArea::both()
                .id_salt(("output", self.active))
                .auto_shrink([false, false]);
            if v.follow {
                area = area.vertical_scroll_offset(target);
            }
            let out = area.show_rows(ui, row_h, total, |ui, range| {
                for i in range {
                    let mut job = LayoutJob::default();
                    job.wrap.max_width = f32::INFINITY;
                    let line = if i < s.lines.len() {
                        Some(&s.lines[i])
                    } else if i < n {
                        Some(&s.current)
                    } else {
                        None
                    };
                    match line {
                        Some(l) if !l.spans.is_empty() => line_job(l, &font, &mut job),
                        _ => job.append(
                            " ",
                            0.0,
                            TextFormat {
                                font_id: font.clone(),
                                ..Default::default()
                            },
                        ),
                    }
                    ui.add(egui::Label::new(job).extend());
                }
            });
            // Hat der Benutzer gescrollt? Dann nicht mehr mitspringen, bis er wieder unten/an der Seite ist.
            let actual = out.state.offset.y;
            if v.follow {
                if (actual - target).abs() > row_h * 0.5 {
                    v.follow = false;
                }
            } else if actual >= target - row_h * 0.5 {
                v.follow = true;
            }
        });
    }

    /// Inhalt eines Plugin-Fensters: Text, Überschriften, Balken, Wertepaare.
    fn plugin_window_ui(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(v) = self.views.get(self.active) else {
            ui.label(RichText::new("Keine Welt geöffnet.").weak());
            return;
        };
        let Some(w) = v.session.windows.get(id) else {
            ui.label(RichText::new("Dieses Fenster gibt es in der aktuellen Welt nicht (Plugin aus?).").weak());
            return;
        };
        crate::ui_widgets::plugin_window(ui, w);
    }
}

/// Verbindet egui_dock mit den Fensterinhalten der App.
struct Viewer<'a> {
    app: &'a mut MudApp,
}

impl egui_dock::TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        match tab {
            Tab::Plugin(id) => self
                .app
                .views
                .get(self.app.active)
                .and_then(|v| v.session.windows.get(id.as_str()))
                .map(|w| w.title.clone())
                .unwrap_or_else(|| id.clone())
                .into(),
            Tab::Output => match self.app.views.get(self.app.active) {
                Some(v) => format!("📜 {}", v.session.world.name).into(),
                None => tab.title().into(),
            },
            t => t.title().into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::Output => self.app.output_ui(ui),
            Tab::Plugin(id) => {
                let id = id.clone();
                self.app.plugin_window_ui(ui, &id);
            }
            t => {
                let t = t.clone();
                egui::Frame::new()
                    .inner_margin(8.0)
                    .show(ui, |ui| self.app.panel_ui(ui, t));
            }
        }
    }

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(("tab", format!("{tab:?}")))
    }

    fn closeable(&mut self, tab: &mut Tab) -> bool {
        *tab != Tab::Output
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab {
            Tab::Output => [false, false],
            _ => [false, true],
        }
    }

    fn clear_background(&self, tab: &Tab) -> bool {
        *tab != Tab::Output
    }
}

fn layout_path() -> PathBuf {
    config_dir().join("layout.json")
}

fn default_layout() -> DockState<Tab> {
    DockState::new(vec![Tab::Output])
}

fn load_layout() -> DockState<Tab> {
    std::fs::read_to_string(layout_path())
        .ok()
        .and_then(|s| serde_json::from_str::<DockState<Tab>>(&s).ok())
        .filter(|d| d.find_tab(&Tab::Output).is_some())
        .unwrap_or_else(default_layout)
}

fn save_layout(d: &DockState<Tab>) {
    if let Ok(text) = serde_json::to_string_pretty(d) {
        let _ = std::fs::create_dir_all(config_dir());
        let _ = std::fs::write(layout_path(), text);
    }
}

/// Akzentfarbe (Rost-Orange wie im Programmsymbol).
pub const ACCENT: Color32 = Color32::from_rgb(206, 120, 50);

/// Dunkles Design mit abgerundeten Ecken und Akzentfarbe.
pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(26, 28, 33);
    v.window_fill = Color32::from_rgb(32, 34, 40);
    v.extreme_bg_color = Color32::from_rgb(14, 15, 18);
    v.faint_bg_color = Color32::from_rgb(36, 38, 45);
    v.selection.bg_fill = ACCENT.linear_multiply(0.55);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    v.hyperlink_color = ACCENT;
    v.window_corner_radius = egui::CornerRadius::same(8);
    v.menu_corner_radius = egui::CornerRadius::same(6);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(5);
    }
    v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(5);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT.linear_multiply(0.8));
    v.widgets.active.bg_fill = ACCENT.linear_multiply(0.7);
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(7.0, 3.0);
    ctx.set_style(style);
}

fn dock_style(ctx: &egui::Context) -> egui_dock::Style {
    let mut st = egui_dock::Style::from_egui(&ctx.style());
    st.tab_bar.bg_fill = Color32::from_rgb(20, 21, 25);
    st.tab_bar.height = 26.0;
    st.tab.active.bg_fill = Color32::from_rgb(40, 43, 50);
    st.tab.active.text_color = Color32::WHITE;
    st.tab.focused.bg_fill = Color32::from_rgb(48, 44, 40);
    st.tab.focused.text_color = ACCENT;
    st.tab.inactive.bg_fill = Color32::from_rgb(26, 28, 33);
    st.tab.hovered.bg_fill = Color32::from_rgb(44, 46, 54);
    st.tab.tab_body.bg_fill = Color32::from_rgb(28, 30, 35);
    st.separator.color_idle = Color32::from_rgb(45, 47, 55);
    st.separator.color_hovered = ACCENT.linear_multiply(0.7);
    st.separator.color_dragged = ACCENT;
    st.overlay.selection_color = ACCENT.linear_multiply(0.35);
    st.main_surface_border_stroke = Stroke::NONE;
    st
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

        // Plugins möchten Fenster anzeigen (WindowShow)
        let requests: Vec<String> = self
            .views
            .get_mut(self.active)
            .map(|v| std::mem::take(&mut v.session.window_requests))
            .unwrap_or_default();
        for id in requests {
            if !self.is_open(&Tab::Plugin(id.clone())) {
                self.open_tab(Tab::Plugin(id));
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
                    v.follow = true;
                    v.focus_input = true;
                }
                if v.focus_input {
                    resp.request_focus();
                    v.focus_input = false;
                }
            });
        }

        // Frei anordnbarer Fensterbereich (Ausgabe, Editoren, Plugin-Fenster)
        let mut dock = self.dock.take().unwrap_or_else(default_layout);
        let style = dock_style(ctx);
        DockArea::new(&mut dock)
            .style(style)
            .show_leaf_collapse_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show(ctx, &mut Viewer { app: self });
        // Die Ausgabe darf nie ganz verschwinden
        if dock.find_tab(&Tab::Output).is_none() {
            dock.push_to_first_leaf(Tab::Output);
        }
        self.dock = Some(dock);

        // Hat nichts den Fokus (z. B. nach Klick in die Ausgabe), bekommt ihn die Eingabezeile zurück
        if let Some(v) = self.views.get_mut(self.active) {
            if ctx.memory(|m| m.focused().is_none()) {
                v.focus_input = true;
                ctx.request_repaint();
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_state();
        for v in &mut self.views {
            v.session.disconnect();
        }
    }
}
