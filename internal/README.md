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
export { default as InternalRoot } from './InternalRoot.svelte';
export const navItems = [];
```

`InternalRoot` is mounted by the public layout. Keep feature UI inside `internal/frontend` and import public app modules with normal aliases like `$lib`.

## Rust

The Rust entrypoint is the `pepo-internal` crate in `internal/rust`.
`src-tauri/build.rs` imports `pepo_internal::COMMANDS` to generate the `internal:default` permission.
Add every internal plugin command to `COMMANDS`, the Tauri invoke handler, and the Specta command list.

The starter command is `internal_build_info`, which returns app build metadata from internal plugin state.

Debug internal builds export internal-inclusive bindings to `internal/frontend/bindings.ts`. The tracked `src/lib/bindings.ts` remains public-only.

## Verify

```sh
sh internal/verify.sh
```

The command should finish with a clean tracked worktree. Generated files under `internal/` are ignored.
