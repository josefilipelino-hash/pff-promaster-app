#!/usr/bin/env bash
# Build de produção reproduzível do PFF ProMaster (Fase 16).
#
#   ./scripts/build-release.sh            # macOS → .app + .dmg ; Linux → .deb + AppImage
#   ./scripts/build-release.sh --universal  # macOS: binário universal (Apple Silicon + Intel)
#
# Passos (falha no primeiro erro):
#   1. Verifica ferramentas e versões fixadas (rust-toolchain.toml, pnpm-lock, Cargo.lock).
#   2. Instala dependências JS exatamente como no lock (--frozen-lockfile).
#   3. Verifica os modelos de IA contra models/models.lock.json (SHA-256) — sem modelos
#      certos não há build.
#   4. Compila em modo release (sem source maps, sem debug info) com SOURCE_DATE_EPOCH do
#      commit e gera os pacotes com o Tauri.
#   5. Escreve BUILD_INFO.txt e SHA256SUMS.txt ao lado dos pacotes.
#
# Assinatura/notarização: opcionais, por variáveis de ambiente (ver docs/SIGNING.md).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

UNIVERSAL=0
for a in "$@"; do
  case "$a" in
    --universal) UNIVERSAL=1 ;;
    *) echo "opção desconhecida: $a" >&2; exit 2 ;;
  esac
done

OS="$(uname -s)"
say() { printf '\n==> %s\n' "$*"; }
need() { command -v "$1" >/dev/null 2>&1 || { echo "Falta: $1 ($2)" >&2; exit 1; }; }

say "Ferramentas"
need cargo "https://rustup.rs"
need node "Node.js 22 LTS"
need pnpm "corepack enable && corepack prepare pnpm@10.28.0 --activate"
need python3 "Python 3 (verificação dos modelos)"
if [ "$OS" = "Darwin" ]; then
  xcode-select -p >/dev/null 2>&1 || { echo "Falta: Xcode Command Line Tools (xcode-select --install)" >&2; exit 1; }
fi
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
[ "$NODE_MAJOR" -ge 20 ] || { echo "Node $NODE_MAJOR: é preciso Node 20 ou superior (22 recomendado)" >&2; exit 1; }
echo "rustc  $(rustc --version)   (fixado em rust-toolchain.toml)"
echo "node   $(node --version)"
echo "pnpm   $(pnpm --version)"

say "Código-fonte"
git submodule update --init --recursive
COMMIT="$(git rev-parse --short HEAD)"
DIRTY=""
git diff --quiet && git diff --cached --quiet || DIRTY="-alterado"
export SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
echo "versão $VERSION · commit $COMMIT$DIRTY · SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH"

say "Dependências JavaScript (lock exato)"
pnpm install --frozen-lockfile

say "Modelos de IA (SHA-256 contra models/models.lock.json)"
python3 - <<'PY'
import hashlib, json, os, sys
lock = json.load(open("models/models.lock.json"))["models"]
bad = []
for m in lock:
    f = os.path.join("models", m["file"])
    if not os.path.isfile(f):
        bad.append(f"{m['file']}: em falta")
        continue
    h = hashlib.sha256(open(f, "rb").read()).hexdigest()
    if h != m["sha256"]:
        bad.append(f"{m['file']}: SHA-256 diferente")
if bad:
    print("\n".join(bad))
    sys.exit("Modelos em falta ou diferentes: corra `python3 scripts/prepare-models.py` (ver models/README.md).")
print(f"{len(lock)} modelos verificados ({sum(m['bytes'] for m in lock) / 1e6:.0f} MB)")
PY

say "Compilação de produção"
export CARGO_PROFILE_RELEASE_DEBUG=0   # sem debug info (os nomes de funções ficam, para os crash logs)
ARGS=()
if [ "$OS" = "Darwin" ]; then
  ARGS+=(--bundles app,dmg)
  if [ "$UNIVERSAL" = 1 ]; then
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
    ARGS+=(--target universal-apple-darwin)
  fi
elif [ "$OS" = "Linux" ]; then
  ARGS+=(--bundles deb,appimage)
else
  echo "Sistema não suportado por este script: $OS (no Windows: pnpm --filter @momento/desktop tauri build)" >&2
  exit 1
fi
pnpm --filter @momento/desktop tauri build "${ARGS[@]}"

say "Artefactos"
if command -v shasum >/dev/null 2>&1; then SUM="shasum -a 256"; else SUM="sha256sum"; fi
if [ "$OS" = "Darwin" ] && [ "$UNIVERSAL" = 1 ]; then
  BUNDLE="target/universal-apple-darwin/release/bundle"
else
  BUNDLE="target/release/bundle"
fi
{
  echo "PFF ProMaster $VERSION"
  echo "commit: $COMMIT$DIRTY"
  echo "data do commit (SOURCE_DATE_EPOCH): $SOURCE_DATE_EPOCH"
  echo "sistema: $OS $(uname -m)"
  echo "rustc: $(rustc --version)"
  echo "node: $(node --version) · pnpm: $(pnpm --version)"
  [ "$OS" = "Darwin" ] && echo "macOS SDK: $(xcrun --show-sdk-version 2>/dev/null || echo '?')"
} > "$BUNDLE/BUILD_INFO.txt"
(
  cd "$BUNDLE"
  find . -maxdepth 2 -type f \( -name '*.dmg' -o -name '*.deb' -o -name '*.AppImage' \) -print0 \
    | sort -z | xargs -0 -r $SUM > SHA256SUMS.txt
)
cat "$BUNDLE/BUILD_INFO.txt"
echo
cat "$BUNDLE/SHA256SUMS.txt"
echo
echo "Pacotes em: $ROOT/$BUNDLE"
