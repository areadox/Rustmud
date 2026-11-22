//! Dialoge zum Bearbeiten von Triggern, Aliassen, Timern, Variablen und Welteinstellungen.

use crate::automation::compile;
use crate::session::Session;
use crate::world::{default_keypad, Alias, Encoding, SendTo, Timer, Trigger, UserButton};
use egui::{ComboBox, DragValue, Grid, RichText, ScrollArea, TextEdit, Ui};

#[derive(Default)]
pub struct EditorState {
    pub selected: Option<usize>,
    pub test_line: String,
    pub filter: String,
}

fn send_to_combo(ui: &mut Ui, id: &str, v: &mut SendTo) -> bool {
    let mut changed = false;
    ComboBox::from_id_salt(id).selected_text(v.label()).show_ui(ui, |ui| {
        for s in SendTo::ALL {
            changed |= ui.selectable_value(v, s, s.label()).changed();
        }
    });
    changed
}

/// Liste links, Details rechts. Liefert `true`, wenn sich etwas geändert hat.
fn list_panel<T>(
    ui: &mut Ui,
    items: &mut Vec<T>,
    st: &mut EditorState,
    label: impl Fn(&T) -> String,
    enabled: impl Fn(&mut T) -> &mut bool,
    new_item: impl Fn() -> T,
) -> bool
where
    T: Clone,
{
    let mut changed = false;
    ui.horizontal(|ui| {
        if ui.button("➕ Neu").clicked() {
            items.push(new_item());
            st.selected = Some(items.len() - 1);
            changed = true;
        }
        let has_sel = st.selected.is_some_and(|i| i < items.len());
        if ui.add_enabled(has_sel, egui::Button::new("🗐 Kopieren")).clicked() {
            let i = st.selected.unwrap();
            let c = items[i].clone();
            items.insert(i + 1, c);
            st.selected = Some(i + 1);
            changed = true;
        }
        if ui.add_enabled(has_sel, egui::Button::new("🗑 Löschen")).clicked() {
            let i = st.selected.unwrap();
            items.remove(i);
            st.selected = if items.is_empty() {
                None
            } else {
                Some(i.min(items.len() - 1))
            };
            changed = true;
        }
        ui.add(
            TextEdit::singleline(&mut st.filter)
                .hint_text("Filter …")
                .desired_width(120.0),
        );
    });
    ui.separator();
    ScrollArea::vertical()
        .id_salt("list")
        .max_height(180.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let filter = st.filter.to_lowercase();
            for (i, it) in items.iter_mut().enumerate() {
                let text = label(it);
                if !filter.is_empty() && !text.to_lowercase().contains(&filter) {
                    continue;
                }
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(enabled(it), "").changed();
                    if ui.selectable_label(st.selected == Some(i), text).clicked() {
                        st.selected = Some(i);
                    }
                });
            }
        });
    ui.separator();
    changed
}

fn label_of(name: &str, pattern: &str, group: &str) -> String {
    let mut s = if name.is_empty() {
        pattern.to_string()
    } else {
        format!("{name}  —  {pattern}")
    };
    if !group.is_empty() {
        s.push_str(&format!("  [{group}]"));
    }
    s
}

fn pattern_check(ui: &mut Ui, pattern: &str, regex: bool, ignore_case: bool, test: &mut String) {
    if pattern.is_empty() {
        return;
    }
    match compile(pattern, regex, ignore_case) {
        Err(e) => {
            ui.label(RichText::new(format!("Fehler im Muster: {e}")).color(egui::Color32::LIGHT_RED));
        }
        Ok(re) => {
            ui.horizontal(|ui| {
                ui.label("Test:");
                ui.add(
                    TextEdit::singleline(test)
                        .hint_text("Beispielzeile hier eingeben")
                        .desired_width(f32::INFINITY),
                );
            });
            if !test.is_empty() {
                match re.captures(test) {
                    Some(c) => {
                        let wc: Vec<String> = (1..c.len())
                            .map(|i| format!("%{i} = „{}“", c.get(i).map(|m| m.as_str()).unwrap_or("")))
                            .collect();
                        ui.label(
                            RichText::new(format!("✔ Treffer  {}", wc.join(", "))).color(egui::Color32::LIGHT_GREEN),
                        );
                    }
                    None => {
                        ui.label(RichText::new("❌ kein Treffer").color(egui::Color32::LIGHT_RED));
                    }
                }
            }
        }
    }
}

pub fn triggers(ui: &mut Ui, s: &mut Session, st: &mut EditorState) {
    let mut changed = list_panel(
        ui,
        &mut s.world.triggers,
        st,
        |t| label_of(&t.name, &t.pattern, &t.group),
        |t| &mut t.enabled,
        || Trigger {
            pattern: "*".into(),
            ..Default::default()
        },
    );
    if let Some(t) = st.selected.and_then(|i| s.world.triggers.get_mut(i)) {
        ui.scope(|ui| {
            Grid::new("trg").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Name");
                changed |= ui.text_edit_singleline(&mut t.name).changed();
                ui.end_row();
                ui.label("Gruppe");
                changed |= ui.text_edit_singleline(&mut t.group).changed();
                ui.end_row();
                ui.label("Muster");
                changed |= ui
                    .add(
                        TextEdit::singleline(&mut t.pattern)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    )
                    .changed();
                ui.end_row();
                ui.label("");
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut t.regex, "Regulärer Ausdruck").changed();
                    changed |= ui.checkbox(&mut t.ignore_case, "Groß/Klein egal").changed();
                });
                ui.end_row();
                ui.label("Senden");
                changed |= ui
                    .add(
                        TextEdit::multiline(&mut t.send)
                            .code_editor()
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    )
                    .changed();
                ui.end_row();
                ui.label("Senden an");
                changed |= send_to_combo(ui, "trg_send_to", &mut t.send_to);
                ui.end_row();
                ui.label("Lua-Funktion");
                changed |= ui
                    .add(TextEdit::singleline(&mut t.script).hint_text("f(name, zeile, wildcards)"))
                    .changed();
                ui.end_row();
                ui.label("Farbe");
                changed |= ui
                    .add(TextEdit::singleline(&mut t.color).hint_text("z. B. gelb oder #ff8800"))
                    .changed();
                ui.end_row();
                ui.label("Reihenfolge");
                changed |= ui.add(DragValue::new(&mut t.sequence).range(0..=10000)).changed();
                ui.end_row();
                ui.label("Optionen");
                ui.vertical(|ui| {
                    changed |= ui.checkbox(&mut t.omit_from_output, "Zeile nicht anzeigen").changed();
                    changed |= ui.checkbox(&mut t.omit_from_log, "Zeile nicht loggen").changed();
                    changed |= ui
                        .checkbox(&mut t.keep_evaluating, "Weitere Trigger auswerten")
                        .changed();
                    changed |= ui.checkbox(&mut t.one_shot, "Nur einmal auslösen").changed();
                });
                ui.end_row();
            });
            pattern_check(ui, &t.pattern, t.regex, t.ignore_case, &mut st.test_line);
            ui.label(
                RichText::new("Im Sendetext: %0 = ganze Zeile, %1…%9 = Wildcards, @name = Variable, %% = %").weak(),
            );
        });
    }
    if changed {
        s.rebuild_automation_quiet();
    }
}

pub fn aliases(ui: &mut Ui, s: &mut Session, st: &mut EditorState) {
    let mut changed = list_panel(
        ui,
        &mut s.world.aliases,
        st,
        |a| label_of(&a.name, &a.pattern, &a.group),
        |a| &mut a.enabled,
        || Alias {
            pattern: "kürzel *".into(),
            ..Default::default()
        },
    );
    if let Some(a) = st.selected.and_then(|i| s.world.aliases.get_mut(i)) {
        ui.scope(|ui| {
            Grid::new("ali").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Name");
                changed |= ui.text_edit_singleline(&mut a.name).changed();
                ui.end_row();
                ui.label("Gruppe");
                changed |= ui.text_edit_singleline(&mut a.group).changed();
                ui.end_row();
                ui.label("Muster");
                changed |= ui
                    .add(
                        TextEdit::singleline(&mut a.pattern)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    )
                    .changed();
                ui.end_row();
                ui.label("");
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut a.regex, "Regulärer Ausdruck").changed();
                    changed |= ui.checkbox(&mut a.ignore_case, "Groß/Klein egal").changed();
                });
                ui.end_row();
                ui.label("Senden");
                changed |= ui
                    .add(
                        TextEdit::multiline(&mut a.send)
                            .code_editor()
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    )
                    .changed();
                ui.end_row();
                ui.label("Senden an");
                changed |= send_to_combo(ui, "ali_send_to", &mut a.send_to);
                ui.end_row();
                ui.label("Lua-Funktion");
                changed |= ui
                    .add(TextEdit::singleline(&mut a.script).hint_text("f(name, zeile, wildcards)"))
                    .changed();
                ui.end_row();
                ui.label("Reihenfolge");
                changed |= ui.add(DragValue::new(&mut a.sequence).range(0..=10000)).changed();
                ui.end_row();
                ui.label("Optionen");
                ui.vertical(|ui| {
                    changed |= ui
                        .checkbox(&mut a.keep_evaluating, "Weitere Aliase auswerten")
                        .changed();
                    changed |= ui
                        .checkbox(&mut a.omit_from_output, "Gesendetes nicht anzeigen")
                        .changed();
                });
                ui.end_row();
            });
            pattern_check(ui, &a.pattern, a.regex, a.ignore_case, &mut st.test_line);
        });
    }
    if changed {
        s.rebuild_automation_quiet();
    }
}

pub fn timers(ui: &mut Ui, s: &mut Session, st: &mut EditorState) {
    let mut changed = list_panel(
        ui,
        &mut s.world.timers,
        st,
        |t| label_of(&t.name, &format!("alle {} s", t.interval), &t.group),
        |t| &mut t.enabled,
        Timer::default,
    );
    if let Some(t) = st.selected.and_then(|i| s.world.timers.get_mut(i)) {
        Grid::new("tim").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Name");
            changed |= ui.text_edit_singleline(&mut t.name).changed();
            ui.end_row();
            ui.label("Gruppe");
            changed |= ui.text_edit_singleline(&mut t.group).changed();
            ui.end_row();
            ui.label("Intervall (s)");
            changed |= ui
                .add(DragValue::new(&mut t.interval).range(0.1..=86400.0).speed(0.5))
                .changed();
            ui.end_row();
            ui.label("Senden");
            changed |= ui
                .add(
                    TextEdit::multiline(&mut t.send)
                        .code_editor()
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
                .changed();
            ui.end_row();
            ui.label("Senden an");
            changed |= send_to_combo(ui, "tim_send_to", &mut t.send_to);
            ui.end_row();
            ui.label("Lua-Funktion");
            changed |= ui.text_edit_singleline(&mut t.script).changed();
            ui.end_row();
            ui.label("Optionen");
            ui.vertical(|ui| {
                changed |= ui.checkbox(&mut t.one_shot, "Nur einmal").changed();
                changed |= ui.checkbox(&mut t.only_when_connected, "Nur wenn verbunden").changed();
            });
            ui.end_row();
        });
    }
    if changed {
        s.rebuild_automation_quiet();
    }
}

#[derive(Default)]
pub struct VarState {
    pub new_name: String,
    pub new_value: String,
}

pub fn variables(ui: &mut Ui, s: &mut Session, st: &mut VarState) {
    let mut shared = s.script.shared.borrow_mut();
    let mut remove = None;
    ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        Grid::new("vars").num_columns(3).striped(true).show(ui, |ui| {
            for (k, v) in shared.variables.iter_mut() {
                ui.label(RichText::new(k).monospace());
                ui.add(TextEdit::singleline(v).desired_width(250.0));
                if ui.small_button("🗑").clicked() {
                    remove = Some(k.clone());
                }
                ui.end_row();
            }
        });
    });
    if let Some(k) = remove {
        shared.variables.remove(&k);
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(&mut st.new_name)
                .hint_text("Name")
                .desired_width(120.0),
        );
        ui.add(
            TextEdit::singleline(&mut st.new_value)
                .hint_text("Wert")
                .desired_width(200.0),
        );
        if ui.button("Hinzufügen").clicked() && !st.new_name.trim().is_empty() {
            shared
                .variables
                .insert(st.new_name.trim().to_string(), std::mem::take(&mut st.new_value));
            st.new_name.clear();
        }
    });
    ui.label(RichText::new("In Triggern/Aliassen als @name, in Lua per GetVariable(\"name\").").weak());
}

/// Liefert `true`, wenn „Skript neu laden“ gedrückt wurde.
pub fn world_settings(ui: &mut Ui, s: &mut Session) -> bool {
    let mut reload = false;
    let w = &mut s.world;
    let mut changed = false;
    ui.scope(|ui| {
        ui.heading("Verbindung");
        Grid::new("conn").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut w.name);
            ui.end_row();
            ui.label("Host");
            ui.text_edit_singleline(&mut w.host);
            ui.end_row();
            ui.label("Port");
            ui.add(DragValue::new(&mut w.port).range(1..=65535));
            ui.end_row();
            ui.label("Zeichensatz");
            ComboBox::from_id_salt("enc")
                .selected_text(match w.encoding {
                    Encoding::Latin1 => "Latin-1 (ISO-8859-1)",
                    Encoding::Utf8 => "UTF-8",
                })
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(&mut w.encoding, Encoding::Latin1, "Latin-1 (ISO-8859-1)")
                        .changed();
                    changed |= ui.selectable_value(&mut w.encoding, Encoding::Utf8, "UTF-8").changed();
                });
            ui.end_row();
            ui.label("Protokolle");
            ui.horizontal(|ui| {
                ui.checkbox(&mut w.mccp, "MCCP");
                ui.checkbox(&mut w.gmcp, "GMCP");
                ui.checkbox(&mut w.msdp, "MSDP");
            });
            ui.end_row();
            ui.label("GMCP-Module");
            let mut mods = w.gmcp_supports.join(", ");
            if ui.text_edit_singleline(&mut mods).changed() {
                w.gmcp_supports = mods
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            ui.end_row();
            ui.label("Nach Verbinden senden");
            ui.add(
                TextEdit::multiline(&mut w.connect_text)
                    .desired_rows(2)
                    .hint_text("eine Zeile pro Befehl, z. B. Charaktername"),
            );
            ui.end_row();
            ui.label("");
            ui.vertical(|ui| {
                ui.checkbox(&mut w.auto_connect, "Beim Öffnen verbinden");
                ui.checkbox(&mut w.auto_reconnect, "Automatisch neu verbinden");
            });
            ui.end_row();
        });
        ui.separator();
        ui.heading("Eingabe");
        Grid::new("input").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Befehlstrenner");
            ui.add(
                TextEdit::singleline(&mut w.command_stack)
                    .char_limit(1)
                    .desired_width(30.0),
            );
            ui.end_row();
            ui.label("Speedwalk-Präfix");
            ui.add(
                TextEdit::singleline(&mut w.speedwalk_prefix)
                    .char_limit(2)
                    .desired_width(30.0),
            );
            ui.end_row();
            ui.label("Lua-Präfix");
            ui.add(
                TextEdit::singleline(&mut w.script_prefix)
                    .char_limit(2)
                    .desired_width(30.0),
            );
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut w.echo_input, "Eingaben in der Ausgabe anzeigen");
            ui.end_row();
        });
        ui.separator();
        ui.heading("Ausgabe");
        Grid::new("out").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Schriftgröße");
            ui.add(DragValue::new(&mut w.font_size).range(8.0..=40.0).speed(0.5));
            ui.end_row();
            ui.label("Max. Zeilen");
            ui.add(DragValue::new(&mut w.max_lines).range(100..=1_000_000).speed(100));
            ui.end_row();
        });
        ui.separator();
        ui.heading("Skript & Log");
        Grid::new("script").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Lua-Datei");
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut w.script_file)
                        .hint_text("Pfad zur .lua-Datei")
                        .desired_width(280.0),
                );
                if ui.button("Neu laden").clicked() {
                    reload = true;
                }
            });
            ui.end_row();
            ui.label("Log");
            ui.vertical(|ui| {
                ui.checkbox(&mut w.logging, "Sitzung mitschreiben (ab nächster Verbindung)");
                ui.checkbox(&mut w.log_input, "Eingaben mitloggen");
            });
            ui.end_row();
            ui.label("Log-Ordner");
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut w.log_dir)
                        .hint_text(crate::world::default_log_dir().display().to_string())
                        .desired_width(240.0),
                );
                if ui.button("…").on_hover_text("Ordner auswählen").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        w.log_dir = p.display().to_string();
                    }
                }
            });
            ui.end_row();
        });
    });
    if changed {
        s.apply_settings();
    }
    reload
}

/// Zeigt einen JSON-Baum (GMCP/MSDP) aufklappbar an.
pub fn json_tree(ui: &mut Ui, key: &str, v: &serde_json::Value, id: &str) {
    use serde_json::Value;
    match v {
        Value::Object(map) => {
            egui::CollapsingHeader::new(key).id_salt(id).show(ui, |ui| {
                for (k, v) in map {
                    json_tree(ui, k, v, &format!("{id}.{k}"));
                }
            });
        }
        Value::Array(arr) => {
            egui::CollapsingHeader::new(format!("{key} [{}]", arr.len()))
                .id_salt(id)
                .show(ui, |ui| {
                    for (i, v) in arr.iter().enumerate() {
                        json_tree(ui, &i.to_string(), v, &format!("{id}[{i}]"));
                    }
                });
        }
        other => {
            let text = match other {
                Value::String(s) => s.clone(),
                o => o.to_string(),
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{key}:")).strong());
                ui.label(RichText::new(text).monospace());
            });
        }
    }
}

/// Ziffernblock-Belegung, angeordnet wie auf der Tastatur.
pub fn keypad(ui: &mut Ui, s: &mut Session) {
    let w = &mut s.world;
    ui.checkbox(&mut w.keypad_enabled, "Mit dem Ziffernblock laufen");
    ui.add_space(4.0);
    ui.add_enabled_ui(w.keypad_enabled, |ui| {
        Grid::new("keypad").num_columns(4).spacing([6.0, 6.0]).show(ui, |ui| {
            // Reihen wie auf einem Ziffernblock: / * -  |  7 8 9 +  |  4 5 6  |  1 2 3  |  0 .
            let rows: [&[&str]; 5] = [
                &["", "/", "*", "-"],
                &["7", "8", "9", "+"],
                &["4", "5", "6", ""],
                &["1", "2", "3", ""],
                &["0", "", ".", ""],
            ];
            for row in rows {
                for key in row {
                    if key.is_empty() {
                        ui.label("");
                        continue;
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new(*key).strong().monospace());
                        let v = w.keypad.entry(key.to_string()).or_default();
                        ui.add(TextEdit::singleline(v).desired_width(80.0));
                    });
                }
                ui.end_row();
            }
        });
    });
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Standard (deutsch) wiederherstellen").clicked() {
            w.keypad = default_keypad();
        }
    });
    let hint = if crate::keypad::native_supported() {
        "Die Tasten wirken nur auf dem Ziffernblock (NumLock an oder aus), die normalen Zifferntasten tippen weiter Zahlen. \
         Mehrere Befehle mit ; trennen; Aliase werden ausgewertet."
    } else {
        "Auf diesem System lässt sich der Ziffernblock nicht von den Zifferntasten unterscheiden: \
         Die Belegung wirkt, wenn die Eingabezeile leer ist (Ziffern 0–9 und +)."
    };
    ui.label(RichText::new(hint).weak());
}

/// Eigene Knöpfe der Icon-Leiste.
pub fn buttons(ui: &mut Ui, s: &mut Session, sel: &mut Option<usize>) {
    let b = &mut s.world.buttons;
    ui.horizontal(|ui| {
        if ui.button("➕ Neu").clicked() {
            b.push(UserButton::default());
            *sel = Some(b.len() - 1);
        }
        let has = sel.is_some_and(|i| i < b.len());
        if ui
            .add_enabled(has, egui::Button::new("⬅"))
            .on_hover_text("nach links")
            .clicked()
        {
            let i = sel.unwrap();
            if i > 0 {
                b.swap(i, i - 1);
                *sel = Some(i - 1);
            }
        }
        if ui
            .add_enabled(has, egui::Button::new("➡"))
            .on_hover_text("nach rechts")
            .clicked()
        {
            let i = sel.unwrap();
            if i + 1 < b.len() {
                b.swap(i, i + 1);
                *sel = Some(i + 1);
            }
        }
        if ui.add_enabled(has, egui::Button::new("🗑 Löschen")).clicked() {
            let i = sel.unwrap();
            b.remove(i);
            *sel = if b.is_empty() { None } else { Some(i.min(b.len() - 1)) };
        }
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        for (i, btn) in b.iter().enumerate() {
            if ui.selectable_label(*sel == Some(i), &btn.label).clicked() {
                *sel = Some(i);
            }
        }
    });
    ui.separator();
    if let Some(btn) = sel.and_then(|i| b.get_mut(i)) {
        Grid::new("btn").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Beschriftung");
            ui.text_edit_singleline(&mut btn.label);
            ui.end_row();
            ui.label("Tooltip");
            ui.text_edit_singleline(&mut btn.tooltip);
            ui.end_row();
            ui.label("Befehl");
            ui.add(
                TextEdit::multiline(&mut btn.send)
                    .code_editor()
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            ui.end_row();
            ui.label("Senden an");
            send_to_combo(ui, "btn_send_to", &mut btn.send_to);
            ui.end_row();
        });
        ui.label(RichText::new("Beschriftung darf auch ein Symbol sein, z. B. ⚔ 🛡 🍺 ❤ ✨ ☠ 🏠").weak());
    } else {
        ui.label(RichText::new("Eigene Knöpfe erscheinen rechts in der Icon-Leiste. Mit „Neu“ anlegen.").weak());
    }
}

/// Zustand des Plugin-Managers.
#[derive(Default)]
pub struct PluginUi {
    cache: Option<(Vec<crate::plugins::PluginMeta>, Vec<String>)>,
    new_name: String,
    confirm_delete: Option<String>,
    message: String,
}

/// Plugin-Manager: installierte Plugins anzeigen, pro Welt aktivieren, installieren, anlegen, löschen.
pub fn plugins(ui: &mut Ui, s: &mut Session, st: &mut PluginUi) {
    use crate::plugins as pl;
    if st.cache.is_none() {
        st.cache = Some(pl::list_plugins());
    }
    let mut refresh = false;

    ui.horizontal_wrapped(|ui| {
        if ui
            .button("📥 Ordner installieren …")
            .on_hover_text("Plugin-Ordner mit plugin.toml")
            .clicked()
        {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                st.message = match pl::install_from_dir(&dir) {
                    Ok(m) => format!("„{}“ installiert.", m.name),
                    Err(e) => format!("Fehler: {e}"),
                };
                refresh = true;
            }
        }
        if ui.button("📥 Lua-Datei installieren …").clicked() {
            if let Some(file) = rfd::FileDialog::new().add_filter("Lua", &["lua"]).pick_file() {
                st.message = match pl::install_from_lua(&file) {
                    Ok(m) => format!("„{}“ installiert.", m.name),
                    Err(e) => format!("Fehler: {e}"),
                };
                refresh = true;
            }
        }
        if ui.button("📂 Plugin-Ordner").clicked() {
            let _ = std::fs::create_dir_all(pl::plugins_dir());
            crate::updater::open_url(&pl::plugins_dir().display().to_string());
        }
        if ui
            .button("🔄 Neu laden")
            .on_hover_text("Liste neu einlesen und aktive Plugins neu laden")
            .clicked()
        {
            refresh = true;
            s.reload_plugins();
        }
    });
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(&mut st.new_name)
                .hint_text("Name für neues Plugin")
                .desired_width(200.0),
        );
        if ui
            .add_enabled(
                !st.new_name.trim().is_empty(),
                egui::Button::new("➕ Neues Plugin anlegen"),
            )
            .clicked()
        {
            st.message = match pl::create_template(&st.new_name, "") {
                Ok(m) => {
                    crate::updater::open_url(&m.main_path().display().to_string());
                    st.new_name.clear();
                    format!("„{}“ angelegt in {}", m.name, m.dir.display())
                }
                Err(e) => format!("Fehler: {e}"),
            };
            refresh = true;
        }
    });
    if !st.message.is_empty() {
        ui.label(RichText::new(&st.message).weak());
    }
    ui.separator();

    let (list, errors) = st.cache.clone().unwrap_or_default();
    if list.is_empty() {
        ui.label("Noch keine Plugins installiert.");
    }
    for m in &list {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let mut on = s.plugin_enabled(&m.id);
                if ui.checkbox(&mut on, "").on_hover_text("In dieser Welt aktiv").changed() {
                    s.set_plugin_enabled(m, on);
                }
                ui.label(RichText::new(&m.name).strong());
                ui.label(RichText::new(format!("v{}", m.version)).weak());
                if !m.author.is_empty() {
                    ui.label(RichText::new(format!("von {}", m.author)).weak());
                }
                if pl::is_bundled(&m.id) {
                    ui.label(RichText::new("mitgeliefert").small().color(crate::app::ACCENT));
                }
            });
            if !m.description.is_empty() {
                ui.label(&m.description);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(&m.id).monospace().weak());
                if ui.small_button("📂").on_hover_text("Ordner öffnen").clicked() {
                    crate::updater::open_url(&m.dir.display().to_string());
                }
                if ui
                    .small_button("✏")
                    .on_hover_text("Lua-Datei im Editor öffnen")
                    .clicked()
                {
                    crate::updater::open_url(&m.main_path().display().to_string());
                }
                if st.confirm_delete.as_deref() == Some(m.id.as_str()) {
                    if ui
                        .small_button(RichText::new("Wirklich löschen?").color(egui::Color32::LIGHT_RED))
                        .clicked()
                    {
                        s.set_plugin_enabled(m, false);
                        st.message = match pl::delete(m) {
                            Ok(()) => format!("„{}“ gelöscht.", m.name),
                            Err(e) => format!("Fehler: {e}"),
                        };
                        st.confirm_delete = None;
                        refresh = true;
                    }
                    if ui.small_button("Nein").clicked() {
                        st.confirm_delete = None;
                    }
                } else if ui.small_button("🗑").on_hover_text("Plugin löschen").clicked() {
                    st.confirm_delete = Some(m.id.clone());
                }
            });
        });
    }
    let removed: Vec<&str> = pl::BUNDLED
        .iter()
        .map(|(id, _, _)| *id)
        .filter(|id| !list.iter().any(|m| m.id == *id))
        .collect();
    if !removed.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Gelöschte mitgelieferte Plugins:").weak());
            for id in removed {
                if ui.small_button(format!("↺ {id} wiederherstellen")).clicked() {
                    st.message = match pl::restore_bundled(id) {
                        Ok(()) => format!("„{id}“ wiederhergestellt."),
                        Err(e) => format!("Fehler: {e}"),
                    };
                    refresh = true;
                }
            }
        });
    }
    for e in &errors {
        ui.label(RichText::new(format!("Defektes Plugin: {e}")).color(egui::Color32::LIGHT_RED));
    }
    ui.separator();
    ui.collapsing("Wie schreibe ich ein Plugin?", |ui| {
        ui.label(
            "Ein Plugin ist ein Ordner mit plugin.toml (Name, Version, Autor, Beschreibung) und main.lua. \
             Es läuft in einem eigenen Lua-Zustand und kann die ganze Lua-API benutzen. Callbacks:",
        );
        for (f, d) in [
            ("OnPluginInstall()", "beim Laden"),
            ("OnPluginConnect() / OnPluginDisconnect()", "Verbindung"),
            (
                "OnPluginLineReceived(zeile, ansi)",
                "jede Zeile; false = ausblenden, Text = ersetzen",
            ),
            ("OnPluginCommand(befehl)", "jeder Befehl; false = nicht senden"),
            ("OnPluginGMCP(paket, json) / OnPluginMSDP(var)", "Protokolldaten"),
            ("OnPluginDisable()", "beim Deaktivieren"),
            ("OnPluginTick()", "etwa zehnmal pro Sekunde"),
        ] {
            ui.horizontal(|ui| {
                ui.label(RichText::new(f).monospace());
                ui.label(RichText::new(d).weak());
            });
        }
        ui.label("Zusätzlich: GetPluginID(), GetPluginName(), GetPluginDirectory().");
        ui.label(
            "Eigene Fenster: WindowCreate(id, titel), WindowClear, WindowAddLine (mit ANSI-Farben), \
             WindowHeading, WindowGauge(id, name, wert, max, farbe), WindowPair, WindowSeparator, \
             WindowShow, WindowOption(id, \"fit\", true). Zeilen umschreiben: OnPluginLineReceived \
             darf einen neuen Text zurückgeben; AnsiSub/AnsiStrip/AnsiLen helfen dabei.",
        );
    });
    if refresh {
        st.cache = None;
    }
}
