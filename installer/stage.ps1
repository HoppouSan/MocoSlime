# Stages build artifacts for installer/mocoslime.iss.
# Run from the workspace root: powershell -File installer/stage.ps1
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$stage = Join-Path $PSScriptRoot "stage"
$installerDir = [System.IO.Path]::GetFullPath($PSScriptRoot)
$stagePath = [System.IO.Path]::GetFullPath($stage)
$expectedStagePath = [System.IO.Path]::Combine($installerDir, "stage")
if ($stagePath -ne $expectedStagePath) {
    throw "Refusing to clean unexpected staging path: $stagePath"
}
$flutterOut = Join-Path $root "flutter\build\windows\x64\runner\Release"
$rustDll = Join-Path $root "target\release\moslime_ffi.dll"

if (-not (Test-Path (Join-Path $flutterOut "Mocoslime.exe"))) {
    throw "Mocoslime.exe not found. Run 'flutter build windows --release' first."
}
if (-not (Test-Path $rustDll)) {
    throw "moslime_ffi.dll not found. Run 'cargo build --release' first."
}

if (Test-Path -LiteralPath $stagePath) {
    Remove-Item -LiteralPath $stagePath -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $stagePath | Out-Null
Copy-Item (Join-Path $flutterOut "Mocoslime.exe") $stage -Force
Copy-Item $rustDll $stage -Force
Copy-Item (Join-Path $flutterOut "data") (Join-Path $stage "data") -Recurse -Force
Get-ChildItem $flutterOut -Filter *.dll | ForEach-Object {
    if ($_.Name -ne "Mocoslime.exe") { Copy-Item $_.FullName $stage -Force }
}

Get-ChildItem $stage | Format-Table Name, Length
Write-Host "Staged OK: $stage"
