//! A `no_std` library takes no C library by itself, and on macOS the linker
//! wants `memcpy` and the like resolved: the emulator that loads it has
//! them, so link against the system's.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=System");
    }
}
