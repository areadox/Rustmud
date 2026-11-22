//! Mitschnitt der Sitzung als Textdatei (eine Datei pro Welt und Tag).

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub struct Logger {
    file: BufWriter<File>,
    pub path: PathBuf,
}

impl Logger {
    pub fn open(dir: &Path, world: &str) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let slug: String = world
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let name = format!("{}_{}.log", slug, jiff::Zoned::now().strftime("%Y-%m-%d"));
        let path = dir.join(name);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut l = Self {
            file: BufWriter::new(file),
            path,
        };
        l.line(&format!(
            "----- Log gestartet {} -----",
            jiff::Zoned::now().strftime("%d.%m.%Y %H:%M:%S")
        ));
        Ok(l)
    }

    pub fn line(&mut self, text: &str) {
        let _ = writeln!(self.file, "{text}");
    }

    pub fn flush(&mut self) {
        let _ = self.file.flush();
    }
}

impl Drop for Logger {
    fn drop(&mut self) {
        self.line(&format!(
            "----- Log beendet {} -----",
            jiff::Zoned::now().strftime("%d.%m.%Y %H:%M:%S")
        ));
        self.flush();
    }
}
