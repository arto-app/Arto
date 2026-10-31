# Run a command in a vitro Windows guest with what Arto's recipes need beside
# the toolchain the guest image already carries (Rust, MSVC, LLVM, Git):
# Node with pnpm through corepack, just, and Git's bash, which just runs the
# recipes with. `vitro.toml` runs the tests through it:
#
#   vitro exec --sync . windows powershell -ExecutionPolicy Bypass `
#     -File platform/windows/vitro.ps1 just standalone
#
# Through `powershell -File` rather than as `./vitro.ps1`, because a fresh
# Windows refuses to run scripts by default.
#
# Every VM starts from the golden image, so the tools are fetched into the VM
# each time rather than baked into the image: an image is the user's own and
# Arto has no say over it. Whatever is already installed is left alone.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# The versions CI's setup-toolchain action installs.
$nodeVersion = '24.22.2'
$justVersion = '1.58.0'

$arm = $env:PROCESSOR_ARCHITECTURE -eq 'ARM64'
$tools = Join-Path $env:LOCALAPPDATA 'arto-tools'
New-Item -ItemType Directory -Force -Path $tools | Out-Null

function Get-Archive([string]$Url, [string]$Destination) {
    $zip = Join-Path $tools 'download.zip'
    Write-Host "downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile $zip -UseBasicParsing
    Expand-Archive $zip -DestinationPath $Destination -Force
    Remove-Item $zip
}

$node = "node-v$nodeVersion-win-$(if ($arm) { 'arm64' } else { 'x64' })"
if (-not (Test-Path (Join-Path $tools "$node\node.exe"))) {
    Get-Archive "https://nodejs.org/dist/v$nodeVersion/$node.zip" $tools
}

$just = Join-Path $tools "just-$justVersion"
if (-not (Test-Path (Join-Path $just 'just.exe'))) {
    $triple = "$(if ($arm) { 'aarch64' } else { 'x86_64' })-pc-windows-msvc"
    Get-Archive "https://github.com/casey/just/releases/download/$justVersion/just-$justVersion-$triple.zip" $just
}

# Git's installer puts only Git\cmd on PATH; bash and sh live in Git\bin.
$env:Path = @((Join-Path $tools $node), $just, "$env:ProgramFiles\Git\bin", $env:Path) -join ';'

# corepack writes its pnpm shim beside node.exe, so it needs no
# administrator; COREPACK_ENABLE_DOWNLOAD_PROMPT keeps the first pnpm call
# from waiting on a question nobody can answer.
$env:COREPACK_ENABLE_DOWNLOAD_PROMPT = '0'
corepack enable
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($args.Count -eq 0) { exit 0 }
# @(...) keeps a single remaining argument an array; splatting a bare string
# would pass its first character alone.
$command = $args[0]
$rest = @($args | Select-Object -Skip 1)
& $command @rest
exit $LASTEXITCODE
