@echo off
rem Lance le port Rust jouable sous Windows (son par waveOut, manettes Xbox par XInput).
rem Prerequis : Rust pour Windows (https://rustup.rs, MSVC), et soit assets\ (produit par extract : jeu sans
rem aucune ROM), soit ddsom.zip et qsound.zip a la racine comme pour jouer.sh.
rem Depuis PowerShell : .\jouer.bat. Fonctionne aussi depuis le partage WSL (\\wsl.localhost\...) : pushd lui
rem donne une lettre de lecteur.
pushd "%~dp0"
where cargo >nul 2>nul || (echo Rust introuvable : installez-le depuis https://rustup.rs puis relancez. & goto fin)
if not defined CARGO_TARGET_DIR set CARGO_TARGET_DIR=%LOCALAPPDATA%\ddsom\target
if exist assets\data (set "JEU=--assets assets" & goto compile)
if not exist ddsom.zip (echo Ni assets\ ni ddsom.zip : copiez le dossier assets\ produit par extract a la racine du projet. & goto fin)
if not exist rom mkdir rom
if not exist rom\ddsom.key tar -xf ddsom.zip -C rom || goto fin
if not exist rom\dl-1425.bin (tar -xf qsound.zip -C rom || (echo qsound.zip introuvable a la racine du projet. & goto fin))
set "JEU=rom"
:compile
cargo build --release -q --manifest-path port\Cargo.toml -p ddsom && "%CARGO_TARGET_DIR%\release\ddsom.exe" %JEU% %*
:fin
popd
