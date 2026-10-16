//! Build script for notecraft.exe and the notecraft binary.
//!
//! - Every target: version info (build SHA and date) from build_info.rs.
//! - Windows: embed the app icon and VERSIONINFO, so the exe shows its icon in Explorer, the
//!   taskbar, the Start menu, and Alt-Tab. A missing resource compiler is a warning (so a
//!   cross-compile still links) unless NOTECRAFT_REQUIRE_WINRES=1, which package.ps1 sets for
//!   release builds. Adapted from the GridCraft template (MIT OR Apache-2.0, see ATTRIBUTION.md).

include!("build_info.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_info.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/notecraft.ico");
    println!("cargo:rerun-if-env-changed=NOTECRAFT_REQUIRE_WINRES");
    emit_build_info();
    // The About page shows the target triple; Cargo always sets TARGET for build scripts.
    let target = std::env::var("TARGET")
        .ok()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=NOTECRAFT_BUILD_TARGET={target}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/notecraft.ico")
        .set("ProductName", "NoteCraft")
        .set("FileDescription", "NoteCraft")
        .set("CompanyName", "The NoteCraft Contributors")
        .set(
            "LegalCopyright",
            "Copyright (c) The NoteCraft Contributors. MIT OR Apache-2.0.",
        )
        .set("OriginalFilename", "notecraft.exe")
        .set("InternalName", "notecraft");
    if let Err(e) = res.compile() {
        if std::env::var_os("NOTECRAFT_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=notecraft.exe built without icon/version resources: {e}");
    }
}
