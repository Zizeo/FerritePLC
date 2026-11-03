fn main() {
    #[cfg(target_os = "windows")]
    {
        use std::path::Path;

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let ico_path = Path::new(&manifest_dir).join("../../assets/ferrite.ico");

        if ico_path.exists() {
            let mut res = winresource::WindowsResource::new();
            if let Some(path_str) = ico_path.to_str() {
                res.set_icon(path_str);
            } else {
                res.set_icon("../../assets/ferrite.ico");
            }
            res.set("FileDescription", "FerritePLC — Industrial PLC Runtime & CLI Simulator");
            res.set("ProductName", "FerritePLC");
            res.set("OriginalFilename", "ferrite.exe");
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=Failed to compile Windows resource: {e}");
            }
        }
    }
}
