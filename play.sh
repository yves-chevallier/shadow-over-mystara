#!/usr/bin/env bash
# Launches the playable Rust port of Shadow over Mystara.
# Requirement: rom/ holds the files of ddsom.zip and dl-1425.bin (qsound.zip); building needs libudev-dev
# (gamepads).
set -euo pipefail
cd "$(dirname "$0")"
[ -f rom/ddsom.key ] || unzip -oq ddsom.zip -d rom
[ -f rom/dl-1425.bin ] || unzip -oq qsound.zip -d rom
cargo build --release -q --manifest-path port/Cargo.toml -p ddsom
exec ./port/target/release/ddsom rom
