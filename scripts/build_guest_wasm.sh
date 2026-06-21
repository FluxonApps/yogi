#!/usr/bin/env bash
# Rebuild the real executor guest (wasm32) and refresh the committed artifact that being-sandbox-wasm
# embeds via include_bytes!. Run manually after editing guest/being-guest-wasm — NEVER part of the
# green-gate (cargo test --all builds host-only; this keeps wasm out of the loop).
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build --release --target wasm32-unknown-unknown --manifest-path guest/being-guest-wasm/Cargo.toml
cp guest/being-guest-wasm/target/wasm32-unknown-unknown/release/being_guest_wasm.wasm \
   crates/being-sandbox-wasm/guest.wasm
echo "refreshed crates/being-sandbox-wasm/guest.wasm ($(wc -c < crates/being-sandbox-wasm/guest.wasm) bytes)"
