param([switch]$Full)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$checkFailures = [System.Collections.Generic.List[string]]::new()

Push-Location -LiteralPath $projectRoot
try {
    $testFilters = if ($Full) { @('') } else { @('eventsub::', 'logging::') }
    foreach ($filter in $testFilters) {
        & cargo test --locked -p pepo --lib $filter
        if ($LASTEXITCODE -ne 0) { $checkFailures.Add("Rust tests: $filter") }
    }
    & cargo check --locked -p pepo
    if ($LASTEXITCODE -ne 0) { $checkFailures.Add('Rust check') }
    & git diff --check
    if ($LASTEXITCODE -ne 0) { $checkFailures.Add('Diff whitespace') }
    if ($checkFailures.Count -gt 0) { throw ('Failed checks: ' + ($checkFailures -join ', ')) }
} finally {
    Pop-Location
}
