# Script para compilar y firmar la release oficial definitiva de NOVA Studio
param (
    [string]$Target = "universal", # "universal", "arm64-v8a", "x86_64"
    [string]$VersionName = "1.0.18",
    [int]$VersionCode = 10018
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RootDir = Split-Path -Parent $ScriptDir
Set-Location $RootDir

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk" }
if (-not $env:NDK_HOME) { $env:NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\26.1.10909125" }

$sdk = $env:ANDROID_HOME
$buildTools = "$sdk\build-tools\34.0.0"
$platform = "$sdk\platforms\android-35\android.jar"
$pkgDir = "$RootDir\target\novastudio-pkg"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Compilando NOVA Studio v$VersionName ($Target)" -ForegroundColor Cyan
Write-Host "   Firma: Oficial de Produccion (nova-release.keystore)" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Compilar bibliotecas nativas
Write-Host "`n[1/4] Compilando librerias nativas con cargo ndk..." -ForegroundColor Yellow
$targetsToBuild = if ($Target -eq "universal") { @("x86_64", "arm64-v8a") } else { @($Target) }

foreach ($t in $targetsToBuild) {
    Write-Host " -> Compilando para $t..." -ForegroundColor Cyan
    cargo ndk -t $t -P 26 build -p prowave-android --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Fallo la compilacion para $t"
        exit 1
    }
}

# 2. Crear Manifest temporal con paquete org.novastudio.daw
Write-Host "`n[2/4] Generando AndroidManifest para NOVA Studio..." -ForegroundColor Yellow
New-Item -ItemType Directory -Path $pkgDir -Force | Out-Null
$manifestContent = @"
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.novastudio.nova"
    android:versionCode="$VersionCode"
    android:versionName="$VersionName">

    <uses-sdk android:minSdkVersion="26" android:targetSdkVersion="35" />

    <uses-feature android:name="android.hardware.audio.low_latency" android:required="false" />
    <uses-feature android:name="android.hardware.audio.pro" android:required="false" />
    <uses-permission android:name="android.permission.RECORD_AUDIO" />
    <uses-permission android:name="android.permission.MODIFY_AUDIO_SETTINGS" />
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
    <uses-permission android:name="android.permission.POST_NOTIFICATIONS" />
    <uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE" />
    <uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE" />
    <uses-permission android:name="android.permission.MANAGE_EXTERNAL_STORAGE" />

    <application
        android:label="ProTrack"
        android:hasCode="false"
        android:hardwareAccelerated="false"
        android:theme="@android:style/Theme.NoTitleBar.Fullscreen"
        android:requestLegacyExternalStorage="true">
        <activity
            android:name="android.app.NativeActivity"
            android:label="ProTrack"
            android:hardwareAccelerated="false"
            android:theme="@android:style/Theme.NoTitleBar.Fullscreen"
            android:screenOrientation="sensorLandscape"
            android:configChanges="orientation|keyboardHidden|screenSize"
            android:exported="true">
            <meta-data
                android:name="android.app.lib_name"
                android:value="prowave_android" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"@
$tmpManifest = "$pkgDir\AndroidManifest.xml"
Set-Content -Path $tmpManifest -Value $manifestContent -Encoding UTF8

# Copiar librerias nativas al staging
foreach ($t in $targetsToBuild) {
    $rustTarget = if ($t -eq "x86_64") { "x86_64-linux-android" } else { "aarch64-linux-android" }
    $soSrc = "$RootDir\target\$rustTarget\release\libprowave_android.so"
    $destDir = "$pkgDir\lib\$t"
    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
    Copy-Item $soSrc "$destDir\libprowave_android.so" -Force
}

# 3. Empaquetar con aapt y zipalign
Write-Host "`n[3/4] Empaquetando y alineando con zipalign..." -ForegroundColor Yellow
$unaligned = "$pkgDir\unaligned.apk"
$aligned = "$pkgDir\aligned.apk"
Remove-Item $unaligned, $aligned -ErrorAction SilentlyContinue

& "$buildTools\aapt.exe" package -F $unaligned -M $tmpManifest -I $platform -f
Push-Location $pkgDir
foreach ($t in $targetsToBuild) {
    & "$buildTools\aapt.exe" add "unaligned.apk" "lib/$t/libprowave_android.so"
}
Pop-Location
& "$buildTools\zipalign.exe" -f -p 4 $unaligned $aligned

# 4. Firmar con nova-release.keystore oficial
Write-Host "`n[4/4] Firmando con la clave oficial de produccion..." -ForegroundColor Yellow
$keystore = "C:\Users\grezu\Downloads\Landy_Proyectos\Apps\Ardour\ardour\nova_ui_qt\android\nova-release.keystore"
$distDir = "$RootDir\dist"
$qtApksDir = "C:\Users\grezu\Downloads\Landy_Proyectos\Apps\Ardour\ardour\nova_ui_qt\apks"
New-Item -ItemType Directory -Path $distDir, $qtApksDir -Force | Out-Null

$mainApk = "$distDir\NovaStudio.apk"
& "$buildTools\apksigner.bat" sign --ks $keystore --ks-pass pass:novastudio2026 --ks-key-alias novastudio --key-pass pass:novastudio2026 --out $mainApk $aligned

# Copiar copias especificas de distribucion
Copy-Item $mainApk "$distDir\NovaStudio-$Target.apk" -Force
Copy-Item $mainApk "$qtApksDir\NovaStudio.apk" -Force
Copy-Item $mainApk "$qtApksDir\NovaStudio-$Target.apk" -Force

$apkSize = (Get-Item $mainApk).Length / 1MB
Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "   ¡APK oficial de NOVA Studio generado y firmado con exito!" -ForegroundColor Green
Write-Host "   Archivo: $mainApk" -ForegroundColor Green
Write-Host "   Tamano: $apkSize MB" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
