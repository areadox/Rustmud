//! Netzwerk: eigener Lese-Thread pro Verbindung, Telnet-Verarbeitung dort,
//! Ereignisse per Kanal an die Oberfläche.

use crate::telnet::{TelnetConfig, TelnetEvent, TelnetParser};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub enum NetEvent {
    Connected(String),
    Telnet(TelnetEvent),
    Disconnected(String),
}

pub struct Connection {
    writer: Arc<Mutex<Option<TcpStream>>>,
    pub rx: Receiver<NetEvent>,
}

impl Connection {
    /// Startet den Verbindungsaufbau im Hintergrund. `wake` wird nach jedem Ereignis aufgerufen
    /// (z. B. um die Oberfläche neu zu zeichnen).
    pub fn open(
        host: String,
        port: u16,
        cfg: TelnetConfig,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (tx, rx) = channel();
        let writer: Arc<Mutex<Option<TcpStream>>> = Arc::new(Mutex::new(None));
        let w2 = writer.clone();
        std::thread::Builder::new()
            .name("mud-net".into())
            .spawn(move || {
                let send = |ev: NetEvent| {
                    let ok = tx.send(ev).is_ok();
                    wake();
                    ok
                };
                let addrs = match (host.as_str(), port).to_socket_addrs() {
                    Ok(a) => a.collect::<Vec<_>>(),
                    Err(e) => {
                        send(NetEvent::Disconnected(format!(
                            "Adresse {host}:{port} nicht auflösbar: {e}"
                        )));
                        return;
                    }
                };
                let mut stream = None;
                let mut last_err = String::from("keine Adresse gefunden");
                for a in &addrs {
                    match TcpStream::connect_timeout(a, Duration::from_secs(15)) {
                        Ok(s) => {
                            stream = Some(s);
                            break;
                        }
                        Err(e) => last_err = e.to_string(),
                    }
                }
                let Some(mut stream) = stream else {
                    send(NetEvent::Disconnected(format!(
                        "Verbindung zu {host}:{port} fehlgeschlagen: {last_err}"
                    )));
                    return;
                };
                let _ = stream.set_nodelay(true);
                match stream.try_clone() {
                    Ok(c) => *w2.lock().unwrap() = Some(c),
                    Err(e) => {
                        send(NetEvent::Disconnected(format!("Socket-Fehler: {e}")));
                        return;
                    }
                }
                let peer = stream
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                if !send(NetEvent::Connected(peer)) {
                    return;
                }
                let mut parser = TelnetParser::new(cfg);
                let mut buf = vec![0u8; 64 * 1024];
                let reason = loop {
                    match stream.read(&mut buf) {
                        Ok(0) => break "Verbindung vom Server geschlossen".to_string(),
                        Ok(n) => {
                            let mut events = Vec::new();
                            let mut reply = Vec::new();
                            parser.feed(&buf[..n], &mut events, &mut reply);
                            if !reply.is_empty() {
                                if let Some(w) = w2.lock().unwrap().as_mut() {
                                    let _ = w.write_all(&reply);
                                }
                            }
                            for ev in events {
                                if tx.send(NetEvent::Telnet(ev)).is_err() {
                                    return;
                                }
                            }
                            wake();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(e) => break format!("Verbindung getrennt: {e}"),
                    }
                };
                *w2.lock().unwrap() = None;
                send(NetEvent::Disconnected(reason));
            })
            .expect("Netzwerk-Thread starten");
        Self { writer, rx }
    }

    /// Sendet rohe Bytes (müssen bereits IAC-escaped sein).
    pub fn send_raw(&self, data: &[u8]) -> Result<(), String> {
        match self.writer.lock().unwrap().as_mut() {
            Some(w) => w.write_all(data).map_err(|e| e.to_string()),
            None => Err("nicht verbunden".into()),
        }
    }

    pub fn close(&self) {
        if let Some(w) = self.writer.lock().unwrap().as_ref() {
            let _ = w.shutdown(Shutdown::Both);
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}
