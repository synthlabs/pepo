param([switch]$Full, [string]$Filter = 'auth_recovery')

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot

Push-Location -LiteralPath $projectRoot
try {
  $testArguments = @('test', '--locked', '-p', 'pepo')
  if (-not $Full) {
    $testArguments += @('--lib', $Filter)
  }
  & cargo @testArguments
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

  & cargo check --locked -p pepo
  exit $LASTEXITCODE
} finally {
  Pop-Location
}
