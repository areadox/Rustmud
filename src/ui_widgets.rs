//! Darstellung von Plugin-Fenstern (Text, Überschriften, Balken, Wertepaare)
//! und gemeinsame Zeichenhilfen.

use crate::ansi::{resolve, Color, Line};
use crate::scripting::WinItem;
use crate::session::PluginWindow;
use egui::text::LayoutJob;
use egui::{Color32, FontId, RichText, Stroke, TextFormat};

pub const DEFAULT_FG: (u8, u8, u8) = (192, 192, 192);
pub const DEFAULT_BG: (u8, u8, u8) = (0, 0, 0);

/// Baut aus einer farbigen Zeile einen egui-LayoutJob.
pub fn line_job(line: &Line, font: &FontId, job: &mut LayoutJob) {
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

fn color32(c: Color) -> Color32 {
    let (r, g, b) = match c {
        Color::Default => (90, 160, 90),
        Color::Idx(i) => crate::ansi::palette(i),
        Color::Rgb(r, g, b) => (r, g, b),
    };
    Color32::from_rgb(r, g, b)
}

/// Fortschrittsbalken mit Beschriftung links und Wert in der Mitte.
fn gauge(ui: &mut egui::Ui, label: &str, value: f64, max: f64, color: Color) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [120.0, 20.0],
            egui::Label::new(RichText::new(label).strong()).truncate(),
        );
        let width = ui.available_width().max(60.0);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
        let frac = if max > 0.0 {
            (value / max).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let p = ui.painter();
        let radius = egui::CornerRadius::same(5);
        p.rect_filled(rect, radius, Color32::from_rgb(18, 19, 23));
        if frac > 0.0 {
            let mut fill = rect;
            fill.set_width(rect.width() * frac);
            let c = color32(color);
            p.rect_filled(fill, radius, c.linear_multiply(0.85));
            // leichter Glanz oben
            let mut shine = fill;
            shine.set_height(fill.height() * 0.45);
            p.rect_filled(shine, radius, Color32::from_white_alpha(18));
        }
        p.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, Color32::from_rgb(60, 63, 72)),
            egui::StrokeKind::Inside,
        );
        let text = if max > 0.0 {
            format!("{} / {}  ({:.0} %)", fmt_num(value), fmt_num(max), frac * 100.0)
        } else {
            fmt_num(value)
        };
        p.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(13.0),
            Color32::WHITE,
        );
        resp.on_hover_text(format!("{label}: {} von {}", fmt_num(value), fmt_num(max)));
    });
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.1}")
    }
}

/// Zeichnet den Inhalt eines Plugin-Fensters.
pub fn plugin_window(ui: &mut egui::Ui, w: &PluginWindow) {
    // Schriftgröße für Textzeilen: fest oder an Fenster angepasst
    let lines: Vec<&Line> = w
        .items
        .iter()
        .filter_map(|i| if let WinItem::Line(l) = i { Some(l) } else { None })
        .collect();
    let size = if w.fit && !lines.is_empty() {
        let cols = lines.iter().map(|l| l.char_len()).max().unwrap_or(1).max(1) as f32;
        let rows = lines.len().max(1) as f32;
        let probe = FontId::monospace(20.0);
        let (cw, rh) = ui.fonts(|f| (f.glyph_width(&probe, 'M') / 20.0, f.row_height(&probe) / 20.0));
        let avail = ui.available_size() - egui::vec2(28.0, 28.0);
        let by_w = avail.x / (cols * cw);
        let by_h = avail.y.max(120.0) / (rows * rh);
        by_w.min(by_h).clamp(6.0, 64.0)
    } else {
        w.font_size
    };
    let font = FontId::monospace(size);

    if w.items.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new("Noch keine Daten.").weak());
        return;
    }

    egui::Frame::new().inner_margin(8.0).show(ui, |ui| {
        let mut i = 0;
        let items = &w.items;
        while i < items.len() {
            match &items[i] {
                WinItem::Line(_) => {
                    // zusammenhängende Textzeilen in einem dunklen Block
                    let bg = Color32::from_rgb(DEFAULT_BG.0, DEFAULT_BG.1, DEFAULT_BG.2);
                    egui::Frame::new()
                        .fill(bg)
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            while let Some(WinItem::Line(l)) = items.get(i) {
                                let mut job = LayoutJob::default();
                                job.wrap.max_width = f32::INFINITY;
                                if l.spans.is_empty() {
                                    job.append(
                                        " ",
                                        0.0,
                                        TextFormat {
                                            font_id: font.clone(),
                                            ..Default::default()
                                        },
                                    );
                                } else {
                                    line_job(l, &font, &mut job);
                                }
                                ui.add(egui::Label::new(job).extend());
                                i += 1;
                            }
                        });
                    continue;
                }
                WinItem::Heading(t) => {
                    ui.add_space(2.0);
                    ui.label(RichText::new(t).size(16.0).strong().color(crate::app::ACCENT));
                    ui.add_space(2.0);
                }
                WinItem::Gauge {
                    label,
                    value,
                    max,
                    color,
                } => gauge(ui, label, *value, *max, *color),
                WinItem::Pair(..) => {
                    // zusammenhängende Wertepaare als Tabelle, bei genug Platz zwei Paare pro Zeile
                    let start = i;
                    while matches!(items.get(i), Some(WinItem::Pair(..))) {
                        i += 1;
                    }
                    let per_row = if ui.available_width() > 420.0 { 2 } else { 1 };
                    egui::Grid::new(("pairs", start))
                        .num_columns(per_row * 2)
                        .striped(true)
                        .spacing([14.0, 4.0])
                        .show(ui, |ui| {
                            for (n, item) in items[start..i].iter().enumerate() {
                                if let WinItem::Pair(k, v) = item {
                                    ui.label(RichText::new(k).weak());
                                    ui.label(RichText::new(v).strong());
                                }
                                if (n + 1) % per_row == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                    continue;
                }
                WinItem::Separator => {
                    ui.add_space(2.0);
                    ui.separator();
                }
            }
            i += 1;
        }
    });
}
