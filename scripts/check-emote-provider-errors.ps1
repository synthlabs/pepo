$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot

Push-Location -LiteralPath $projectRoot
try {
  & cargo test --locked --lib
  $testExitCode = $LASTEXITCODE
} finally {
  Pop-Location
}

exit $testExitCode
