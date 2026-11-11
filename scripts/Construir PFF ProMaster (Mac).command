#!/bin/bash
# PFF ProMaster — construir a aplicação neste Mac (duplo clique).
# Instala só o que faltar (ferramentas da Apple, Rust, Node.js) e gera o .dmg.
# Nada é enviado para fora: tudo corre neste computador.
set -e
cd "$(dirname "$0")/.."
clear
echo "=============================================="
echo "  PFF ProMaster — construir para este Mac"
echo "=============================================="
echo

fim() { echo; read -n1 -r -p "Prima uma tecla para fechar esta janela." _; exit "${1:-0}"; }
trap 'echo; echo "Ocorreu um erro (ver acima)."; fim 1' ERR

# 1. Ferramentas da Apple (compilador, git, python3)
if ! xcode-select -p >/dev/null 2>&1; then
  echo "1/4 É preciso instalar as ferramentas da Apple (Xcode Command Line Tools)."
  echo "    Vai abrir uma janela do macOS: carregue em 'Instalar'."
  xcode-select --install >/dev/null 2>&1 || true
  echo "    Quando a instalação terminar, volte a abrir este ficheiro."
  fim 0
fi
echo "1/4 Ferramentas da Apple: OK"

# 2. Rust (a versão exata é instalada sozinha a partir de rust-toolchain.toml)
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
if ! command -v cargo >/dev/null 2>&1; then
  echo "2/4 A instalar o Rust (rustup.rs)…"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --no-modify-path
  source "$HOME/.cargo/env"
fi
echo "2/4 Rust: OK"

# 3. Node.js 22 + pnpm (numa pasta própria, sem mexer no resto do sistema)
TOOLS="$HOME/.pff-promaster-ferramentas"
NODE_V="v22.12.0"
if [ "$(uname -m)" = "arm64" ]; then NA=arm64; else NA=x64; fi
export PATH="$TOOLS/node-$NODE_V-darwin-$NA/bin:$TOOLS/bin:$PATH"
if ! command -v node >/dev/null 2>&1 || [ "$(node -p 'process.versions.node.split(".")[0]')" -lt 22 ]; then
  echo "3/4 A instalar o Node.js $NODE_V…"
  mkdir -p "$TOOLS"
  curl -sSfL "https://nodejs.org/dist/$NODE_V/node-$NODE_V-darwin-$NA.tar.gz" | tar -xz -C "$TOOLS"
fi
if ! command -v pnpm >/dev/null 2>&1 || [ "$(pnpm --version)" != "10.28.0" ]; then
  npm install -g --silent --prefix "$TOOLS" pnpm@10.28.0
fi
echo "3/4 Node.js $(node --version) e pnpm $(pnpm --version): OK"

# 4. Compilar (10–25 minutos na primeira vez)
echo "4/4 A compilar o PFF ProMaster (demora 10–25 minutos na primeira vez)…"
./scripts/build-release.sh
DMG=$(ls -t target/release/bundle/dmg/*.dmg | head -1)
cp "$DMG" "$HOME/Desktop/"
echo
echo "PRONTO: $(basename "$DMG") foi copiado para a Secretária."
open -R "$HOME/Desktop/$(basename "$DMG")"
fim 0
