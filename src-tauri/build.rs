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
}
