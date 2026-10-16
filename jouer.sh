#!/usr/bin/env bash
# Lance le port Rust jouable de Shadow over Mystara.
# Prérequis : rom/ contient les fichiers de ddsom.zip et dl-1425.bin (qsound.zip).
set -euo pipefail
cd "$(dirname "$0")"
[ -f rom/ddsom.key ] || unzip -oq ddsom.zip -d rom
[ -f rom/dl-1425.bin ] || unzip -oq qsound.zip -d rom
cargo build --release -q --manifest-path port/Cargo.toml -p ddsom
exec ./port/target/release/ddsom rom
