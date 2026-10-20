#!/usr/bin/env bash
# Builds Scenar, publishes the page where Caddy serves scenar.geiger.ink,
# and restarts the server of shared stories.
set -euo pipefail
cd "$(dirname "$0")"
cargo test --workspace --quiet
trunk build --release
cargo build --release -p scenar-server
mkdir -p "$HOME/server/scenar" "$HOME/server/data/scenar"
rsync -a --delete dist/ "$HOME/server/scenar/"
install -D target/release/scenar-server "$HOME/server/apps/scenar/scenar-server"
systemctl --user restart geiger-scenar
