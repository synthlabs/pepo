param([switch]$Full)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot

Push-Location -LiteralPath $projectRoot
try {
  if ($Full) {
    & cargo test --locked -p pepo
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } else {
    & cargo test --locked -p pepo --lib channel_cache_poll
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    # Run the existing auth log-capture regressions serially.
    & cargo test --locked -p pepo --lib auth_recovery -- --test-threads=1
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  }

  & cargo check --locked -p pepo
  exit $LASTEXITCODE
} finally {
  Pop-Location
}
