# Instalação

Plataformas-alvo: **macOS 12+** (Apple Silicon e Intel) e **Windows 10/11 (64 bits)**.
Versão atual: **1.0.0-personal** — aplicação instalável para uso pessoal, compilada
localmente com `scripts/build-release.sh` (sem assinatura Apple; ver
"Versão 1.0.0-personal no Mac").

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

## Versão 1.0.0-personal no Mac (uso pessoal)

### Requisitos

| | |
|---|---|
| Sistema | macOS 12 Monterey ou mais recente; Apple Silicon (recomendado) ou Intel |
| Memória | 8 GB mínimo; 16 GB recomendado para casamentos de 5 000+ fotografias com análise |
| Disco | ~200 MB para a aplicação; projetos num disco **local** (interno ou externo USB/Thunderbolt) — nunca numa partilha de rede |
| Para compilar | Xcode Command Line Tools, Rust (instalado pelo `rustup`, versão fixada no repositório), Node.js 22, pnpm 10, Python 3, ~15 GB livres durante a compilação |

### 1. Compilar (uma vez por versão)

Com a preparação do macOS acima feita e os modelos de IA em `models/` (ver
`models/README.md`):

```bash
./scripts/build-release.sh
```

O script verifica as ferramentas, instala as dependências exatamente como nos locks,
confirma os 9 modelos de IA pelo SHA-256 e gera:

- `target/release/bundle/macos/Momento Studio.app`
- `target/release/bundle/dmg/Momento Studio_1.0.0_aarch64.dmg` (ou `_x64` num Mac Intel)
- `target/release/bundle/BUILD_INFO.txt` (commit, ferramentas) e `SHA256SUMS.txt`

Para um único `.dmg` que funcione em Apple Silicon e Intel: `./scripts/build-release.sh --universal`
(resultado em `target/universal-apple-darwin/release/bundle/`).

### 2. Instalar

1. Abrir o `.dmg` e arrastar **Momento Studio** para **Aplicações**.
2. **Primeira abertura** (aplicação não assinada): clique direito na aplicação → **Abrir** →
   **Abrir**. Em macOS 15+, se não aparecer a opção: **Definições do Sistema → Privacidade e
   segurança → Abrir mesmo assim**. Só é preciso uma vez.
   Alternativa no Terminal: `xattr -dr com.apple.quarantine "/Applications/Momento Studio.app"`
   (só necessário se o `.dmg` tiver sido descarregado da internet).

### 3. Permissões de acesso

A aplicação não usa sandbox: lê e escreve onde o utilizador escolher. O macOS pede
autorização uma vez, com uma explicação, quando se acede pela primeira vez a:

| Local | Pedido do macOS |
|---|---|
| Cartões de memória e discos externos | "Aceder a ficheiros num volume amovível" |
| Secretária, Documentos, Transferências | "Aceder a ficheiros na pasta …" |
| Pastas de rede | "Aceder a ficheiros num volume de rede" |
| `~/Imagens` (Pictures) e outras pastas escolhidas no diálogo | Sem pedido (não são pastas protegidas) |

Se uma autorização for recusada por engano: **Definições do Sistema → Privacidade e
segurança → Ficheiros e pastas → Momento Studio**. A aplicação não acede à fototeca do
Fotos.

### 4. Atualizar para uma versão nova

1. Fechar a aplicação (os projetos ficam gravados — não há "Guardar").
2. Instalar a versão nova por cima (arrastar para Aplicações → **Substituir**).
3. Abrir os projetos normalmente. Se a versão nova mudar o formato do catálogo, **antes** de
   o alterar é guardada uma cópia completa em
   `<projeto>.momento/backups/catalog-antes-da-atualizacao-v<N>-<data>.db` (visível em
   **Segurança e cópias → Versões**). Sem conseguir fazer essa cópia, o catálogo não é
   alterado.
4. Voltar a uma versão anterior: reinstalar a versão antiga e, em **Segurança e cópias →
   Versões**, restaurar a cópia "antes da atualização". Uma versão antiga recusa abrir um
   catálogo de uma versão mais recente (nunca o estraga).

As preferências e a lista de projetos recentes (`~/Library/Application Support/
pt.momentostudio.desktop/`) passam de versão para versão; o identificador
`pt.momentostudio.desktop` não muda.

### 5. Backup

| O quê | Como |
|---|---|
| **Originais** | Na importação: **Copiar também para um disco de segurança** + **Verificar cada cópia**. Nunca formatar o cartão antes de a importação acabar sem erros |
| **Catálogo** (classificações, edições, momentos, álbuns) | Automático: versões a cada 15/30/60/240 min (60 por omissão), ao abrir e ao fechar, em `<projeto>.momento/backups/`. Manual: **Segurança e cópias → Criar versão** ou **Exportar catálogo** (ficheiro único para outro disco) |
| **Projeto completo** | Copiar a pasta `<AAAA-MM-DD_Cliente>.momento` (com a aplicação fechada) para outro disco; a pasta `cache/` pode ser omitida (é regenerada) |
| Time Machine | Complemento útil (não testado pela aplicação): uma cópia feita com o projeto aberto pode apanhar o catálogo a meio de uma escrita — as versões em `backups/` são sempre consistentes e são as que se restauram |

### 6. Desinstalar

Arrastar **Momento Studio** de Aplicações para o Lixo. Os projetos e originais **não** são
tocados. Para apagar também as preferências e os registos:
`~/Library/Application Support/pt.momentostudio.desktop/` e
`~/Library/Logs/pt.momentostudio.desktop/`.

### Assinatura e notarização

Não são necessárias para uso pessoal. Passos para quando houver conta Apple Developer:
[`docs/SIGNING.md`](docs/SIGNING.md).

### Instaladores gerados pelo CI

Com o repositório no GitHub, cada tag `v*` (ex.: `v1.0.0-personal`) gera os artefactos
`momento-studio-macOS` (`.dmg` universal, `BUILD_INFO.txt`, `SHA256SUMS.txt`) e
`momento-studio-Windows` em **Actions**.

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
| Registos (logs, 14 dias) e relatórios de falhas | macOS: `~/Library/Logs/pt.momentostudio.desktop/` · Windows: `%LOCALAPPDATA%\pt.momentostudio.desktop\logs\` |

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
