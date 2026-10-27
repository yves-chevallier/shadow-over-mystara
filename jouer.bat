@echo off
rem Lance le port Rust jouable sous Windows (son par waveOut, manettes Xbox par XInput).
rem Prerequis : Rust pour Windows (rustup, MSVC) et rom\ prepare comme pour jouer.sh.
rem Fonctionne aussi depuis le partage WSL (\\wsl.localhost\...) : pushd lui donne une lettre de lecteur.
pushd "%~dp0"
if not defined CARGO_TARGET_DIR set CARGO_TARGET_DIR=%LOCALAPPDATA%\ddsom\target
cargo build --release -q --manifest-path port\Cargo.toml -p ddsom && "%CARGO_TARGET_DIR%\release\ddsom.exe" rom %*
popd
