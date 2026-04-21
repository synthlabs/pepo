use specta_typescript::Typescript;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has no parent directory")
        .join("src/lib/bindings.ts");

    pepo_lib::specta_builder()
        .export(
            Typescript::default()
                .formatter(specta_typescript::formatter::prettier)
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .header("/* eslint-disable */"),
            &out,
        )
        .expect("Failed to export typescript bindings");

    println!("Bindings exported to {}", out.display());
}
