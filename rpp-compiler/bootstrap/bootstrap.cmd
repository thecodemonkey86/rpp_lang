@echo off
rem Baut rppcc ohne Rust aus dem eingecheckten C-Startpunkt (Meilenstein M8), mit MSVC.
rem
rem   rpp-compiler\bootstrap\bootstrap.cmd [--check-seed]
rem
rem 1. Stufe 0: bootstrap\rppcc.c mit der C-Laufzeit uebersetzen.
rem 2. Stufe 0 uebersetzt den Compiler aus dem Quelltext (C-Code gen1, Stufe 1).
rem 3. Stufe 1 uebersetzt ihn noch einmal (gen2). Fixpunkt: gen1 und gen2 sind gleich.
rem Mit --check-seed muss ausserdem gen1 gleich dem Startpunkt sein (er ist aktuell).
rem Ergebnis: rpp-compiler\build\rppcc.exe. Ist cl nicht im Suchpfad, wird die
rem Umgebung von Visual Studio ueber vswhere geladen.
setlocal

set "here=%~dp0"
set "here=%here:~0,-1%"
for %%i in ("%here%\..") do set "compiler=%%~fi"
for %%i in ("%compiler%\..") do set "root=%%~fi"
set "runtime=%compiler%\runtime"
set "build=%compiler%\build"
set "flags=/nologo /std:c17 /utf-8 /O2"

set check_seed=0
if "%~1"=="--check-seed" set check_seed=1

where cl >nul 2>nul
if errorlevel 1 call :msvc
where cl >nul 2>nul
if errorlevel 1 (
    echo Cannot find cl.exe; run from a developer command prompt or install Visual Studio with the C++ tools. 1>&2
    exit /b 1
)

if not exist "%build%\stage0" mkdir "%build%\stage0"
if not exist "%build%\stage1" mkdir "%build%\stage1"

echo Stage 0: building the seed with cl
pushd "%build%\stage0"
cl %flags% /I "%runtime%" "%here%\rppcc.c" "%runtime%\rpp_rt.c" /Fe:rppcc.exe /link /STACK:16777216 >build.log
if errorlevel 1 (type build.log & popd & exit /b 1)
popd

echo Stage 1: the seed compiles rpp-compiler
"%build%\stage0\rppcc.exe" c --release "%root%\rppc\std" "%compiler%" "%build%\gen1.c"
if errorlevel 1 exit /b 1
pushd "%build%\stage1"
cl %flags% /I "%runtime%" "%build%\gen1.c" "%runtime%\rpp_rt.c" /Fe:rppcc.exe /link /STACK:16777216 >build.log
if errorlevel 1 (type build.log & popd & exit /b 1)
popd

echo Stage 2: stage 1 compiles rpp-compiler again
"%build%\stage1\rppcc.exe" c --release "%root%\rppc\std" "%compiler%" "%build%\gen2.c"
if errorlevel 1 exit /b 1

fc /b "%build%\gen1.c" "%build%\gen2.c" >nul
if errorlevel 1 (
    echo No fixpoint: stage 1 and stage 2 generated different C code 1>&2
    exit /b 1
)
echo Fixpoint reached: stage 1 and stage 2 generate the same C code.

copy /y "%build%\stage1\rppcc.exe" "%build%\rppcc.exe" >nul
fc /b "%here%\rppcc.c" "%build%\gen1.c" >nul
if errorlevel 1 (
    if "%check_seed%"=="1" (
        echo The seed bootstrap\rppcc.c is out of date; refresh it with 1>&2
        echo   rpp-compiler\build\rppcc.exe c --release rppc/std rpp-compiler rpp-compiler/bootstrap/rppcc.c 1>&2
        exit /b 1
    )
    echo Note: the seed differs from the current compiler ^(refresh bootstrap\rppcc.c^).
) else (
    echo The seed is up to date.
)
echo Built %build%\rppcc.exe
exit /b 0

:msvc
set "vswhere=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%vswhere%" exit /b 0
for /f "usebackq delims=" %%i in (`"%vswhere%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "vsdir=%%i"
if not defined vsdir exit /b 0
call "%vsdir%\VC\Auxiliary\Build\vcvars64.bat" >nul 2>nul
exit /b 0
