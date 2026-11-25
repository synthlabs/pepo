# Pepo tests

`pnpm test` runs the portable suites sequentially: app unit tests, tooling/CI script tests, Rust
unit tests, mocked packaging tests, Playwright Chromium browser tests, and the internal hook. A
failure stops the sequence.

`pnpm verify` runs frontend type-checking, linting/formatting, frontend build, `pnpm test`, Rust
formatting (`cargo fmt --all -- --check`), Rust linting (`cargo clippy --workspace --all-targets --locked -- -D warnings`),
and root/submodule whitespace diff checks. `make test` and `make verify` delegate to these commands.

## Setup

Portable CI targets Node 26, the pnpm version declared in `package.json`, and the Rust toolchain
(`rustfmt` + `clippy`) with Tauri platform build prerequisites.

```sh
pnpm install --frozen-lockfile
pnpm exec playwright install chromium
```

Linux hosts running browser tests also need Playwright's system dependencies
(`pnpm exec playwright install --with-deps chromium`).

## Commands

| Suite      | Command                | Make alias             | Covers                                                                                                                                         | Portable?            |
| ---------- | ---------------------- | ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| Unit       | `pnpm test:unit`       | `make test-unit`       | Vitest 5 jsdom suite under `tests/unit/**/*.test.ts` (app logic, Svelte components, CI contract)                                               | Yes                  |
| Scripts    | `pnpm test:scripts`    | `make test-scripts`    | `node --test tests/scripts/*.test.mjs` (patch amendment, CI/ESLint contracts, `make dev` env, Tailwind class lint, window-controls diagnostic) | Yes                  |
| Rust       | `pnpm test:rust`       | `make test-rust`       | `cargo test --locked --manifest-path src-tauri/Cargo.toml`                                                                                     | Yes                  |
| Rust build | `pnpm test:rust:build` | `make test-rust-build` | Compiles Rust tests without running (`--no-run`)                                                                                               | Yes                  |
| Packaging  | `pnpm test:packaging`  | `make test-packaging`  | `node tests/packaging/run.mjs` (PowerShell installer tests on Windows; bash integration tests on Linux/macOS)                                  | Yes (CI skips macOS) |
| Browser    | `pnpm test:browser`    | `make test-browser`    | Playwright Chromium specs under `tests/browser/*.spec.ts` served by `tests/support/browser/dev-server.mjs`                                     | Yes (CI runs Linux)  |
| Internal   | `pnpm test:internal`   | `make test-internal`   | `node internal/internal-test.mjs` (`ENABLE_INTERNAL=1` frontend check, Rust check, and Rust library tests)                                     | Yes                  |

Focused reruns accept runner arguments:

```sh
pnpm test:unit tests/unit/chat/autoscroll.test.ts
node --test tests/scripts/ci.test.mjs
pnpm test:rust --lib chat::
pnpm test:browser user-card.spec.ts
pnpm test:browser --grep "expanded-dark"
```

Watch mode for unit tests:

```sh
pnpm exec vitest --config vitest.config.ts
```

## Suites

- **`tests/unit/**`** — Mirrors `src/lib/**`. Shared unit helpers and Svelte component fixtures live in `tests/support/unit/`. Includes `tests/unit/ci-contract.test.ts` to keep `package.json`, `.github/workflows/validation.yml`, and `Makefile` in sync. Reports write to `target/test-results/vitest.json`.
- **`tests/scripts/*.test.mjs`** — Node built-in test runner (`node:test`) covering `scripts/` tooling, `Makefile` targets, and GitHub workflow contracts.
- **`tests/packaging/`** — OS dispatcher `run.mjs` plus `integration.test.sh` (Linux/macOS) and `utils/packaging/tauri/install.test.ps1` (Windows).
- **`tests/browser/*.spec.ts`** — Serial Playwright Chromium specs (`workers: 1`, `retries: 0`) backed by `tests/support/browser/dev-server.mjs`, which serves isolated Vite harnesses on an ephemeral port (`Pepo browser port <n>`). Reports write to `target/test-results/playwright.json`; screenshots and traces are retained only on failure under `target/test-results/browser/`.
- **`internal/internal-test.mjs`** — Shared internal hook verifying `ENABLE_INTERNAL=1` frontend and Rust checks/tests without calling host aggregate test commands.
- **Rust (`src-tauri/src/**`)** — Inline `#[cfg(test)] mod tests` by default, with sibling `tests.rs` / `*_tests.rs` modules where suites are larger.

## Platform limits

| Suite                                  | Linux (CI)     | Windows (CI)       | macOS (CI)                       |
| -------------------------------------- | -------------- | ------------------ | -------------------------------- |
| `test:unit`                            | Run            | Run                | Run                              |
| `test:scripts`                         | Run            | Run                | Run                              |
| `test:rust`                            | Run            | Run                | Run                              |
| `test:packaging`                       | Run (`bash`)   | Run (`powershell`) | Skipped (`runner.os != 'macOS'`) |
| `test:browser`                         | Run (Chromium) | Local only         | Local only                       |
| `test:internal`                        | Run            | Run                | Run                              |
| AUR SSH container tests (`verify-ssh`) | Run (Docker)   | Skipped            | Skipped                          |

## Adding a test

1. **Choose the runtime folder**:
   - TypeScript/Svelte unit or component test → `tests/unit/<path-mirroring-src-lib>/<name>.test.ts` (import app modules via `$lib/...`; put test-only fixtures in `tests/support/unit/`).
   - Tooling/CI/Makefile test → `tests/scripts/<name>.test.mjs`.
   - Browser/layout/interaction spec → `tests/browser/<name>.spec.ts` (reusing `tests/support/browser/fixtures.ts` and a harness under `tests/support/browser/harnesses/`).
   - Rust unit test → inline `#[cfg(test)] mod tests` or sibling `tests.rs` in `src-tauri/src/`.
2. **Naming rules**: `<name>.test.ts` / `<name>.test.mjs` / `<name>.test.sh` / `<name>.test.ps1`; Playwright specs use `*.spec.ts`.
3. **Registering a new suite**: add `test:<suite>` to `package.json` (and chain it in `scripts.test` if portable), add a named CI step in `.github/workflows/validation.yml`, and document it in the Commands table above. `tests/unit/ci-contract.test.ts` fails if `package.json` and `validation.yml` drift.

## Diagnostics

Scripts under `scripts/diagnostics/` are explicit manual or debugging tools — **not** automated test suites and not part of `pnpm test`:

- `node scripts/diagnostics/window-controls.mjs` (`pnpm check:window-controls`) — Interactive KDE Wayland window-state verification checklist.
- `scripts/diagnostics/windows-test-loader.ps1` — Runs Windows SDK `cdb.exe` against a Rust test binary to trace loader errors (`0xc0000139`).
