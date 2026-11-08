# Instalação

> Fase 0: a aplicação mostra apenas um ecrã de diagnóstico. Ainda não existe instalador
> distribuído; executa-se a partir do código-fonte.

## Requisitos

| Componente | Versão | Notas |
|---|---|---|
| macOS | 12 Monterey ou superior | Apple Silicon (prioritário) ou Intel |
| Xcode Command Line Tools | atual | compilador C/C++ e SDK do macOS |
| Rust | estável ≥ 1.85 (testado com 1.97) | via rustup; `rust-toolchain.toml` escolhe o canal |
| Node.js | ≥ 22 | testado com 22.23 |
| pnpm | 10.28 | ativado via Corepack (versão fixada em `package.json`) |
| Espaço em disco | ~4 GB | dependências e `target/` do Rust |

Windows 10/11: previsto para a Fase 9 (requer WebView2 e Visual Studio Build Tools).
Linux: usado para desenvolvimento e CI (requer `libwebkit2gtk-4.1-dev`, ver abaixo).

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

Opcional, para gerar um binário universal (Apple Silicon + Intel):

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

## Obter e executar

```bash
git clone <URL-do-repositório> momento-studio
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

A primeira execução compila todas as dependências Rust (alguns minutos). Abre uma janela
“Momento Studio” com o diagnóstico: versão da aplicação, do núcleo, do esquema do catálogo
e plataforma (ex.: `macos / aarch64`).

## Criar a aplicação (.app / .dmg)

```bash
pnpm build
```

Resultado em `target/release/bundle/` (`macos/Momento Studio.app` e `dmg/`).

Sem conta Apple Developer a aplicação não é assinada nem notarizada. Na primeira abertura:
clique direito na aplicação → **Abrir** → **Abrir**. Em macOS 15+, se não aparecer a
opção: **Definições do Sistema → Privacidade e segurança → Abrir mesmo assim**.

## Linux (desenvolvimento/CI)

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev pkg-config build-essential
```

Depois, os mesmos passos `pnpm install` e `pnpm dev`.

## Dados da aplicação

A partir da Fase 1: cada projeto fica numa pasta `<nome>.momento/` escolhida pelo
utilizador (**tem de estar num disco local**, interno ou externo; não numa partilha de
rede). Preferências e lista de projetos recentes ficam em
`~/Library/Application Support/pt.momentostudio.desktop/`.

## Resolução de problemas

| Sintoma | Causa provável | Solução |
|---|---|---|
| `ERR_PNPM_NO_MATURE_MATCHING_VERSION` | Proteção `minimumReleaseAge` (7 dias) | Usar uma versão mais antiga do pacote; não desativar a proteção |
| `Port 1420 is already in use` | Outro `pnpm dev` aberto | Fechar o processo anterior |
| Janela branca em `pnpm dev` | Vite ainda não arrancou | Aguardar; ver o terminal |
| Erro `xcrun: error` | Faltam as Command Line Tools | `xcode-select --install` |
