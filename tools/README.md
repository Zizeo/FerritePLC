# Cross-compilation support files

Used by `.cargo/config.toml` so `cargo zigbuild --target x86_64-unknown-linux-gnu`
works from Windows. Nothing here is part of the FerritePLC runtime.

## `linux-sysroot/usr/lib/x86_64-linux-gnu/libudev.so`

Link-only **stub** of `libudev` (SONAME `libudev.so.1`, 98 exported symbols with
empty bodies, no libc dependency).

Why: `ferrite-fieldbus-modbus` depends on `serialport`, which pulls in
`libudev-sys`. That crate asks pkg-config for libudev regardless of whether the
final binary ends up referencing it (the linker drops it again via
`--as-needed`). Zig's bundled glibc sysroot ships no libudev, and linking
against a real Ubuntu `libudev.so.1` fails because it requires `GLIBC_2.32+`
symbols that the default zig glibc target does not provide. A stub keeps the
target at the default glibc level, and if the library is ever needed at runtime
the real `libudev.so.1` from the target system is loaded, because the ELF only
records the SONAME.

Regenerate after changing the zig/glibc target:

```bash
# On a Linux box with readelf/nm:
readelf --dyn-syms --wide /usr/lib/x86_64-linux-gnu/libudev.so.1 \
  | awk '$4 == "FUNC" && $7 != "UND" { print $8 }' | sed 's/@.*//' | sort -u \
  | sed 's/.*/void &(void) {} /' > libudev_stub.c

zig build-lib libudev_stub.c -target x86_64-linux-gnu -dynamic -O ReleaseSmall \
  -femit-bin=libudev.so -fsoname=libudev.so.1
```

## `linux-sysroot/usr/lib/x86_64-linux-gnu/libdl.a`

Empty static archive (8 bytes, just the `!<arch>` header). winit/clipboard/x11
crates emit `-ldl`, which does not exist in musl — `dlopen` lives in musl's libc.
The empty archive satisfies the linker lookup, enabled for musl builds by the
`-L` in `.cargo/config.toml`.

## `pkgconfig-shim/pkg-config.exe`
A ~10 line Rust program that answers exactly one query: `libudev` resolves to
`-L<repo>/tools/linux-sysroot/usr/lib/x86_64-linux-gnu -ludev`. It exists
because `libudev-sys`'s build script panics if pkg-config is unavailable and
`LIBUDEV_NO_PKG_CONFIG` is set.

Rebuild with:

```powershell
rustc -O --edition 2021 tools/pkgconfig-shim/pkg-config.rs -o tools/pkgconfig-shim/pkg-config.exe
```
