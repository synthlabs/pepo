#!/usr/bin/env sh
set -eu

if [ -f project.inlang/settings.json ]; then
	pnpm exec paraglide-js compile \
		--project ./project.inlang \
		--outdir ./src/lib/paraglide \
		--strategy localStorage preferredLanguage baseLocale
fi

pnpm check
cargo check --manifest-path src-tauri/Cargo.toml
ENABLE_INTERNAL=1 pnpm check
ENABLE_INTERNAL=1 cargo check --manifest-path src-tauri/Cargo.toml
git status --short
