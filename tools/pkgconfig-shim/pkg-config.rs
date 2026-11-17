// Minimal pkg-config shim used only for cross-compiling FerritePLC to Linux.
//
// `libudev-sys` (pulled in transitively by `serialport`) insists on asking
// pkg-config where libudev lives. Zig's bundled sysroot has no libudev, so we
// answer the query ourselves and point at the vendored stub in
// tools/linux-sysroot (see .cargo/config.toml).
//
// Build:  rustc -O --edition 2021 pkg-config.rs -o pkg-config.exe
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let joined = args.join(" ");

    if joined.contains("--modversion") {
        println!("1");
        return;
    }
    if joined.contains("--version") {
        println!("0.29.2");
        return;
    }
    if !joined.contains("libudev") {
        // Nothing else in this workspace needs a pkg-config answer.
        return;
    }

    // musl builds are static: no libudev to link against, and nothing in the
    // workspace references its symbols (the linker drops the -ludev otherwise).
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("musl") {
        return;
    }

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("pkg-config shim: cannot locate itself: {e}");
            std::process::exit(1);
        }
    };
    // <repo>/tools/pkgconfig-shim/pkg-config.exe -> <repo>
    let root = exe
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent());
    let libdir = match root {
        Some(root) => root.join("tools/linux-sysroot/usr/lib/x86_64-linux-gnu"),
        None => {
            eprintln!("pkg-config shim: unexpected install location");
            std::process::exit(1);
        }
    };

    let libdir = libdir.to_string_lossy().replace('\\', "/");
    println!("-L{libdir} -ludev");
}
