//! Update-Prüfung über die GitHub-Releases von areadox/Rustmud.
//!
//! Unter Windows wird bei einer Installer-Installation das neue Setup
//! heruntergeladen und still ausgeführt; es startet rustmud danach neu.
//! Sonst wird die Release-Seite im Browser geöffnet.

use serde::Deserialize;
use std::sync::{Arc, Mutex};

pub const REPO: &str = "areadox/Rustmud";
pub const SETUP_ASSET: &str = "rustmud-setup-x64.exe";

#[derive(Debug, Clone, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub page_url: String,
    pub notes: String,
    pub setup_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(UpdateInfo),
    Downloading(String),
    /// Setup gestartet – Programm soll sich beenden
    Installing,
    Failed(String),
}

pub type Shared = Arc<Mutex<UpdateState>>;

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Vergleicht Versionen wie "0.0.1" und "v0.1.0". Liefert `true`, wenn `remote` neuer ist.
pub fn is_newer(remote: &str, local: &str) -> bool {
    fn parse(v: &str) -> Vec<u64> {
        v.trim()
            .trim_start_matches(['v', 'V'])
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    }
    let (mut r, mut l) = (parse(remote), parse(local));
    let n = r.len().max(l.len());
    r.resize(n, 0);
    l.resize(n, 0);
    r > l
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .try_proxy_from_env(true)
        .timeout(std::time::Duration::from_secs(60))
        .user_agent(&format!("rustmud/{}", current_version()))
        .build()
}

fn fetch_latest() -> Result<GhRelease, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => "Es gibt noch kein veröffentlichtes Release.".to_string(),
            other => other.to_string(),
        })?;
    resp.into_json::<GhRelease>().map_err(|e| e.to_string())
}

/// Prüft im Hintergrund auf Updates.
pub fn check(state: Shared, ctx: egui::Context) {
    *state.lock().unwrap() = UpdateState::Checking;
    std::thread::spawn(move || {
        let result = match fetch_latest() {
            Ok(rel) if is_newer(&rel.tag_name, current_version()) => UpdateState::Available(UpdateInfo {
                version: rel.tag_name.trim_start_matches('v').to_string(),
                page_url: rel.html_url,
                notes: rel.body,
                setup_url: rel
                    .assets
                    .into_iter()
                    .find(|a| a.name == SETUP_ASSET)
                    .map(|a| a.browser_download_url),
            }),
            Ok(_) => UpdateState::UpToDate,
            Err(e) => UpdateState::Failed(e),
        };
        *state.lock().unwrap() = result;
        ctx.request_repaint();
    });
}

/// Ordner, in den der Installer installiert (benutzerbezogen, ohne Adminrechte).
#[cfg(windows)]
fn install_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join("Programs").join("rustmud"))
}

/// Läuft das Programm aus der Installer-Installation?
pub fn installed_via_setup() -> bool {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(std::path::PathBuf::from));
        match (exe, install_dir()) {
            (Some(a), Some(b)) => {
                let norm =
                    |p: std::path::PathBuf| p.to_string_lossy().to_lowercase().trim_end_matches('\\').to_string();
                norm(a) == norm(b)
            }
            _ => false,
        }
    }
    #[cfg(not(windows))]
    false
}

/// Kann das Update automatisch installiert werden?
pub fn can_auto_install(info: &UpdateInfo) -> bool {
    info.setup_url.is_some() && installed_via_setup()
}

/// Lädt das Setup herunter und startet es still. Danach soll sich das Programm beenden.
pub fn download_and_install(state: Shared, info: UpdateInfo, ctx: egui::Context) {
    let Some(url) = info.setup_url.clone() else { return };
    *state.lock().unwrap() = UpdateState::Downloading(info.version.clone());
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let resp = agent().get(&url).call().map_err(|e| e.to_string())?;
            let mut data = Vec::new();
            std::io::Read::read_to_end(&mut resp.into_reader(), &mut data).map_err(|e| e.to_string())?;
            if data.len() < 100_000 {
                return Err("Download unvollständig".into());
            }
            let path = std::env::temp_dir().join(format!("rustmud-setup-{}.exe", info.version));
            std::fs::write(&path, &data).map_err(|e| format!("{}: {e}", path.display()))?;
            // /S = still installieren, /RUN = danach rustmud starten
            std::process::Command::new(&path)
                .args(["/S", "/RUN"])
                .spawn()
                .map_err(|e| format!("Setup konnte nicht gestartet werden: {e}"))?;
            Ok(())
        })();
        *state.lock().unwrap() = match result {
            Ok(()) => UpdateState::Installing,
            Err(e) => UpdateState::Failed(e),
        };
        ctx.request_repaint();
    });
}

/// Öffnet eine Adresse im Standardbrowser.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = r {
        eprintln!("Browser konnte nicht geöffnet werden: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("v0.0.2", "0.0.1"));
        assert!(is_newer("0.1.0", "0.0.9"));
        assert!(is_newer("v1.0", "0.9.9"));
        assert!(!is_newer("v0.0.1", "0.0.1"));
        assert!(!is_newer("0.0.1", "0.1.0"));
        assert!(!is_newer("v0.0.1-beta", "0.0.1"));
    }
}

#[cfg(test)]
mod net_tests {
    /// Echter Abruf bei GitHub – nur manuell: `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn fetch_latest_release() {
        match super::fetch_latest() {
            Ok(r) => println!("neueste Version: {} ({} Dateien)", r.tag_name, r.assets.len()),
            Err(e) => println!("Fehler/kein Release: {e}"),
        }
    }
}
