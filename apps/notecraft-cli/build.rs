//! Build script for notecraft-cli: version info (build SHA and date) only.

include!("../notecraft/build_info.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../notecraft/build_info.rs");
    emit_build_info();
}
