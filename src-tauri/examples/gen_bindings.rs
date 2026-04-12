use specta_typescript::Typescript;

fn main() {
    pepo_lib::specta_builder()
        .export(
            Typescript::default()
                .formatter(specta_typescript::formatter::prettier)
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .header("/* eslint-disable */"),
            "../src/lib/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    println!("Bindings exported to src/lib/bindings.ts");
}
