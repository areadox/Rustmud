//! ANSI-Escape-Parser: wandelt Text mit SGR-Sequenzen in farbige Abschnitte um.
//! Unterstützt 16 Farben, 256 Farben und Truecolor sowie Fett/Kursiv/Unterstrichen/Invers.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Color {
    Default,
    Idx(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// Eine Zeile der Ausgabe.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Line {
    pub spans: Vec<Span>,
    pub is_prompt: bool,
}

impl Line {
    pub fn plain(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    pub fn push(&mut self, ch: char, style: Style) {
        match self.spans.last_mut() {
            Some(last) if last.style == style => last.text.push(ch),
            _ => self.spans.push(Span {
                text: ch.to_string(),
                style,
            }),
        }
    }

    pub fn push_str(&mut self, s: &str, style: Style) {
        for c in s.chars() {
            self.push(c, style);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.spans.iter().all(|s| s.text.is_empty())
    }

    /// Teilt die Zeile nach `n` Zeichen; Farben bleiben erhalten.
    pub fn split_at_char(&self, n: usize) -> (Line, Line) {
        let mut left = Line::default();
        let mut right = Line {
            is_prompt: self.is_prompt,
            ..Line::default()
        };
        let mut i = 0;
        for span in &self.spans {
            for c in span.text.chars() {
                if i < n {
                    left.push(c, span.style);
                } else {
                    right.push(c, span.style);
                }
                i += 1;
            }
        }
        (left, right)
    }

    pub fn from_text(s: &str, style: Style) -> Self {
        let mut l = Line::default();
        l.push_str(s, style);
        l
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Text,
    Esc,
    Csi,
    /// OSC und andere Strings bis BEL/ST überspringen
    Osc,
}

pub enum AnsiEvent {
    Char(char, Style),
    Newline,
}

pub struct AnsiParser {
    state: State,
    params: String,
    pub style: Style,
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: State::Text,
            params: String::new(),
            style: Style::default(),
        }
    }

    pub fn feed(&mut self, text: &str, out: &mut Vec<AnsiEvent>) {
        for c in text.chars() {
            match self.state {
                State::Text => match c {
                    '\x1b' => self.state = State::Esc,
                    '\n' => out.push(AnsiEvent::Newline),
                    '\r' | '\x07' | '\0' => {}
                    '\t' => {
                        for _ in 0..4 {
                            out.push(AnsiEvent::Char(' ', self.style));
                        }
                    }
                    c if c.is_control() => {}
                    c => out.push(AnsiEvent::Char(c, self.style)),
                },
                State::Esc => match c {
                    '[' => {
                        self.params.clear();
                        self.state = State::Csi;
                    }
                    ']' => self.state = State::Osc,
                    _ => self.state = State::Text,
                },
                State::Csi => {
                    if c.is_ascii_digit() || c == ';' || c == ':' || c == '?' {
                        self.params.push(c);
                    } else {
                        if c == 'm' {
                            self.apply_sgr();
                        }
                        // Andere CSI-Befehle (Cursor etc.) ignorieren wir.
                        self.state = State::Text;
                    }
                }
                State::Osc => {
                    if c == '\x07' || c == '\\' {
                        self.state = State::Text;
                    }
                }
            }
        }
    }

    fn apply_sgr(&mut self) {
        let nums: Vec<u32> = if self.params.is_empty() {
            vec![0]
        } else {
            self.params.split([';', ':']).map(|p| p.parse().unwrap_or(0)).collect()
        };
        let mut i = 0;
        while i < nums.len() {
            let n = nums[i];
            match n {
                0 => self.style = Style::default(),
                1 => self.style.bold = true,
                3 => self.style.italic = true,
                4 => self.style.underline = true,
                7 => self.style.inverse = true,
                22 => self.style.bold = false,
                23 => self.style.italic = false,
                24 => self.style.underline = false,
                27 => self.style.inverse = false,
                30..=37 => self.style.fg = Color::Idx((n - 30) as u8),
                39 => self.style.fg = Color::Default,
                40..=47 => self.style.bg = Color::Idx((n - 40) as u8),
                49 => self.style.bg = Color::Default,
                90..=97 => self.style.fg = Color::Idx((n - 90 + 8) as u8),
                100..=107 => self.style.bg = Color::Idx((n - 100 + 8) as u8),
                38 | 48 => {
                    let color = match nums.get(i + 1) {
                        Some(5) => {
                            let c = nums.get(i + 2).copied().unwrap_or(0).min(255) as u8;
                            i += 2;
                            Some(Color::Idx(c))
                        }
                        Some(2) => {
                            let g = |k: usize| nums.get(i + k).copied().unwrap_or(0).min(255) as u8;
                            let c = Color::Rgb(g(2), g(3), g(4));
                            i += 4;
                            Some(c)
                        }
                        _ => None,
                    };
                    if let Some(c) = color {
                        if n == 38 {
                            self.style.fg = c;
                        } else {
                            self.style.bg = c;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

/// Standard-xterm-Palette für Index 0..=255.
pub fn palette(idx: u8) -> (u8, u8, u8) {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    match idx {
        0..=15 => BASE[idx as usize],
        16..=231 => {
            let i = idx - 16;
            let conv = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (conv(i / 36), conv((i / 6) % 6), conv(i % 6))
        }
        _ => {
            let g = 8 + (idx - 232) * 10;
            (g, g, g)
        }
    }
}

/// Löst Farben eines Stils zu RGB auf (inkl. „fett = hell“ wie bei MUSHclient).
pub type Rgb = (u8, u8, u8);

pub fn resolve(style: &Style, default_fg: Rgb, default_bg: Rgb) -> (Rgb, Option<Rgb>) {
    let fg = match style.fg {
        Color::Default => {
            if style.bold {
                (255, 255, 255)
            } else {
                default_fg
            }
        }
        Color::Idx(i) if i < 8 && style.bold => palette(i + 8),
        Color::Idx(i) => palette(i),
        Color::Rgb(r, g, b) => (r, g, b),
    };
    let bg = match style.bg {
        Color::Default => None,
        Color::Idx(i) => Some(palette(i)),
        Color::Rgb(r, g, b) => Some((r, g, b)),
    };
    if style.inverse {
        (bg.unwrap_or(default_bg), Some(fg))
    } else {
        (fg, bg)
    }
}

/// Wandelt Farbnamen (MUSHclient-kompatibel, z. B. "red", "#ff8800") in eine Farbe um.
pub fn color_from_name(name: &str) -> Option<Color> {
    let n = name.trim().to_ascii_lowercase();
    if let Some(hex) = n.strip_prefix('#') {
        if hex.len() == 6 {
            let v = u32::from_str_radix(hex, 16).ok()?;
            return Some(Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8));
        }
        return None;
    }
    let rgb = match n.as_str() {
        "" | "default" => return Some(Color::Default),
        "black" | "schwarz" => (0, 0, 0),
        "maroon" | "darkred" => (128, 0, 0),
        "green" | "grün" | "gruen" => (0, 128, 0),
        "olive" => (128, 128, 0),
        "navy" => (0, 0, 128),
        "purple" | "lila" => (128, 0, 128),
        "teal" => (0, 128, 128),
        "silver" => (192, 192, 192),
        "gray" | "grey" | "grau" => (128, 128, 128),
        "red" | "rot" => (255, 0, 0),
        "lime" => (0, 255, 0),
        "yellow" | "gelb" => (255, 255, 0),
        "blue" | "blau" => (0, 0, 255),
        "fuchsia" | "magenta" => (255, 0, 255),
        "aqua" | "cyan" => (0, 255, 255),
        "white" | "weiß" | "weiss" => (255, 255, 255),
        "orange" => (255, 165, 0),
        "pink" => (255, 192, 203),
        "brown" | "braun" => (165, 42, 42),
        "gold" => (255, 215, 0),
        "lightblue" | "hellblau" => (173, 216, 230),
        "lightgreen" | "hellgrün" => (144, 238, 144),
        "darkgray" | "dunkelgrau" => (64, 64, 64),
        _ => return None,
    };
    Some(Color::Rgb(rgb.0, rgb.1, rgb.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(p: &mut AnsiParser, s: &str) -> Vec<Line> {
        let mut ev = Vec::new();
        p.feed(s, &mut ev);
        let mut lines = vec![Line::default()];
        for e in ev {
            match e {
                AnsiEvent::Char(c, st) => lines.last_mut().unwrap().push(c, st),
                AnsiEvent::Newline => lines.push(Line::default()),
            }
        }
        lines
    }

    #[test]
    fn basic_colors() {
        let mut p = AnsiParser::new();
        let l = collect(&mut p, "\x1b[1;31mRot\x1b[0m normal");
        assert_eq!(l[0].spans.len(), 2);
        assert_eq!(l[0].spans[0].style.fg, Color::Idx(1));
        assert!(l[0].spans[0].style.bold);
        assert_eq!(l[0].spans[1].style, Style::default());
        assert_eq!(l[0].plain(), "Rot normal");
    }

    #[test]
    fn split_escape() {
        let mut p = AnsiParser::new();
        let _ = collect(&mut p, "a\x1b[3");
        let l = collect(&mut p, "2mgrün");
        assert_eq!(l[0].spans[0].style.fg, Color::Idx(2));
    }

    #[test]
    fn colors_256_and_rgb() {
        let mut p = AnsiParser::new();
        let l = collect(&mut p, "\x1b[38;5;208mx\x1b[48;2;1;2;3my");
        assert_eq!(l[0].spans[0].style.fg, Color::Idx(208));
        assert_eq!(l[0].spans[1].style.bg, Color::Rgb(1, 2, 3));
    }

    #[test]
    fn split_keeps_styles() {
        let mut p = AnsiParser::new();
        let l = collect(&mut p, "|\x1b[31m*\x1b[0m| Text");
        let (a, b) = l[0].split_at_char(3);
        assert_eq!(a.plain(), "|*|");
        assert_eq!(a.spans[1].style.fg, Color::Idx(1));
        assert_eq!(b.plain(), " Text");
    }

    #[test]
    fn crlf() {
        let mut p = AnsiParser::new();
        let l = collect(&mut p, "eins\r\nzwei");
        assert_eq!(l.len(), 2);
        assert_eq!(l[1].plain(), "zwei");
    }
}
