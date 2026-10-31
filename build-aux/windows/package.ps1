# Builds dist/Ductus-<version>-windows-x64.zip and -setup.exe.
# Needs Rust (MSVC), Qt 6.10 for MSVC (QT_ROOT_DIR), Git Bash and Inno Setup 6.
#   package.ps1 [-Version 0.2.0] [-SkipTests] [-NoInstaller]
param(
    [string]$Version = "",
    [switch]$SkipTests,
    [switch]$NoInstaller
)
$ErrorActionPreference = "Stop"
Set-Location (Resolve-Path "$PSScriptRoot\..\..")

if (-not $env:QT_ROOT_DIR) { throw "QT_ROOT_DIR must point at Qt for MSVC" }
if (-not $Version) {
    $Version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"$' | Select-Object -First 1).Matches[0].Groups[1].Value
}
# Git Bash by path: a bare `bash` can be WSL's.
$bash = Join-Path $env:ProgramFiles "Git\bin\bash.exe"
$env:QMAKE = "$env:QT_ROOT_DIR\bin\qmake.exe"
cargo build --release --locked -p ductus-qt
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$app = "target\windows\Ductus"
Remove-Item -Recurse -Force $app -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $app | Out-Null
Copy-Item target\release\ductus.exe $app\
& $bash build-aux/fetch-pdfium.sh win-x64 target/pdfium
if ($LASTEXITCODE -ne 0) { throw "pdfium download failed" }
Copy-Item target\pdfium\bin\pdfium.dll $app\
& $bash build-aux/fetch-onnxruntime.sh win-x64 target/onnxruntime
if ($LASTEXITCODE -ne 0) { throw "ONNX Runtime download failed" }
Copy-Item target\onnxruntime\lib\onnxruntime.dll $app\
Copy-Item LICENSE $app\LICENSE.txt
# The MSVC runtime goes next to ductus.exe, so neither the zip nor the installer needs the
# VC++ redistributable on the machine (windeployqt would only add its installer).
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$crt = Get-ChildItem "$vs\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Parent.Parent.Name -match '^\d+(\.\d+)+$' } |
    Sort-Object { [version]$_.Parent.Parent.Name } | Select-Object -Last 1
if (-not $crt) { throw "MSVC runtime not found under $vs" }
Copy-Item "$($crt.FullName)\*.dll" $app\
# Qt Quick renders with Direct3D 11 and ships its shaders compiled, so DXC isn't needed.
& "$env:QT_ROOT_DIR\bin\windeployqt.exe" --release --no-translations --no-compiler-runtime `
    --no-system-dxc-compiler --qmldir crates\ductus-qt\qml $app\ductus.exe
if ($LASTEXITCODE -ne 0) { throw "windeployqt failed" }
# windeployqt only adds the Windows platform; the UI tests need offscreen.
Copy-Item "$env:QT_ROOT_DIR\plugins\platforms\qoffscreen.dll" $app\platforms\

if (-not $SkipTests) {
    & $bash build-aux/ui-tests.sh target/windows/Ductus/ductus.exe
    if ($LASTEXITCODE -ne 0) { throw "UI tests failed" }
}

New-Item -ItemType Directory -Force dist | Out-Null
$zip = "dist\Ductus-$Version-windows-x64.zip"
Remove-Item -Force $zip -ErrorAction SilentlyContinue
Compress-Archive -Path $app -DestinationPath $zip
if ($NoInstaller) { exit 0 }

$iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) { $iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" }
& $iscc /Qp "/DAppVersion=$Version" "/DSourceDir=$((Resolve-Path $app).Path)" "/DOutputDir=$((Resolve-Path dist).Path)" build-aux\windows\ductus.iss
if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed" }
