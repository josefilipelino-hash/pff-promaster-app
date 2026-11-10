# Lista de verificação de lançamento

Antes de cada versão (e antes de usar uma compilação nova num trabalho real). Cada linha
tem o comando ou o passo exato; não se assinala sem o resultado à vista.

## 1. Código e testes automáticos

- [ ] Árvore limpa e versão igual em `Cargo.toml`, `package.json`,
      `apps/desktop/package.json` e `apps/desktop/src-tauri/tauri.conf.json`.
- [ ] Formatação e lints:

```bash
cargo fmt --all --check
```

```bash
cargo clippy --release --workspace --all-targets -- -D warnings
```

```bash
pnpm --filter @momento/desktop exec tsc --noEmit
```

- [ ] Testes da interface (inclui auditoria de acessibilidade):

```bash
pnpm -r test
```

- [ ] Testes do núcleo **com material real** (RAW de várias marcas, modelos, rostos):

```bash
MOMENTO_SAMPLES_DIR=<amostras> MOMENTO_REF_DIR=<referências> MOMENTO_FACES_DIR=<rostos> cargo test --release --workspace --no-fail-fast
```

- [ ] Casamento de ponta a ponta (cartão → álbum, com reinício a meio) passou:
      `cargo test --release -p momento-project-management --test wedding_e2e -- --nocapture`
- [ ] Falhas deliberadas (Fase 13) passaram: `--test resilience`.
- [ ] Condições adversas com discos reais (Linux, root):

```bash
mkdir -p /tmp/fs/cheio /tmp/fs/ro && mount -t tmpfs -o size=6m tmpfs /tmp/fs/cheio && mount -t tmpfs -o size=8m tmpfs /tmp/fs/ro
```

  copiar 3 JPEG para `/tmp/fs/ro`, `mount -o remount,ro /tmp/fs/ro`, e:

```bash
MOMENTO_FULL_FS=/tmp/fs/cheio MOMENTO_RO_FS=/tmp/fs/ro cargo test --release -p momento-project-management --test adverse -- --nocapture
```

  e as permissões como utilizador sem privilégios:

```bash
setpriv --reuid=65534 --regid=65534 --clear-groups env HOME=/tmp TMPDIR=/tmp target/release/deps/adverse-<hash> permissions --exact --nocapture
```

- [ ] CI verde em macOS (Apple Silicon e Intel) e Windows. *(Pendente: repositório remoto.)*

## 2. Aplicação real (compilação de lançamento)

```bash
pnpm --filter @momento/desktop tauri build
```

Num computador limpo de cada sistema:

- [ ] Instala e abre; versão certa no rodapé do início.
- [ ] Novo projeto com nome Unicode ("Casamento Inês & João").
- [ ] Importar um **cartão real** (DCIM, RAW + JPEG) em modo cópia, com verificação e disco
      de segurança; relatório sem erros inesperados; o cartão fica intacto.
- [ ] Importar o **mesmo cartão outra vez** → tudo "já importado"; um **segundo cartão** com
      os mesmos nomes → importado (não ignorado).
- [ ] Miniaturas de todas as fotografias; RAW abre no visualizador e no Develop.
- [ ] Análise termina; rajadas agrupadas; aceitar sugestões não rejeita fotografias com estrelas.
- [ ] Editar, mudar de fotografia a meio de um Auto/máscara IA → cada fotografia com os seus ajustes.
- [ ] Preset a 50 % e a 100 %; sincronizar aspeto não copia máscaras.
- [ ] Fechar a aplicação e reabrir → tudo igual.
- [ ] Exportar o plano "Exportar casamento"; abrir ficheiros: orientação, cor (sRGB/Adobe
      RGB), dimensões, edições aplicadas, GPS removido.
- [ ] Exportar com "Substituir" para a pasta dos originais → os originais **não** mudam.
- [ ] Galeria gerada e aberta num navegador; com PIN; sem caminhos locais no HTML.
- [ ] Álbum: verificação sem erros, PDF de impressão aberto num leitor de PDF (páginas,
      sangria).
- [ ] Desligar o disco dos originais com o projeto aberto → offline; voltar a ligar → reconecta.
- [ ] Terminar o processo à força durante uma importação e uma exportação → reabrir: aviso,
      retoma, nada a meio.
- [ ] Tema claro e escuro; ecrã Retina/150 %: texto e ícones nítidos.

## 3. Dados e privacidade

- [ ] Nenhuma fotografia, modelo grande ou credencial no repositório:

```bash
git ls-files | grep -iE '\.(jpe?g|cr[23w]|nef|arw|raf|orf|dng|heic|onnx|tflite)$'
```

  (só as fixtures sintéticas pequenas são permitidas).
- [ ] `logs/` e crash logs sem caminhos pessoais nem e-mails (ver `RELIABILITY.md`).
- [ ] Nada é enviado para a internet sem o utilizador escolher (galeria: só publicação).

## 4. Documentação

- [ ] `CHANGELOG.md` com a versão e a data.
- [ ] `PROJECT_STATUS.md`, `ROADMAP.md`, `TESTING.md` atualizados.
- [ ] Limitações conhecidas escritas (ver `docs/reports/fase15-auditoria.md`).

## 5. Antes de um trabalho real (fotógrafo)

- [ ] Versão nova experimentada primeiro num projeto de teste.
- [ ] Disco de segurança ligado e escolhido na importação.
- [ ] **Não formatar o cartão** antes de: importação sem erros, verificação ligada e cópia de
      segurança concluída.
- [ ] "Segurança e cópias": versão manual do catálogo criada antes de grandes alterações.
