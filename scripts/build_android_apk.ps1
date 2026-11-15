# Script automatizado para compilar y generar el APK de Pro Wave para Android
param (
    [string]$Target = "universal", # "universal", "x86_64" o "arm64-v8a"
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RootDir = Split-Path -Parent $ScriptDir
Set-Location $RootDir

# Asegurar Cargo y herramientas en PATH
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# Localizar Android SDK y NDK
if (-not $env:ANDROID_HOME) {
    $env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
}
if (-not $env:NDK_HOME) {
    $env:NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\26.1.10909125"
}

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Compilando Pro Wave DAW para Android ($Target)" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Compilación de la biblioteca nativa en Rust con cargo-ndk
Write-Host "`n[1/4] Compilando librerias nativas con cargo ndk..." -ForegroundColor Yellow
$profileFlag = if ($Profile -eq "release") { "--release" } else { "" }

$targetsToBuild = @()
if ($Target -eq "universal") {
    $targetsToBuild = @("x86_64", "arm64-v8a")
} else {
    $targetsToBuild = @($Target)
}

foreach ($t in $targetsToBuild) {
    Write-Host " -> Compilando para $t..." -ForegroundColor Cyan
    cargo ndk -t $t -P 26 build -p prowave-android $profileFlag
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Fallo la compilacion para $t con cargo ndk"
        exit 1
    }
}

# 2. Preparar empaquetado APK
Write-Host "`n[2/4] Preparando estructura de empaquetado..." -ForegroundColor Yellow
$sdk = $env:ANDROID_HOME
$buildTools = "$sdk\build-tools\34.0.0"
$platform = "$sdk\platforms\android-35\android.jar"
$manifest = "$RootDir\crates\prowave-android\AndroidManifest.xml"
$pkgDir = "$RootDir\target\android-pkg"

New-Item -ItemType Directory -Path "$RootDir\dist" -Force | Out-Null

foreach ($t in $targetsToBuild) {
    $rustTarget = if ($t -eq "x86_64") { "x86_64-linux-android" } else { "aarch64-linux-android" }
    $subDir = if ($Profile -eq "release") { "release" } else { "debug" }
    $soSrc = "$RootDir\target\$rustTarget\$subDir\libprowave_android.so"
    $destDir = "$pkgDir\lib\$t"
    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
    Copy-Item $soSrc "$destDir\libprowave_android.so" -Force
}

$unaligned = "$pkgDir\unaligned.apk"
$aligned = "$pkgDir\aligned.apk"
$finalApk = "$RootDir\dist\ProTrack-$Target.apk"

Remove-Item $unaligned, $aligned, $finalApk -ErrorAction SilentlyContinue

# 3. Empaquetar con aapt y zipalign
Write-Host "`n[3/4] Empaquetando recursos y alineando con zipalign..." -ForegroundColor Yellow
& "$buildTools\aapt.exe" package -F $unaligned -M $manifest -I $platform -f
Push-Location $pkgDir
foreach ($t in $targetsToBuild) {
    & "$buildTools\aapt.exe" add "unaligned.apk" "lib/$t/libprowave_android.so"
}
Pop-Location
& "$buildTools\zipalign.exe" -f -p 4 $unaligned $aligned

# 4. Firmar APK (Clave persistente para que la firma nunca cambie)
Write-Host "`n[4/4] Firmando APK con apksigner (clave persistente)..." -ForegroundColor Yellow
$keystoreDir = "$RootDir\crates\prowave-android\keystore"
$keystore = "$keystoreDir\prowave-debug.keystore"
if (-not (Test-Path $keystoreDir)) {
    New-Item -ItemType Directory -Path $keystoreDir -Force | Out-Null
}
if (-not (Test-Path $keystore)) {
    & keytool -genkeypair -keystore $keystore -storepass android -alias androiddebugkey -keypass android -dname "CN=Android Debug,O=Android,C=US" -validity 10000 -keyalg RSA -keysize 2048
}
& "$buildTools\apksigner.bat" sign --ks $keystore --ks-pass pass:android --out $finalApk $aligned

# Si es universal, tambien actualizar ProWave-arm64-v8a.apk para compatibilidad de rutas
if ($Target -eq "universal") {
    Copy-Item $finalApk "$RootDir\dist\ProTrack-arm64-v8a.apk" -Force
}

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "   ¡APK universal generado con exito!" -ForegroundColor Green
Write-Host "   Ubicacion: $finalApk" -ForegroundColor Green
Write-Host "   Tamano: $((Get-Item $finalApk).Length / 1MB) MB" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
