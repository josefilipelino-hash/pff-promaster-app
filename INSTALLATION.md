# Instalação

Plataformas-alvo: **macOS 12+** (Apple Silicon e Intel) e **Windows 10/11 (64 bits)**.
Ainda não existe um instalador assinado: executa-se a partir do código-fonte ou a partir
dos instaladores gerados pelo CI (ver no fim).

## macOS — preparação (uma vez)

```bash
xcode-select --install
```

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

```bash
brew install node@22
```

```bash
corepack enable
```

## Windows 10/11 — preparação (uma vez)

1. **Visual Studio Build Tools 2022** com a carga de trabalho “Desenvolvimento para
   ambiente de trabalho com C++” (inclui o SDK do Windows). Necessário para compilar o Rust
   no Windows: <https://visualstudio.microsoft.com/visual-cpp-build-tools/>.
2. **WebView2**: já incluído no Windows 11 e nas versões atualizadas do Windows 10. Se
   faltar: <https://developer.microsoft.com/microsoft-edge/webview2/>.
3. **Rust**: instalar com `rustup-init.exe` a partir de <https://rustup.rs> (escolher a
   opção predefinida, alvo `x86_64-pc-windows-msvc`).
4. **Node.js 22 LTS**: <https://nodejs.org>.
5. Numa janela do PowerShell:

```powershell
corepack enable
```

## Obter e executar (macOS e Windows)

```bash
git clone --recurse-submodules <URL-do-repositório> momento-studio
```

A LibRaw (descodificação RAW) é um submódulo Git. Se o repositório já tiver sido clonado
sem `--recurse-submodules`:

```bash
git submodule update --init --recursive
```

```bash
cd momento-studio
```

```bash
pnpm install
```

```bash
pnpm dev
```

A primeira execução compila todas as dependências Rust e a LibRaw em C++ (alguns minutos;
o compilador C++ vem com as Xcode Command Line Tools no macOS e com as Build Tools no
Windows). Abre a janela
“Momento Studio” no ecrã inicial.

## Criar a aplicação instalável

```bash
pnpm build
```

Resultado em `target/release/bundle/`:
- macOS: `macos/Momento Studio.app` e `dmg/Momento Studio_0.1.0_aarch64.dmg`
- Windows: `nsis/Momento Studio_0.1.0_x64-setup.exe`

Binário universal para macOS (Apple Silicon + Intel):

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

```bash
pnpm --filter @momento/desktop tauri build --target universal-apple-darwin
```

### Aplicação não assinada

- **macOS**: na primeira abertura, clique direito → **Abrir** → **Abrir**. Em macOS 15+, se
  não aparecer a opção: **Definições do Sistema → Privacidade e segurança → Abrir mesmo
  assim**.
- **Windows**: o SmartScreen pode mostrar “O Windows protegeu o computador” → **Mais
  informações** → **Executar mesmo assim**.

### Instaladores gerados pelo CI

Quando o repositório estiver no GitHub, cada envio para `main` gera (após os testes) os
artefactos `momento-studio-macOS` (`.dmg` universal) e `momento-studio-Windows` (`.exe`
NSIS), descarregáveis na página da execução em **Actions**.

## Linux (desenvolvimento e CI)

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev pkg-config build-essential
```

## Onde ficam os dados

| O quê | Onde |
|---|---|
| Projeto (catálogo, cache, cópias de segurança) | `<pasta de trabalho>/<AAAA-MM-DD_Cliente>.momento/` — **disco local**, interno ou externo, nunca numa partilha de rede |
| Originais | Onde já estavam (modo “manter”) ou na pasta dos originais do projeto (modo “copiar”) |
| Lista de projetos recentes | macOS: `~/Library/Application Support/pt.momentostudio.desktop/library.db` · Windows: `%APPDATA%\pt.momentostudio.desktop\library.db` |
| Registos (logs) | macOS: `~/Library/Logs/pt.momentostudio.desktop/` · Windows: `%LOCALAPPDATA%\pt.momentostudio.desktop\logs\` |

Apagar a pasta `cache/` de um projeto é seguro: as miniaturas são regeneradas.

## Resolução de problemas

| Sintoma | Causa provável | Solução |
|---|---|---|
| `ERR_PNPM_NO_MATURE_MATCHING_VERSION` | Proteção `minimumReleaseAge` (7 dias) | Usar uma versão mais antiga do pacote; não desativar a proteção |
| `Port 1420 is already in use` | Outro `pnpm dev` aberto | Fechar o processo anterior |
| Janela branca em `pnpm dev` | Vite ainda não arrancou | Aguardar; ver o terminal |
| `xcrun: error` (macOS) | Faltam as Command Line Tools | `xcode-select --install` |
| `link.exe not found` (Windows) | Faltam as Build Tools de C++ | Instalar o ponto 1 da preparação Windows |
| `Código-fonte da LibRaw em falta` | Submódulo não descarregado | `git submodule update --init --recursive` |
| HEIC/AVIF aparecem como “codec em falta” no Windows | Extensões da Microsoft Store em falta | Instalar “HEIF Image Extensions”, “HEVC Video Extensions” (HEIC) e “AV1 Video Extension” (AVIF) |
| “Pasta não encontrada” num projeto recente | Disco externo desligado ou projeto movido | Ligar o disco, ou “Abrir projeto…” e escolher a pasta `.momento` |
