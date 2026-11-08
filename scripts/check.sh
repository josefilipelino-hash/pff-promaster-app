#!/usr/bin/env bash
# Verificação completa executada antes de cada commit e no CI.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== TypeScript: tipos =="
pnpm -r typecheck
echo "== TypeScript: testes =="
pnpm -r test
echo "== Rust: formatação =="
cargo fmt --all --check
echo "== Rust: clippy =="
cargo clippy --workspace --all-targets -- -D warnings
echo "== Rust: testes =="
cargo test --workspace
echo "OK: todas as verificações passaram."
