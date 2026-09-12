@echo off
rem Build/test wrapper: sets up the MSVC environment then runs cargo in the project dir.
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
cd /d C:\Workspace\OpenRedSamurai\redsamurai-config
cargo %*
