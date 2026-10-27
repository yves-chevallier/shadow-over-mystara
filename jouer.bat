@echo off
rem Lance le port Rust jouable sous Windows (son par waveOut, manettes Xbox par XInput), sans ROM : depuis assets\.
rem Prerequis : Rust pour Windows (https://rustup.rs, MSVC). Si assets\ manque, il est produit une fois par
rem extract depuis ddsom.zip et qsound.zip (a la racine, comme pour jouer.sh) ; les zips et rom\ ne servent plus ensuite.
rem Depuis PowerShell : .\jouer.bat. Fonctionne aussi depuis le partage WSL (\\wsl.localhost\...) : pushd lui
rem donne une lettre de lecteur.
pushd "%~dp0"
where cargo >nul 2>nul || (echo Rust introuvable : installez-le depuis https://rustup.rs puis relancez. & goto fin)
if not defined CARGO_TARGET_DIR set CARGO_TARGET_DIR=%LOCALAPPDATA%\ddsom\target
rem le code 68000 recompile (ddsom-rc) ne sert qu'aux diagnostics : sans ROM pour lui, la compilation est bien plus courte
set DDSOM_ROMS=%CARGO_TARGET_DIR%\sans-rom
if exist assets\data\natives.json goto jeu
if not exist ddsom.zip (echo Ni assets\ ni ddsom.zip : copiez ddsom.zip et qsound.zip a la racine du projet. & goto fin)
if not exist qsound.zip (echo qsound.zip manque a la racine du projet ^(programme de la puce son, dl-1425.bin^). & goto fin)
if not exist rom mkdir rom
if not exist rom\ddsom.key tar -xf ddsom.zip -C rom || goto fin
if not exist rom\dl-1425.bin tar -xf qsound.zip -C rom || goto fin
echo Compilation de l'extracteur puis extraction des ressources dans assets\ (une seule fois, quelques minutes)...
cargo run --release --manifest-path port\Cargo.toml -p extract -- rom assets >nul || goto fin
:jeu
echo Compilation du jeu (longue la premiere fois)...
cargo build --release --manifest-path port\Cargo.toml -p ddsom && "%CARGO_TARGET_DIR%\release\ddsom.exe" --assets assets %*
:fin
popd
