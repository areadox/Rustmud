//! Ziffernblock zum Laufen.
//!
//! egui unterscheidet die Zifferntasten des Ziffernblocks nicht von denen der
//! Buchstabentastatur. Unter Windows hängen wir uns deshalb mit
//! `SetWindowSubclass` in die Fensternachrichten und fangen die echten
//! Ziffernblock-Tasten (VK_NUMPAD0 … VK_DIVIDE, bei ausgeschaltetem NumLock
//! auch die nicht-erweiterten Pfeil-/Pos1-/Ende-Tasten) ab, bevor egui sie sieht.
//! Auf anderen Systemen gibt es einen Ersatz: Ziffern lösen nur dann eine
//! Richtung aus, wenn die Eingabezeile leer ist.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static ENABLED: AtomicBool = AtomicBool::new(true);
/// Nur aktiv, wenn die Eingabezeile (oder nichts) den Fokus hat – nicht beim Tippen in Dialogen.
static INPUT_FOCUSED: AtomicBool = AtomicBool::new(true);
static QUEUE: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

pub fn set_active(enabled: bool, input_focused: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    INPUT_FOCUSED.store(input_focused, Ordering::Relaxed);
}

fn active() -> bool {
    ENABLED.load(Ordering::Relaxed) && INPUT_FOCUSED.load(Ordering::Relaxed)
}

/// Liefert alle seit dem letzten Aufruf gedrückten Ziffernblock-Tasten ("8", "+", …).
pub fn take() -> Vec<&'static str> {
    std::mem::take(&mut *QUEUE.lock().unwrap())
}

/// `true`, wenn der Ziffernblock exakt erkannt wird (nur Windows).
pub fn native_supported() -> bool {
    cfg!(windows)
}

/// Ersatzlösung ohne native Erkennung: Ziffern bei leerer Eingabezeile abfangen.
pub fn fallback_filter(raw: &mut egui::RawInput, input_empty: bool) {
    if native_supported() || !active() || !input_empty {
        return;
    }
    let mut swallow_text: Vec<String> = Vec::new();
    let mut queue = QUEUE.lock().unwrap();
    raw.events.retain(|e| {
        if let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = e
        {
            if modifiers.any() {
                return true;
            }
            if let Some(k) = key_name(*key) {
                queue.push(k);
                swallow_text.push(k.to_string());
                return false;
            }
        }
        true
    });
    if !swallow_text.is_empty() {
        raw.events
            .retain(|e| !matches!(e, egui::Event::Text(t) if swallow_text.contains(t)));
        raw.events
            .retain(|e| !matches!(e, egui::Event::Key { key, pressed: false, .. } if key_name(*key).is_some()));
    }
}

fn key_name(key: egui::Key) -> Option<&'static str> {
    use egui::Key::*;
    Some(match key {
        Num0 => "0",
        Num1 => "1",
        Num2 => "2",
        Num3 => "3",
        Num4 => "4",
        Num5 => "5",
        Num6 => "6",
        Num7 => "7",
        Num8 => "8",
        Num9 => "9",
        Plus => "+",
        _ => return None,
    })
}

#[cfg(windows)]
pub use win::install;

#[cfg(not(windows))]
pub fn install(_cc: &eframe::CreationContext<'_>) -> bool {
    false
}

#[cfg(windows)]
mod win {
    use super::{active, QUEUE};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, VK_ADD, VK_CLEAR, VK_CONTROL, VK_DECIMAL, VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_HOME,
        VK_INSERT, VK_LEFT, VK_MENU, VK_MULTIPLY, VK_NEXT, VK_NUMPAD0, VK_NUMPAD9, VK_PRIOR, VK_RIGHT, VK_SUBTRACT,
        VK_UP,
    };
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{WM_CHAR, WM_KEYDOWN, WM_KEYUP};

    static CTX: OnceLock<egui::Context> = OnceLock::new();
    /// Zeichen, das nach einem abgefangenen Tastendruck als WM_CHAR folgt und verschluckt werden muss.
    static EXPECT_CHAR: AtomicU32 = AtomicU32::new(0);

    const EXTENDED: isize = 1 << 24;

    /// Ordnet eine virtuelle Taste einer Ziffernblock-Taste zu.
    fn map(vk: u16, lparam: LPARAM) -> Option<&'static str> {
        const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
        if (VK_NUMPAD0..=VK_NUMPAD9).contains(&vk) {
            return Some(DIGITS[(vk - VK_NUMPAD0) as usize]);
        }
        match vk {
            VK_ADD => return Some("+"),
            VK_SUBTRACT => return Some("-"),
            VK_MULTIPLY => return Some("*"),
            VK_DIVIDE if lparam & EXTENDED != 0 => return Some("/"),
            VK_DECIMAL => return Some("."),
            _ => {}
        }
        // NumLock aus: Ziffernblock sendet Navigationstasten ohne Erweitert-Bit
        if lparam & EXTENDED != 0 {
            return None;
        }
        Some(match vk {
            VK_INSERT => "0",
            VK_END => "1",
            VK_DOWN => "2",
            VK_NEXT => "3",
            VK_LEFT => "4",
            VK_CLEAR => "5",
            VK_RIGHT => "6",
            VK_HOME => "7",
            VK_UP => "8",
            VK_PRIOR => "9",
            VK_DELETE => ".",
            _ => return None,
        })
    }

    fn modifier_down() -> bool {
        unsafe { GetKeyState(VK_CONTROL as i32) < 0 || GetKeyState(VK_MENU as i32) < 0 }
    }

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if active() {
            match msg {
                WM_KEYDOWN => {
                    let vk = wparam as u16;
                    match map(vk, lparam) {
                        Some(key) if !modifier_down() => {
                            QUEUE.lock().unwrap().push(key);
                            // Bei NumLock an folgt ein WM_CHAR mit dem Zeichen – vormerken
                            let c = key.chars().next().unwrap() as u32;
                            EXPECT_CHAR.store(c, Ordering::Relaxed);
                            if let Some(ctx) = CTX.get() {
                                ctx.request_repaint();
                            }
                            return 0;
                        }
                        _ => EXPECT_CHAR.store(0, Ordering::Relaxed),
                    }
                }
                WM_KEYUP => {
                    if map(wparam as u16, lparam).is_some() && !modifier_down() {
                        return 0;
                    }
                }
                WM_CHAR => {
                    let expected = EXPECT_CHAR.swap(0, Ordering::Relaxed);
                    if expected != 0 && expected == wparam as u32 {
                        return 0;
                    }
                }
                _ => {}
            }
        }
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }

    /// Installiert den Ziffernblock-Hook am Hauptfenster.
    pub fn install(cc: &eframe::CreationContext<'_>) -> bool {
        let _ = CTX.set(cc.egui_ctx.clone());
        let Ok(handle) = cc.window_handle() else { return false };
        let RawWindowHandle::Win32(h) = handle.as_raw() else {
            return false;
        };
        let hwnd = h.hwnd.get() as HWND;
        unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), 0x4d55_4421, 0) != 0 }
    }
}
