//! Dialoge zum Bearbeiten von Triggern, Aliassen, Timern, Variablen und Welteinstellungen.

use crate::automation::compile;
use crate::session::Session;
use crate::world::{Alias, Encoding, SendTo, Timer, Trigger};
use egui::{ComboBox, DragValue, Grid, RichText, ScrollArea, TextEdit, Ui};

#[derive(Default)]
pub struct EditorState {
    pub selected: Option<usize>,
    pub test_line: String,
    pub filter: String,
}

fn send_to_combo(ui: &mut Ui, id: &str, v: &mut SendTo) -> bool {
    let mut changed = false;
    ComboBox::from_id_salt(id)
        .selected_text(v.label())
        .show_ui(ui, |ui| {
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
        if ui
            .add_enabled(has_sel, egui::Button::new("⧉ Kopieren"))
            .clicked()
        {
            let i = st.selected.unwrap();
            let c = items[i].clone();
            items.insert(i + 1, c);
            st.selected = Some(i + 1);
            changed = true;
        }
        if ui
            .add_enabled(has_sel, egui::Button::new("🗑 Löschen"))
            .clicked()
        {
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
            ui.label(
                RichText::new(format!("Fehler im Muster: {e}")).color(egui::Color32::LIGHT_RED),
            );
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
                            .map(|i| {
                                format!("%{i} = „{}“", c.get(i).map(|m| m.as_str()).unwrap_or(""))
                            })
                            .collect();
                        ui.label(
                            RichText::new(format!("✔ Treffer  {}", wc.join(", ")))
                                .color(egui::Color32::LIGHT_GREEN),
                        );
                    }
                    None => {
                        ui.label(RichText::new("✘ kein Treffer").color(egui::Color32::LIGHT_RED));
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
        ScrollArea::vertical().id_salt("detail").show(ui, |ui| {
            Grid::new("trg")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
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
                        .add(
                            TextEdit::singleline(&mut t.script)
                                .hint_text("f(name, zeile, wildcards)"),
                        )
                        .changed();
                    ui.end_row();
                    ui.label("Farbe");
                    changed |= ui
                        .add(
                            TextEdit::singleline(&mut t.color).hint_text("z. B. gelb oder #ff8800"),
                        )
                        .changed();
                    ui.end_row();
                    ui.label("Reihenfolge");
                    changed |= ui
                        .add(DragValue::new(&mut t.sequence).range(0..=10000))
                        .changed();
                    ui.end_row();
                    ui.label("Optionen");
                    ui.vertical(|ui| {
                        changed |= ui
                            .checkbox(&mut t.omit_from_output, "Zeile nicht anzeigen")
                            .changed();
                        changed |= ui
                            .checkbox(&mut t.omit_from_log, "Zeile nicht loggen")
                            .changed();
                        changed |= ui
                            .checkbox(&mut t.keep_evaluating, "Weitere Trigger auswerten")
                            .changed();
                        changed |= ui
                            .checkbox(&mut t.one_shot, "Nur einmal auslösen")
                            .changed();
                    });
                    ui.end_row();
                });
            pattern_check(ui, &t.pattern, t.regex, t.ignore_case, &mut st.test_line);
            ui.label(
                RichText::new(
                    "Im Sendetext: %0 = ganze Zeile, %1…%9 = Wildcards, @name = Variable, %% = %",
                )
                .weak(),
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
        ScrollArea::vertical().id_salt("detail").show(ui, |ui| {
            Grid::new("ali")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
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
                        .add(
                            TextEdit::singleline(&mut a.script)
                                .hint_text("f(name, zeile, wildcards)"),
                        )
                        .changed();
                    ui.end_row();
                    ui.label("Reihenfolge");
                    changed |= ui
                        .add(DragValue::new(&mut a.sequence).range(0..=10000))
                        .changed();
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
        Grid::new("tim")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Name");
                changed |= ui.text_edit_singleline(&mut t.name).changed();
                ui.end_row();
                ui.label("Gruppe");
                changed |= ui.text_edit_singleline(&mut t.group).changed();
                ui.end_row();
                ui.label("Intervall (s)");
                changed |= ui
                    .add(
                        DragValue::new(&mut t.interval)
                            .range(0.1..=86400.0)
                            .speed(0.5),
                    )
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
                    changed |= ui
                        .checkbox(&mut t.only_when_connected, "Nur wenn verbunden")
                        .changed();
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
        Grid::new("vars")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
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
            shared.variables.insert(
                st.new_name.trim().to_string(),
                std::mem::take(&mut st.new_value),
            );
            st.new_name.clear();
        }
    });
    ui.label(
        RichText::new("In Triggern/Aliassen als @name, in Lua per GetVariable(\"name\").").weak(),
    );
}

/// Liefert `true`, wenn „Skript neu laden“ gedrückt wurde.
pub fn world_settings(ui: &mut Ui, s: &mut Session) -> bool {
    let mut reload = false;
    let w = &mut s.world;
    let mut changed = false;
    ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Verbindung");
        Grid::new("conn")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
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
                            .selectable_value(
                                &mut w.encoding,
                                Encoding::Latin1,
                                "Latin-1 (ISO-8859-1)",
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(&mut w.encoding, Encoding::Utf8, "UTF-8")
                            .changed();
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
        Grid::new("input")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
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
        Grid::new("out")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Schriftgröße");
                ui.add(
                    DragValue::new(&mut w.font_size)
                        .range(8.0..=40.0)
                        .speed(0.5),
                );
                ui.end_row();
                ui.label("Max. Zeilen");
                ui.add(
                    DragValue::new(&mut w.max_lines)
                        .range(100..=1_000_000)
                        .speed(100),
                );
                ui.end_row();
            });
        ui.separator();
        ui.heading("Skript & Log");
        Grid::new("script")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
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
                    ui.checkbox(
                        &mut w.logging,
                        "Sitzung mitschreiben (ab nächster Verbindung)",
                    );
                    ui.checkbox(&mut w.log_input, "Eingaben mitloggen");
                });
                ui.end_row();
                ui.label("Log-Ordner");
                ui.add(
                    TextEdit::singleline(&mut w.log_dir)
                        .hint_text(crate::world::default_log_dir().display().to_string())
                        .desired_width(280.0),
                );
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
