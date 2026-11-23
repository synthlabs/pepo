param(
  [Parameter(Mandatory = $true)][string]$TestBinary,
  [string]$OutputPath = 'target/windows-test-loader.log'
)
$ErrorActionPreference = 'Stop'
$debugger = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Debuggers\x64\cdb.exe'
if (!(Test-Path -LiteralPath $debugger)) { throw 'Install the Windows SDK Debugging Tools to collect a loader trace.' }
$binary = (Resolve-Path -LiteralPath $TestBinary).Path
$logPath = [System.IO.Path]::GetFullPath($OutputPath)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $logPath) | Out-Null
& $debugger -o -G -xe cpr -logo $logPath -c 'sxe -c ".exr -1; .lastevent; k; q" 0xc0000139; g' $binary --list
exit $LASTEXITCODE
