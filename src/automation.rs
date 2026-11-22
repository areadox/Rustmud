//! Auswertung von Triggern, Aliassen, Timern und Speedwalks.

use crate::world::{Alias, Timer, Trigger};
use regex::{Regex, RegexBuilder};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Wandelt ein MUSHclient-Wildcard-Muster (`*`) in einen verankerten Regex um.
pub fn wildcard_to_regex(pattern: &str) -> String {
    let mut out = String::from("^");
    for part in pattern.split('*').enumerate() {
        if part.0 > 0 {
            out.push_str("(.*?)");
        }
        out.push_str(&regex::escape(part.1));
    }
    out.push('$');
    out
}

pub fn compile(pattern: &str, is_regex: bool, ignore_case: bool) -> Result<Regex, String> {
    let src = if is_regex {
        pattern.to_string()
    } else {
        wildcard_to_regex(pattern)
    };
    RegexBuilder::new(&src)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| e.to_string())
}

/// Ein Treffer mit allen Wildcards (`%0` = gesamter Treffer).
#[derive(Debug, Clone, Default)]
pub struct Match {
    pub wildcards: Vec<String>,
    pub named: BTreeMap<String, String>,
}

impl Match {
    fn from_caps(re: &Regex, caps: &regex::Captures) -> Self {
        let wildcards = (0..caps.len())
            .map(|i| {
                caps.get(i)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default()
            })
            .collect();
        let named = re
            .capture_names()
            .flatten()
            .filter_map(|n| {
                caps.name(n)
                    .map(|m| (n.to_string(), m.as_str().to_string()))
            })
            .collect();
        Self { wildcards, named }
    }
}

/// Ersetzt `%0`..`%9`, `%<name>`, `%%` sowie `@variable` / `@@`.
pub fn substitute(template: &str, m: &Match, vars: &BTreeMap<String, String>) -> String {
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::with_capacity(template.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '%' && i + 1 < chars.len() {
            let n = chars[i + 1];
            if let Some(d) = n.to_digit(10) {
                out.push_str(
                    m.wildcards
                        .get(d as usize)
                        .map(String::as_str)
                        .unwrap_or(""),
                );
                i += 2;
                continue;
            }
            if n == '%' {
                out.push('%');
                i += 2;
                continue;
            }
            if n == '<' {
                if let Some(end) = chars[i + 2..].iter().position(|&c| c == '>') {
                    let name: String = chars[i + 2..i + 2 + end].iter().collect();
                    out.push_str(m.named.get(&name).map(String::as_str).unwrap_or(""));
                    i += 3 + end;
                    continue;
                }
            }
        }
        if c == '@' && i + 1 < chars.len() {
            if chars[i + 1] == '@' {
                out.push('@');
                i += 2;
                continue;
            }
            let rest: String = chars[i + 1..]
                .iter()
                .take_while(|c| c.is_alphanumeric() || **c == '_')
                .collect();
            if !rest.is_empty() {
                if let Some(v) = vars.get(&rest) {
                    out.push_str(v);
                    i += 1 + rest.chars().count();
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Vorkompilierte Trigger/Aliase. Muss nach jeder Änderung neu gebaut werden.
#[derive(Default)]
pub struct Automation {
    triggers: Vec<(usize, Regex)>,
    aliases: Vec<(usize, Regex)>,
    pub errors: Vec<String>,
    timer_next: Vec<Option<Instant>>,
}

impl Automation {
    pub fn rebuild(&mut self, triggers: &[Trigger], aliases: &[Alias], timers: &[Timer]) {
        self.errors.clear();
        self.triggers.clear();
        self.aliases.clear();
        for (i, t) in triggers.iter().enumerate() {
            if t.pattern.is_empty() {
                continue;
            }
            match compile(&t.pattern, t.regex, t.ignore_case) {
                Ok(re) => self.triggers.push((i, re)),
                Err(e) => self.errors.push(format!(
                    "Trigger „{}“: {e}",
                    display_name(&t.name, &t.pattern)
                )),
            }
        }
        self.triggers
            .sort_by_key(|(i, _)| (triggers[*i].sequence, *i));
        for (i, a) in aliases.iter().enumerate() {
            if a.pattern.is_empty() {
                continue;
            }
            match compile(&a.pattern, a.regex, a.ignore_case) {
                Ok(re) => self.aliases.push((i, re)),
                Err(e) => self.errors.push(format!(
                    "Alias „{}“: {e}",
                    display_name(&a.name, &a.pattern)
                )),
            }
        }
        self.aliases
            .sort_by_key(|(i, _)| (aliases[*i].sequence, *i));
        self.reset_timers(timers);
    }

    pub fn reset_timers(&mut self, timers: &[Timer]) {
        let now = Instant::now();
        self.timer_next = timers
            .iter()
            .map(|t| {
                (t.enabled && t.interval > 0.0).then(|| now + Duration::from_secs_f64(t.interval))
            })
            .collect();
    }

    /// Liefert alle passenden Trigger (Index + Treffer) in Auswertungsreihenfolge.
    pub fn match_triggers(&self, triggers: &[Trigger], line: &str) -> Vec<(usize, Match)> {
        let mut out = Vec::new();
        for (i, re) in &self.triggers {
            let t = &triggers[*i];
            if !t.enabled {
                continue;
            }
            if let Some(caps) = re.captures(line) {
                out.push((*i, Match::from_caps(re, &caps)));
                if !t.keep_evaluating {
                    break;
                }
            }
        }
        out
    }

    pub fn match_aliases(&self, aliases: &[Alias], input: &str) -> Vec<(usize, Match)> {
        let mut out = Vec::new();
        for (i, re) in &self.aliases {
            let a = &aliases[*i];
            if !a.enabled {
                continue;
            }
            if let Some(caps) = re.captures(input) {
                out.push((*i, Match::from_caps(re, &caps)));
                if !a.keep_evaluating {
                    break;
                }
            }
        }
        out
    }

    /// Gibt die Indizes fälliger Timer zurück und plant sie neu ein.
    pub fn due_timers(&mut self, timers: &mut [Timer], connected: bool) -> Vec<usize> {
        let now = Instant::now();
        if self.timer_next.len() != timers.len() {
            self.reset_timers(timers);
        }
        let mut due = Vec::new();
        for (i, t) in timers.iter_mut().enumerate() {
            let Some(next) = self.timer_next[i] else {
                if t.enabled && t.interval > 0.0 {
                    self.timer_next[i] = Some(now + Duration::from_secs_f64(t.interval));
                }
                continue;
            };
            if !t.enabled {
                self.timer_next[i] = None;
                continue;
            }
            if now >= next {
                self.timer_next[i] = Some(now + Duration::from_secs_f64(t.interval.max(0.1)));
                if t.only_when_connected && !connected {
                    continue;
                }
                due.push(i);
                if t.one_shot {
                    t.enabled = false;
                    self.timer_next[i] = None;
                }
            }
        }
        due
    }
}

fn display_name<'a>(name: &'a str, pattern: &'a str) -> &'a str {
    if name.is_empty() {
        pattern
    } else {
        name
    }
}

/// Zerlegt einen Speedwalk wie `3n2o(nw)u` in einzelne Befehle.
/// Buchstaben werden unverändert gesendet, Klammern erlauben beliebige Befehle.
pub fn parse_speedwalk(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut i = 0;
    while i < chars.len() {
        let mut count = 0usize;
        while i < chars.len() && chars[i].is_ascii_digit() {
            count = count * 10 + chars[i].to_digit(10).unwrap() as usize;
            i += 1;
        }
        let count = count.clamp(1, 99);
        if i >= chars.len() {
            return Err("Speedwalk endet mit einer Zahl".into());
        }
        let cmd = if chars[i] == '(' {
            let end = chars[i..]
                .iter()
                .position(|&c| c == ')')
                .ok_or("fehlende „)“ im Speedwalk")?;
            let c: String = chars[i + 1..i + end].iter().collect();
            i += end + 1;
            c
        } else if chars[i].is_alphabetic() {
            let c = chars[i].to_string();
            i += 1;
            c
        } else {
            return Err(format!("unerwartetes Zeichen „{}“ im Speedwalk", chars[i]));
        };
        for _ in 0..count {
            out.push(cmd.clone());
        }
    }
    Ok(out)
}

/// Teilt eine Eingabe am Befehlstrenner; ein doppeltes Trennzeichen ergibt ein wörtliches.
pub fn split_command_stack(input: &str, sep: Option<char>) -> Vec<String> {
    let Some(sep) = sep else {
        return vec![input.to_string()];
    };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut it = input.chars().peekable();
    while let Some(c) = it.next() {
        if c == sep {
            if it.peek() == Some(&sep) {
                cur.push(sep);
                it.next();
            } else {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard() {
        let re = compile("* sagt: *", false, true).unwrap();
        let c = re.captures("Gandalf sagt: Hallo Welt").unwrap();
        assert_eq!(&c[1], "Gandalf");
        assert_eq!(&c[2], "Hallo Welt");
        assert!(re.captures("xx Gandalf sagt Hallo").is_none());
    }

    #[test]
    fn wildcard_escapes_regex_chars() {
        let re = compile("[LP: *]", false, false).unwrap();
        assert!(re.is_match("[LP: 120]"));
    }

    #[test]
    fn subst() {
        let m = Match {
            wildcards: vec!["all".into(), "Ork".into()],
            named: Default::default(),
        };
        let mut vars = BTreeMap::new();
        vars.insert("waffe".into(), "schwert".into());
        assert_eq!(
            substitute("toete %1 mit @waffe, 100%% @@x @unbekannt", &m, &vars),
            "toete Ork mit schwert, 100% @x @unbekannt"
        );
    }

    #[test]
    fn speedwalk() {
        assert_eq!(
            parse_speedwalk("3n2o(nw)").unwrap(),
            vec!["n", "n", "n", "o", "o", "nw"]
        );
        assert!(parse_speedwalk("3").is_err());
    }

    #[test]
    fn command_stack() {
        assert_eq!(split_command_stack("a;b;;c", Some(';')), vec!["a", "b;c"]);
        assert_eq!(split_command_stack("a;b", None), vec!["a;b"]);
    }

    #[test]
    fn trigger_order_and_keep_evaluating() {
        let triggers = vec![
            Trigger {
                pattern: "*".into(),
                sequence: 200,
                ..Default::default()
            },
            Trigger {
                pattern: "Hallo*".into(),
                sequence: 10,
                keep_evaluating: false,
                ..Default::default()
            },
        ];
        let mut a = Automation::default();
        a.rebuild(&triggers, &[], &[]);
        let m = a.match_triggers(&triggers, "Hallo du");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].0, 1);
    }
}
