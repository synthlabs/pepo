fn main() {
    println!("cargo:rustc-check-cfg=cfg(internal_enabled)");
    println!("cargo:rerun-if-env-changed=ENABLE_INTERNAL");
    println!("cargo:rerun-if-changed=../internal/rust/mod.rs");

    let internal_enabled = std::env::var("ENABLE_INTERNAL").as_deref() == Ok("1");
    if internal_enabled && std::path::Path::new("../internal/rust/mod.rs").exists() {
        println!("cargo:rustc-cfg=internal_enabled");
    }

    tauri_build::build()
}
