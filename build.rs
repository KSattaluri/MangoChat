fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icons/mango.ico");
        res.set("FileDescription", "Mango Chat");
        res.set("ProductName", "Mango Chat");
        res.set("OriginalFilename", "mangochat.exe");
        res.set("InternalName", "mangochat");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=failed to compile Windows resources: {e}");
        }
    }
}
