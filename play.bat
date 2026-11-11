@echo off
rem Launches the playable Rust port on Windows (sound through waveOut, Xbox pads through XInput), without ROM: from assets\.
rem Requirement: Rust for Windows (https://rustup.rs, MSVC). If assets\ is missing, it is produced once by
rem extract from ddsom.zip and qsound.zip (at the root, as for play.sh); the zips and rom\ are not used afterwards.
rem An assets\ from an older version (without data\overreads.json) is re-extracted the same way; after another
rem update of the extractor, delete assets\ to force the re-extraction (docs/sans-rom.md).
rem From PowerShell: .\play.bat. Also works from the WSL share (\\wsl.localhost\...): pushd gives it
rem a drive letter.
pushd "%~dp0"
where cargo >nul 2>nul || (echo Rust not found: install it from https://rustup.rs then run again. & goto end)
if not defined CARGO_TARGET_DIR set CARGO_TARGET_DIR=%LOCALAPPDATA%\ddsom\target
rem the recompiled 68000 code (ddsom-rc) only serves diagnostics: without ROM for it, the build is much shorter
set DDSOM_ROMS=%CARGO_TARGET_DIR%\sans-rom
if exist assets\data\overreads.json goto game
if exist assets echo assets\ is from an older version (data\overreads.json is missing): new extraction.
if not exist rom\ddsom.key if not exist ddsom.zip (echo Neither an up-to-date assets\ nor ddsom.zip: copy ddsom.zip and qsound.zip to the project root. & goto end)
if not exist rom\dl-1425.bin if not exist qsound.zip (echo qsound.zip is missing at the project root ^(program of the sound chip, dl-1425.bin^). & goto end)
if not exist rom mkdir rom
if not exist rom\ddsom.key tar -xf ddsom.zip -C rom || goto end
if not exist rom\dl-1425.bin tar -xf qsound.zip -C rom || goto end
echo Building the extractor then extracting the resources into assets\ (only once, a few minutes)...
cargo run --release --manifest-path port\Cargo.toml -p extract -- rom assets >nul || goto end
:game
echo Building the game (long the first time)...
cargo build --release --manifest-path port\Cargo.toml -p ddsom && "%CARGO_TARGET_DIR%\release\ddsom.exe" --assets assets %*
:end
popd
