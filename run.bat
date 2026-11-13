@echo off
setlocal enabledelayedexpansion

REM Cambiar al directorio del script
cd /d "%~dp0"

REM Asegurar que Cargo este en el PATH si no esta disponible directamente
where cargo >nul 2>&1
if %ERRORLEVEL% neq 0 (
    if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
        set "PATH=%USERPROFILE%\.cargo\bin;!PATH!"
    ) else (
        echo [ERROR] No se encontro Cargo o Rust instalado en el sistema.
        echo Por favor instala Rust desde https://rustup.rs/ o agrega Cargo al PATH.
        pause
        exit /b 1
    )
)

REM Comprobacion opcional de upstream con Python (si esta instalado)
if exist "scripts\check_upstream.py" (
    set "PYTHON_EXE="
    where py >nul 2>&1 && set "PYTHON_EXE=py"
    if not defined PYTHON_EXE (
        where python >nul 2>&1 && set "PYTHON_EXE=python"
    )
    if not defined PYTHON_EXE (
        where python3 >nul 2>&1 && set "PYTHON_EXE=python3"
    )

    if defined PYTHON_EXE (
        for /f "delims=" %%i in ('!PYTHON_EXE! scripts\check_upstream.py 2^>nul') do (
            echo %%i | findstr /c:"UPDATE_FOUND" >nul && (
                echo ======================================================================
                echo [i] Novedades detectadas en los repositorios de SoundCraft o Pro-Wave!
                echo ======================================================================
            )
        )
    )
)

echo Iniciando Nova (Pro Wave)...
if "%~1"=="" (
    cargo run --bin prowave
) else (
    cargo run --bin prowave -- %*
)
if %ERRORLEVEL% neq 0 (
    echo.
    echo Ocurrio un error al ejecutar Nova.
    pause
)
