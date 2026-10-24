use tauri_build::{Attributes, DefaultPermissionRule, InlinedPlugin};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(internal_enabled)");
    println!("cargo:rerun-if-env-changed=ENABLE_INTERNAL");

    let internal_enabled = std::env::var("ENABLE_INTERNAL").as_deref() == Ok("1");
    if internal_enabled {
        println!("cargo:rustc-cfg=internal_enabled");
    }

    inbound_build::stamp();
    let attrs = Attributes::new()
        .plugin(
            "inbound",
            InlinedPlugin::new()
                .commands(inbound_build::COMMANDS)
                .default_permission(DefaultPermissionRule::AllowAllCommands),
        )
        .plugin(
            "internal",
            InlinedPlugin::new()
                .commands(pepo_internal::COMMANDS)
                .default_permission(DefaultPermissionRule::AllowAllCommands),
        );

    tauri_build::try_build(attrs).expect("failed to run tauri build");

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // Library test executables also need Common Controls v6.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
        // Tauri already embeds the manifest in binary resources.
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    }
}
