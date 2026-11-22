//! Bettet unter Windows Programmsymbol und Versionsinfos in die .exe ein.

fn main() {
    println!("cargo:rerun-if-changed=assets/rustmud.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/rustmud.ico")
            .set("ProductName", "rustmud")
            .set("FileDescription", "rustmud – MUD-Client")
            .set("CompanyName", "areadox")
            .set("LegalCopyright", "© 2026 areadox – MIT-Lizenz");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Ressourcen konnten nicht eingebettet werden: {e}");
        }
    }
}
