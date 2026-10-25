param([switch]$Browser, [switch]$Full)
$ErrorActionPreference = 'Stop'
$script:checkFailures = [System.Collections.Generic.List[string]]::new()
function Invoke-Check([string]$Name, [scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) { $script:checkFailures.Add($Name) }
}
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    if ($Full) {
        Invoke-Check 'Rust tests' { cargo test --manifest-path src-tauri/Cargo.toml --locked }
        Invoke-Check 'Frontend tests' { pnpm test:run }
        Invoke-Check 'Lint' { make lint }
        Invoke-Check 'Frontend build' { pnpm build }
    } else {
        Invoke-Check 'Rust compile' { cargo check --manifest-path src-tauri/Cargo.toml --locked }
        Invoke-Check 'Auth Rust tests' { cargo test --manifest-path src-tauri/Cargo.toml --locked token:: }
        Invoke-Check 'Chat Rust tests' { cargo test --manifest-path src-tauri/Cargo.toml --locked chat:: }
        Invoke-Check 'EventSub tests' { cargo test --manifest-path src-tauri/Cargo.toml --locked eventsub:: }
        Invoke-Check 'Frontend types' { pnpm check }
        Invoke-Check 'Chat UI tests' { pnpm exec vitest run src/lib/chat/feed.test.ts src/lib/chat/history-boundary.test.ts src/lib/chat/message-history.test.ts src/lib/chat/translation.test.ts src/lib/settings.test.ts }
    }
    Invoke-Check 'Working diff whitespace' { git diff --check }
    Invoke-Check 'Staged diff whitespace' { git diff --cached --check }
    git grep -n -E '^(<<<<<<< |=======$|>>>>>>> )' -- src src-tauri scripts
    if ($LASTEXITCODE -ne 1) { $script:checkFailures.Add('Conflict markers') }
    if ($Browser) {
        Invoke-Check 'Chat browser scenarios' { node scripts/check-chat-engine.mjs }
    }
    if ($script:checkFailures.Count -gt 0) { throw ('Failed checks: ' + ($script:checkFailures -join ', ')) }
} finally {
    Pop-Location
}
