fn main() {
    println!("cargo:rerun-if-changed=../../packaging/icons/icon.ico");
    // Embed the application icon and version info into the Windows executable.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/icons/icon.ico")
            .set("ProductName", "Macropad Remapper")
            .set("FileDescription", "Macropad Remapper")
            .set("LegalCopyright", "MIT OR Apache-2.0");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows icon: {e}");
        }
    }
}
