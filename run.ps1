# Script de ejecución de Nova para PowerShell en Windows
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir

# Asegurar Cargo en el PATH
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path (Join-Path $CargoBin "cargo.exe")) {
        $env:PATH = "$CargoBin;$env:PATH"
    } else {
        Write-Error "No se encontró Cargo/Rust en el sistema. Instálalo desde https://rustup.rs/"
        exit 1
    }
}

# Comprobación de upstream si Python está disponible
$CheckScript = Join-Path $ScriptDir "scripts\check_upstream.py"
if (Test-Path $CheckScript) {
    $PythonCmd = if (Get-Command py -ErrorAction SilentlyContinue) { "py" }
                 elseif (Get-Command python -ErrorAction SilentlyContinue) { "python" }
                 elseif (Get-Command python3 -ErrorAction SilentlyContinue) { "python3" }
                 else { $null }

    if ($PythonCmd) {
        try {
            $CheckOutput = & $PythonCmd $CheckScript 2>$null
            if ($CheckOutput -match "UPDATE_FOUND") {
                Write-Host "======================================================================" -ForegroundColor Yellow
                Write-Host "🔔 ¡Novedades detectadas en los repositorios de SoundCraft o Pro-Wave!" -ForegroundColor Yellow
                Write-Host "======================================================================" -ForegroundColor Yellow
            }
        } catch {}
    }
}

# Ejecutar Nova
Write-Host "Iniciando Nova (Pro Wave)..." -ForegroundColor Cyan
if ($args.Count -gt 0) {
    cargo run --bin prowave -- @args
} else {
    cargo run --bin prowave
}
