#!/usr/bin/env bash
# Build the static site: compile standuser-core to WebAssembly and copy the film table and the name candidates next to the page.
# Needs: rustup with the wasm32-unknown-unknown target, and wasm-pack (cargo install wasm-pack).
set -euo pipefail
cd "$(dirname "$0")/.."
# keep build-machine paths (home directory, user name) out of the committed binary
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME/.cargo=/cargo --remap-path-prefix=$HOME/.rustup=/rustup --remap-path-prefix=$PWD=/src"
wasm-pack build crates/standuser-wasm --target web --release --out-dir ../../site/pkg --out-name standuser
rm -f site/pkg/.gitignore site/pkg/package.json site/pkg/*.d.ts   # the repo commits site/pkg so Cloudflare Pages needs no build step
cp crates/standuser-core/data/films.json site/films.json
cp crates/standuser-core/data/names.json site/names.json
ls -la site site/pkg
