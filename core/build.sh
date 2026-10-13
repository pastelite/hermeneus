#!/bin/sh
set -eu

cd "$(dirname "$0")"
wasm-pack build --target bundler --out-dir ../apps/web/pkg
