@echo off
setlocal
set "SELF_DIR=%~dp0"
set "JAVA=%SELF_DIR%runtime\bin\java.exe"
set "LIB=%SELF_DIR%lib"
set "CFG=%SELF_DIR%xrechnung-config"

if /I "%~1"=="validator" (
    shift
    "%JAVA%" -jar "%LIB%\kosit-validator.jar" -r "%CFG%" %*
    exit /b %ERRORLEVEL%
)
if /I "%~1"=="mustang-zugferd" (
    shift
    "%JAVA%" -jar "%LIB%\mustang.jar" %*
    exit /b %ERRORLEVEL%
)

echo Klein.Buch Java-Sidecar
echo Versionen: KoSIT 1.6.2 + XRechnung-Config 2026-01-31 + Mustang 2.23.0
echo.
echo Usage: %~n0 ^<subcommand^> [args...]
echo.
echo Subcommands:
echo   validator         KoSIT XRechnung/ZUGFeRD-Validator
echo   mustang-zugferd   Mustang ZUGFeRD/PDF-A-3 Generator + Parser
exit /b 1
