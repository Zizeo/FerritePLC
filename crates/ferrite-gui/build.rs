fn main() {
    #[cfg(target_os = "windows")]
    {
        use std::path::Path;

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let ico_path = Path::new(&manifest_dir).join("assets/ferrite.ico");
        let fallback_path = Path::new(&manifest_dir).join("../../assets/ferrite.ico");

        let icon_to_use = if ico_path.exists() {
            Some(ico_path)
        } else if fallback_path.exists() {
            Some(fallback_path)
        } else {
            None
        };

        if let Some(path) = icon_to_use {
            let mut res = winresource::WindowsResource::new();
            if let Some(path_str) = path.to_str() {
                res.set_icon(path_str);
            } else {
                res.set_icon("assets/ferrite.ico");
            }
            res.set("FileDescription", "FerritePLC Studio — Industrial PLC IDE & Simulator");
            res.set("ProductName", "FerritePLC Studio");
            res.set("OriginalFilename", "ferrite-gui.exe");
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=Failed to compile Windows resource: {e}");
            }
        }
    }
}
