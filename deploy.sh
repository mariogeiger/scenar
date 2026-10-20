#!/usr/bin/env bash
# Builds Scenar and publishes it where Caddy serves scenar.geiger.ink.
set -euo pipefail
cd "$(dirname "$0")"
cargo test --lib --quiet
trunk build --release
mkdir -p "$HOME/server/scenar"
rsync -a --delete dist/ "$HOME/server/scenar/"
