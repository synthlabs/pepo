use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has no parent directory")
        .join("src/lib/bindings.ts");

    pepo_lib::export_bindings(&pepo_lib::specta_builder(), &out);

    println!("Bindings exported to {}", out.display());
}
