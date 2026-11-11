#!/usr/bin/env bash
# Instalador Windows compilado em Linux com MinGW (sem a CI do GitHub nem ferramentas da
# Microsoft). Requer: apt install mingw-w64 nsis; rustup target add x86_64-pc-windows-gnu.
# A libstdc++ fica embutida (ligação estática) e a WebView2Loader.dll vai no instalador
# (tauri.windows-gnu.conf.json). Saída: target/x86_64-pc-windows-gnu/release/bundle/nsis/.
set -euo pipefail
cd "$(dirname "$0")/.."
GCC_LIB="$(dirname "$(x86_64-w64-mingw32-g++-posix -print-file-name=libstdc++.a)")"
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
export CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc-posix
export CXXSTDLIB_x86_64_pc_windows_gnu="static=stdc++"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-L native=$GCC_LIB -L native=/usr/x86_64-w64-mingw32/lib -C link-arg=-static -C link-arg=-static-libgcc -C link-arg=-static-libstdc++"
export CARGO_PROFILE_RELEASE_DEBUG=0
pnpm install --frozen-lockfile
pnpm --filter @momento/desktop tauri build --target x86_64-pc-windows-gnu --bundles nsis --config src-tauri/tauri.windows-gnu.conf.json
# Só DLL do sistema e a WebView2Loader.dll (nada do MinGW):
x86_64-w64-mingw32-objdump -p target/x86_64-pc-windows-gnu/release/momento-desktop.exe | grep "DLL Name" | sort -u
