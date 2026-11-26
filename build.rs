// Embed the app icon and version information in the Windows executable.
fn main() {
    println!("cargo:rerun-if-changed=packaging/icons/ferrum-paint.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/icons/ferrum-paint.ico")
            .set("ProductName", "Ferrum Paint")
            .set("FileDescription", "Ferrum Paint")
            .set("CompanyName", "Tim Lorne")
            .set("LegalCopyright", "Copyright (c) 2026 Tim Lorne. MIT licence.")
            .set("OriginalFilename", "ferrum-paint.exe");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
