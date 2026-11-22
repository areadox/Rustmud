//! Telnet-Protokollschicht: Optionsverhandlung, MCCP2 (Kompression),
//! GMCP, MSDP, NAWS, TTYPE/MTTS sowie GA/EOR-Prompterkennung.

use flate2::{Decompress, FlushDecompress, Status};
use serde_json::{Map, Value};

pub const IAC: u8 = 255;
pub const DONT: u8 = 254;
pub const DO: u8 = 253;
pub const WONT: u8 = 252;
pub const WILL: u8 = 251;
pub const SB: u8 = 250;
pub const GA: u8 = 249;
pub const SE: u8 = 240;
pub const EOR_CMD: u8 = 239;

pub const OPT_ECHO: u8 = 1;
pub const OPT_SGA: u8 = 3;
pub const OPT_TTYPE: u8 = 24;
pub const OPT_EOR: u8 = 25;
pub const OPT_NAWS: u8 = 31;
pub const OPT_MSDP: u8 = 69;
pub const OPT_MCCP2: u8 = 86;
pub const OPT_GMCP: u8 = 201;

const MSDP_VAR: u8 = 1;
const MSDP_VAL: u8 = 2;
const MSDP_TABLE_OPEN: u8 = 3;
const MSDP_TABLE_CLOSE: u8 = 4;
const MSDP_ARRAY_OPEN: u8 = 5;
const MSDP_ARRAY_CLOSE: u8 = 6;

/// Ergebnis der Verarbeitung eingehender Bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum TelnetEvent {
    /// Normale Nutzdaten (noch nicht dekodiert, Zeichensatz kommt später).
    Data(Vec<u8>),
    /// GA oder EOR: der bisherige Text ist ein Prompt.
    Prompt,
    /// Server möchte, dass wir die Eingabe verbergen (Passwort) – `true` = verbergen.
    HideInput(bool),
    Gmcp {
        package: String,
        data: Value,
    },
    Msdp(Vec<(String, Value)>),
    CompressionStarted,
    CompressionEnded,
    /// Server hat NAWS akzeptiert – Client sollte jetzt die Fenstergröße senden.
    NawsEnabled,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct TelnetConfig {
    pub mccp: bool,
    pub gmcp: bool,
    pub msdp: bool,
    pub utf8: bool,
    pub client_name: String,
    pub client_version: String,
    pub gmcp_supports: Vec<String>,
}

impl Default for TelnetConfig {
    fn default() -> Self {
        Self {
            mccp: true,
            gmcp: true,
            msdp: true,
            utf8: false,
            client_name: "RUSTMUD".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
            gmcp_supports: vec![
                "Core 1".into(),
                "Char 1".into(),
                "Room 1".into(),
                "Comm 1".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Data,
    Iac,
    Will,
    Wont,
    Do,
    Dont,
    Sb,
    SbData,
    SbIac,
}

pub struct TelnetParser {
    cfg: TelnetConfig,
    state: State,
    sb_opt: u8,
    sb_buf: Vec<u8>,
    data_buf: Vec<u8>,
    zlib: Option<Decompress>,
    /// Optionen, die der Server bei sich aktiviert hat (WILL bestätigt).
    remote: [bool; 256],
    /// Optionen, die wir bei uns aktiviert haben (DO bestätigt).
    local: [bool; 256],
    ttype_cycle: u8,
}

impl TelnetParser {
    pub fn new(cfg: TelnetConfig) -> Self {
        Self {
            cfg,
            state: State::Data,
            sb_opt: 0,
            sb_buf: Vec::new(),
            data_buf: Vec::new(),
            zlib: None,
            remote: [false; 256],
            local: [false; 256],
            ttype_cycle: 0,
        }
    }

    #[cfg(test)]
    pub fn compressing(&self) -> bool {
        self.zlib.is_some()
    }

    /// Verarbeitet rohe Socket-Bytes. `reply` erhält Bytes, die direkt an den
    /// Server zurückgeschickt werden müssen (Verhandlungsantworten).
    pub fn feed(&mut self, input: &[u8], events: &mut Vec<TelnetEvent>, reply: &mut Vec<u8>) {
        let mut pending: Vec<u8> = input.to_vec();
        while !pending.is_empty() {
            if self.zlib.is_some() {
                match self.decompress(&pending) {
                    Ok((plain, rest_after_end)) => {
                        // Dekomprimierte Daten können selbst kein neues MCCP starten.
                        let _ = self.parse(&plain, events, reply);
                        match rest_after_end {
                            Some(rest) => {
                                self.zlib = None;
                                events.push(TelnetEvent::CompressionEnded);
                                pending = rest;
                            }
                            None => pending.clear(),
                        }
                    }
                    Err(e) => {
                        self.zlib = None;
                        events.push(TelnetEvent::Error(format!("MCCP-Fehler: {e}")));
                        events.push(TelnetEvent::CompressionEnded);
                        pending.clear();
                    }
                }
            } else {
                match self.parse(&pending, events, reply) {
                    Some(idx) => {
                        self.zlib = Some(Decompress::new(true));
                        events.push(TelnetEvent::CompressionStarted);
                        pending = pending[idx..].to_vec();
                    }
                    None => pending.clear(),
                }
            }
        }
        self.flush_data(events);
    }

    /// Dekomprimiert. Gibt (Klartext, Restbytes nach Stream-Ende) zurück.
    fn decompress(&mut self, input: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
        let z = self.zlib.as_mut().expect("zlib aktiv");
        let mut out = Vec::with_capacity(input.len() * 4);
        let mut buf = [0u8; 16 * 1024];
        let mut offset = 0;
        loop {
            let before_in = z.total_in();
            let before_out = z.total_out();
            let status = z
                .decompress(&input[offset..], &mut buf, FlushDecompress::None)
                .map_err(|e| e.to_string())?;
            let consumed = (z.total_in() - before_in) as usize;
            let produced = (z.total_out() - before_out) as usize;
            offset += consumed;
            out.extend_from_slice(&buf[..produced]);
            match status {
                Status::StreamEnd => {
                    return Ok((out, Some(input[offset..].to_vec())));
                }
                _ => {
                    if offset >= input.len() && produced < buf.len() {
                        return Ok((out, None));
                    }
                    if consumed == 0 && produced == 0 {
                        return Ok((out, None));
                    }
                }
            }
        }
    }

    fn flush_data(&mut self, events: &mut Vec<TelnetEvent>) {
        if !self.data_buf.is_empty() {
            events.push(TelnetEvent::Data(std::mem::take(&mut self.data_buf)));
        }
    }

    /// Parst Klartext-Telnet. Liefert `Some(index)`, wenn ab `index` der
    /// MCCP-komprimierte Datenstrom beginnt.
    fn parse(
        &mut self,
        input: &[u8],
        events: &mut Vec<TelnetEvent>,
        reply: &mut Vec<u8>,
    ) -> Option<usize> {
        for (i, &b) in input.iter().enumerate() {
            match self.state {
                State::Data => {
                    if b == IAC {
                        self.state = State::Iac;
                    } else {
                        self.data_buf.push(b);
                    }
                }
                State::Iac => {
                    self.state = State::Data;
                    match b {
                        IAC => self.data_buf.push(IAC),
                        WILL => self.state = State::Will,
                        WONT => self.state = State::Wont,
                        DO => self.state = State::Do,
                        DONT => self.state = State::Dont,
                        SB => self.state = State::Sb,
                        GA | EOR_CMD => {
                            self.flush_data(events);
                            events.push(TelnetEvent::Prompt);
                        }
                        _ => {}
                    }
                }
                State::Will => {
                    self.state = State::Data;
                    self.flush_data(events);
                    self.on_will(b, events, reply);
                }
                State::Wont => {
                    self.state = State::Data;
                    self.flush_data(events);
                    self.on_wont(b, events, reply);
                }
                State::Do => {
                    self.state = State::Data;
                    self.flush_data(events);
                    self.on_do(b, events, reply);
                }
                State::Dont => {
                    self.state = State::Data;
                    self.on_dont(b, reply);
                }
                State::Sb => {
                    self.sb_opt = b;
                    self.sb_buf.clear();
                    self.state = State::SbData;
                }
                State::SbData => {
                    if b == IAC {
                        self.state = State::SbIac;
                    } else {
                        self.sb_buf.push(b);
                    }
                }
                State::SbIac => match b {
                    IAC => {
                        self.sb_buf.push(IAC);
                        self.state = State::SbData;
                    }
                    SE => {
                        self.state = State::Data;
                        self.flush_data(events);
                        if self.sb_opt == OPT_MCCP2 && self.cfg.mccp {
                            return Some(i + 1);
                        }
                        self.on_subneg(events, reply);
                    }
                    _ => {
                        // Fehlerhafte Subverhandlung – abbrechen.
                        self.state = State::Data;
                    }
                },
            }
        }
        None
    }

    fn wants_remote(&self, opt: u8) -> bool {
        match opt {
            OPT_ECHO | OPT_SGA | OPT_EOR => true,
            OPT_MCCP2 => self.cfg.mccp,
            OPT_GMCP => self.cfg.gmcp,
            OPT_MSDP => self.cfg.msdp,
            _ => false,
        }
    }

    fn on_will(&mut self, opt: u8, events: &mut Vec<TelnetEvent>, reply: &mut Vec<u8>) {
        let o = opt as usize;
        if self.wants_remote(opt) {
            if !self.remote[o] {
                self.remote[o] = true;
                reply.extend_from_slice(&[IAC, DO, opt]);
                match opt {
                    OPT_ECHO => events.push(TelnetEvent::HideInput(true)),
                    OPT_GMCP => self.send_gmcp_hello(reply),
                    _ => {}
                }
            }
        } else {
            reply.extend_from_slice(&[IAC, DONT, opt]);
        }
    }

    fn on_wont(&mut self, opt: u8, events: &mut Vec<TelnetEvent>, reply: &mut Vec<u8>) {
        let o = opt as usize;
        if self.remote[o] {
            self.remote[o] = false;
            reply.extend_from_slice(&[IAC, DONT, opt]);
        }
        if opt == OPT_ECHO {
            events.push(TelnetEvent::HideInput(false));
        }
    }

    fn on_do(&mut self, opt: u8, events: &mut Vec<TelnetEvent>, reply: &mut Vec<u8>) {
        let o = opt as usize;
        match opt {
            OPT_TTYPE | OPT_NAWS => {
                if !self.local[o] {
                    self.local[o] = true;
                    reply.extend_from_slice(&[IAC, WILL, opt]);
                }
                if opt == OPT_NAWS {
                    events.push(TelnetEvent::NawsEnabled);
                }
            }
            _ => {
                reply.extend_from_slice(&[IAC, WONT, opt]);
            }
        }
    }

    fn on_dont(&mut self, opt: u8, reply: &mut Vec<u8>) {
        let o = opt as usize;
        if self.local[o] {
            self.local[o] = false;
            reply.extend_from_slice(&[IAC, WONT, opt]);
        }
    }

    fn on_subneg(&mut self, events: &mut Vec<TelnetEvent>, reply: &mut Vec<u8>) {
        let data = std::mem::take(&mut self.sb_buf);
        match self.sb_opt {
            OPT_TTYPE => {
                // IAC SB TTYPE SEND IAC SE
                if data.first() == Some(&1) {
                    let name = self.next_ttype();
                    reply.extend_from_slice(&[IAC, SB, OPT_TTYPE, 0]);
                    reply.extend_from_slice(name.as_bytes());
                    reply.extend_from_slice(&[IAC, SE]);
                }
            }
            OPT_GMCP => {
                let text = String::from_utf8_lossy(&data).to_string();
                let (package, rest) = match text.find([' ', '\n']) {
                    Some(p) => (text[..p].to_string(), text[p + 1..].trim().to_string()),
                    None => (text.clone(), String::new()),
                };
                let value = if rest.is_empty() {
                    Value::Null
                } else {
                    serde_json::from_str(&rest).unwrap_or(Value::String(rest))
                };
                events.push(TelnetEvent::Gmcp {
                    package,
                    data: value,
                });
            }
            OPT_MSDP => {
                let vars = parse_msdp(&data);
                if !vars.is_empty() {
                    events.push(TelnetEvent::Msdp(vars));
                }
            }
            _ => {}
        }
    }

    /// MTTS: erst Clientname, dann Terminaltyp, dann Fähigkeiten-Bitmaske.
    fn next_ttype(&mut self) -> String {
        let s = match self.ttype_cycle {
            0 => self.cfg.client_name.clone(),
            1 => "ANSI-256COLOR".to_string(),
            _ => {
                // 1 = ANSI, 4 = UTF-8, 8 = 256 Farben
                let mut bits = 1 | 8;
                if self.cfg.utf8 {
                    bits |= 4;
                }
                format!("MTTS {bits}")
            }
        };
        if self.ttype_cycle < 2 {
            self.ttype_cycle += 1;
        }
        s
    }

    fn send_gmcp_hello(&self, reply: &mut Vec<u8>) {
        let hello = serde_json::json!({
            "client": self.cfg.client_name,
            "version": self.cfg.client_version,
        });
        reply.extend(gmcp_packet("Core.Hello", Some(&hello.to_string())));
        let supports = Value::Array(
            self.cfg
                .gmcp_supports
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect(),
        );
        reply.extend(gmcp_packet(
            "Core.Supports.Set",
            Some(&supports.to_string()),
        ));
    }
}

/// Baut ein GMCP-Paket.
pub fn gmcp_packet(package: &str, json: Option<&str>) -> Vec<u8> {
    let mut v = vec![IAC, SB, OPT_GMCP];
    let mut body = package.to_string();
    if let Some(j) = json {
        body.push(' ');
        body.push_str(j);
    }
    for b in body.bytes() {
        v.push(b);
        if b == IAC {
            v.push(IAC);
        }
    }
    v.extend_from_slice(&[IAC, SE]);
    v
}

/// Baut ein MSDP-Paket „VAR name VAL wert…“.
pub fn msdp_packet(var: &str, vals: &[&str]) -> Vec<u8> {
    let mut v = vec![IAC, SB, OPT_MSDP, MSDP_VAR];
    v.extend_from_slice(var.as_bytes());
    for val in vals {
        v.push(MSDP_VAL);
        v.extend_from_slice(val.as_bytes());
    }
    v.extend_from_slice(&[IAC, SE]);
    v
}

/// Baut ein NAWS-Paket (Fenstergröße in Zeichen).
pub fn naws_packet(width: u16, height: u16) -> Vec<u8> {
    let mut v = vec![IAC, SB, OPT_NAWS];
    for b in width.to_be_bytes().into_iter().chain(height.to_be_bytes()) {
        v.push(b);
        if b == IAC {
            v.push(IAC);
        }
    }
    v.extend_from_slice(&[IAC, SE]);
    v
}

/// Verdoppelt IAC-Bytes in ausgehenden Nutzdaten.
pub fn escape_iac(data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(data.len() + 4);
    for &b in data {
        v.push(b);
        if b == IAC {
            v.push(IAC);
        }
    }
    v
}

// ---------------------------------------------------------------- MSDP

fn parse_msdp(data: &[u8]) -> Vec<(String, Value)> {
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < data.len() {
        if data[pos] != MSDP_VAR {
            pos += 1;
            continue;
        }
        pos += 1;
        let name = read_msdp_string(data, &mut pos);
        let value = read_msdp_values(data, &mut pos);
        out.push((name, value));
    }
    out
}

fn read_msdp_string(data: &[u8], pos: &mut usize) -> String {
    let start = *pos;
    while *pos < data.len() && data[*pos] > MSDP_ARRAY_CLOSE {
        *pos += 1;
    }
    String::from_utf8_lossy(&data[start..*pos]).to_string()
}

/// Liest ein oder mehrere VAL nach einem VAR. Mehrere VAL ergeben ein Array.
fn read_msdp_values(data: &[u8], pos: &mut usize) -> Value {
    let mut vals = Vec::new();
    while *pos < data.len() && data[*pos] == MSDP_VAL {
        *pos += 1;
        vals.push(read_msdp_value(data, pos));
    }
    match vals.len() {
        0 => Value::Null,
        1 => vals.pop().unwrap(),
        _ => Value::Array(vals),
    }
}

fn read_msdp_value(data: &[u8], pos: &mut usize) -> Value {
    if *pos >= data.len() {
        return Value::Null;
    }
    match data[*pos] {
        MSDP_TABLE_OPEN => {
            *pos += 1;
            let mut map = Map::new();
            while *pos < data.len() && data[*pos] != MSDP_TABLE_CLOSE {
                if data[*pos] == MSDP_VAR {
                    *pos += 1;
                    let k = read_msdp_string(data, pos);
                    let v = read_msdp_values(data, pos);
                    map.insert(k, v);
                } else {
                    *pos += 1;
                }
            }
            *pos += 1;
            Value::Object(map)
        }
        MSDP_ARRAY_OPEN => {
            *pos += 1;
            let mut arr = Vec::new();
            while *pos < data.len() && data[*pos] != MSDP_ARRAY_CLOSE {
                if data[*pos] == MSDP_VAL {
                    *pos += 1;
                    arr.push(read_msdp_value(data, pos));
                } else {
                    *pos += 1;
                }
            }
            *pos += 1;
            Value::Array(arr)
        }
        _ => Value::String(read_msdp_string(data, pos)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn run(p: &mut TelnetParser, input: &[u8]) -> (Vec<TelnetEvent>, Vec<u8>) {
        let mut ev = Vec::new();
        let mut rep = Vec::new();
        p.feed(input, &mut ev, &mut rep);
        (ev, rep)
    }

    fn data_of(ev: &[TelnetEvent]) -> Vec<u8> {
        ev.iter()
            .filter_map(|e| {
                if let TelnetEvent::Data(d) = e {
                    Some(d.clone())
                } else {
                    None
                }
            })
            .flatten()
            .collect()
    }

    #[test]
    fn plain_and_escaped_iac() {
        let mut p = TelnetParser::new(TelnetConfig::default());
        let (ev, _) = run(&mut p, &[b'a', IAC, IAC, b'b']);
        assert_eq!(data_of(&ev), vec![b'a', 255, b'b']);
    }

    #[test]
    fn prompt_on_ga() {
        let mut p = TelnetParser::new(TelnetConfig::default());
        let (ev, _) = run(&mut p, &[b'>', IAC, GA]);
        assert_eq!(ev, vec![TelnetEvent::Data(vec![b'>']), TelnetEvent::Prompt]);
    }

    #[test]
    fn negotiation_answers() {
        let mut p = TelnetParser::new(TelnetConfig::default());
        let (_, rep) = run(&mut p, &[IAC, WILL, 99, IAC, DO, OPT_NAWS]);
        assert_eq!(rep, vec![IAC, DONT, 99, IAC, WILL, OPT_NAWS]);
    }

    #[test]
    fn echo_hides_input() {
        let mut p = TelnetParser::new(TelnetConfig::default());
        let (ev, _) = run(&mut p, &[IAC, WILL, OPT_ECHO]);
        assert!(ev.contains(&TelnetEvent::HideInput(true)));
        let (ev, _) = run(&mut p, &[IAC, WONT, OPT_ECHO]);
        assert!(ev.contains(&TelnetEvent::HideInput(false)));
    }

    #[test]
    fn gmcp_parsing() {
        let mut p = TelnetParser::new(TelnetConfig::default());
        let mut input = vec![IAC, SB, OPT_GMCP];
        input.extend_from_slice(br#"Char.Vitals {"hp": 10, "maxhp": 20}"#);
        input.extend_from_slice(&[IAC, SE]);
        let (ev, _) = run(&mut p, &input);
        assert_eq!(
            ev,
            vec![TelnetEvent::Gmcp {
                package: "Char.Vitals".into(),
                data: serde_json::json!({"hp": 10, "maxhp": 20})
            }]
        );
    }

    #[test]
    fn msdp_parsing() {
        let mut p = TelnetParser::new(TelnetConfig::default());
        let mut input = vec![IAC, SB, OPT_MSDP, MSDP_VAR];
        input.extend_from_slice(b"HEALTH");
        input.push(MSDP_VAL);
        input.extend_from_slice(b"42");
        input.push(MSDP_VAR);
        input.extend_from_slice(b"ROOM");
        input.extend_from_slice(&[MSDP_VAL, MSDP_TABLE_OPEN, MSDP_VAR]);
        input.extend_from_slice(b"NAME");
        input.push(MSDP_VAL);
        input.extend_from_slice(b"Marktplatz");
        input.push(MSDP_TABLE_CLOSE);
        input.extend_from_slice(&[IAC, SE]);
        let (ev, _) = run(&mut p, &input);
        assert_eq!(
            ev,
            vec![TelnetEvent::Msdp(vec![
                ("HEALTH".into(), Value::String("42".into())),
                ("ROOM".into(), serde_json::json!({"NAME": "Marktplatz"})),
            ])]
        );
    }

    #[test]
    fn mccp_in_same_packet() {
        let mut p = TelnetParser::new(TelnetConfig::default());
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(b"Willkommen im MUD!\r\n").unwrap();
        let compressed = enc.finish().unwrap();
        let mut input = b"vorher ".to_vec();
        input.extend_from_slice(&[IAC, SB, OPT_MCCP2, IAC, SE]);
        input.extend_from_slice(&compressed);
        input.extend_from_slice(b"nachher");
        let (ev, _) = run(&mut p, &input);
        assert_eq!(
            data_of(&ev),
            b"vorher Willkommen im MUD!\r\nnachher".to_vec()
        );
        assert!(ev.contains(&TelnetEvent::CompressionStarted));
        assert!(ev.contains(&TelnetEvent::CompressionEnded));
    }

    #[test]
    fn mccp_split_across_reads() {
        let mut p = TelnetParser::new(TelnetConfig::default());
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(b"Zeile eins\r\nZeile zwei\r\n").unwrap();
        enc.flush().unwrap();
        let compressed = enc.get_ref().clone();
        run(&mut p, &[IAC, SB, OPT_MCCP2, IAC, SE]);
        let (half1, half2) = compressed.split_at(compressed.len() / 2);
        let (ev1, _) = run(&mut p, half1);
        let (ev2, _) = run(&mut p, half2);
        let mut all = data_of(&ev1);
        all.extend(data_of(&ev2));
        assert_eq!(all, b"Zeile eins\r\nZeile zwei\r\n".to_vec());
        assert!(p.compressing());
    }

    #[test]
    fn ttype_cycle() {
        let mut p = TelnetParser::new(TelnetConfig::default());
        let req = [IAC, SB, OPT_TTYPE, 1, IAC, SE];
        let (_, r1) = run(&mut p, &req);
        assert!(String::from_utf8_lossy(&r1).contains("RUSTMUD"));
        let (_, r2) = run(&mut p, &req);
        assert!(String::from_utf8_lossy(&r2).contains("ANSI-256COLOR"));
        let (_, r3) = run(&mut p, &req);
        assert!(String::from_utf8_lossy(&r3).contains("MTTS 9"));
    }
}
