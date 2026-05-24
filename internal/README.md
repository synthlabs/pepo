# Internal Features

`internal/` contains a small tracked starter set plus ignored local-only experiment files.
New clones and CI use the tracked starter files; private experiments should remain untracked unless explicitly force-added.

## Run

```sh
ENABLE_INTERNAL=1 pnpm tauri dev
```

Without `ENABLE_INTERNAL=1`, the app uses tracked no-op hooks and builds as the public open-source app.

## Frontend

The frontend entrypoint is `internal/frontend/index.ts`.

```ts
export { default as InternalRoot } from "./InternalRoot.svelte";
export const navItems = [];
```

`InternalRoot` is mounted by the public layout. Keep feature UI inside `internal/frontend` and import public app modules with normal aliases like `$lib`.

## Rust

The Rust entrypoint is the `pepo-internal` crate in `internal/rust`.
`src-tauri/build.rs` imports `pepo_internal::COMMANDS` to generate the `internal:default` permission.
Add every internal plugin command to `COMMANDS`, the Tauri invoke handler, and the Specta command list.

```rust
pub const COMMANDS: &[&str] = &["internal_ping"];

use tauri::{plugin::TauriPlugin, Runtime};
use tauri_specta::collect_commands;

#[tauri::command]
#[specta::specta]
fn internal_ping() -> String {
    "pong from internal".to_owned()
}

fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("internal")
        .invoke_handler(tauri::generate_handler![internal_ping])
        .build()
}

pub fn apply_plugins<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.plugin(init())
}

#[cfg(debug_assertions)]
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name("internal")
        .commands(collect_commands![internal_ping])
}

pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
```

Debug internal builds export internal-inclusive bindings to `internal/frontend/bindings.ts`. The tracked `src/lib/bindings.ts` remains public-only.

## Verify

```sh
sh internal/verify.sh
```

The command should finish with a clean tracked worktree. Generated files under `internal/` are ignored.
