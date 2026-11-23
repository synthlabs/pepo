#!/usr/bin/env sh
set -eu

if [ -f project.inlang/settings.json ]; then
	pnpm exec paraglide-js compile \
		--project ./project.inlang \
		--outdir ./src/lib/paraglide \
		--strategy localStorage preferredLanguage baseLocale
fi

ENABLE_INTERNAL=0 pnpm check
ENABLE_INTERNAL=0 cargo check --manifest-path src-tauri/Cargo.toml
node internal/internal-test.mjs
git status --short
