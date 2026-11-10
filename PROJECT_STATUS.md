# Estado do projeto

**Última atualização:** 2026-11-10 · **Fase atual:** 16 — Versão instalável · **Estado:** ✅ concluída
**Versão:** 1.0.0-personal · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

Primeira versão instalável para uso pessoal. Build de produção reproduzível
(`scripts/build-release.sh`), atualização de versões sem risco para os catálogos (cópia
completa antes de cada migração — testada com catálogos reais de versões antigas),
permissões do macOS explicadas, pacote mais pequeno, registos com retenção. Smoke test do
pacote final instalado: lançamento, projeto, importação, edição, reinício, exportação,
recuperação e atualização de catálogos — **todos ✅** (`docs/reports/fase16-smoke-test.md`).
Notas da versão: **`RELEASE_NOTES.md`**. Instalação: **`INSTALLATION.md`**.

**Importante:** o `.app`/`.dmg` de macOS é gerado **no Mac** (`./scripts/build-release.sh`)
ou pela CI (macos-14, em cada tag). Este ambiente é Linux e não tem as ferramentas da Apple:
aqui foram gerados e testados os pacotes Linux do mesmo código.

## Funcional (verificado)

| Pedido | Como | Verificação |
|---|---|---|
| Build de produção macOS | `scripts/build-release.sh` → `.app` + `.dmg` (opção `--universal`); CI em tags | Script executado em Linux (mesmo processo, pacotes `.deb`/AppImage); **não executado em macOS** |
| Tamanho | Sem source maps (−2,3 MB), sem debug info; `.deb` 108 MB (dos quais 102 MB modelos) | Medido |
| Dependências | Locks exatos (`--frozen-lockfile`, `Cargo.lock`), Rust 1.97.0 fixado; IA em Rust puro (tract) — sem bibliotecas externas | Verificado |
| Assets / modelos | 9 modelos, todos usados, verificados por SHA-256 no build; mantidos em precisão total | Verificado no build |
| Debug symbols | Debug info removida; nomes de funções mantidos (relatórios de falhas legíveis; −9 MB possíveis, decisão D-107) | Medido |
| Logging | Info, um ficheiro por dia, últimos 14 | Código |
| Processo reproduzível | Ferramentas e versões verificadas, `SOURCE_DATE_EPOCH`, `BUILD_INFO.txt`, `SHA256SUMS.txt` | Executado |
| Ícone, nome, versão, informações | Ícone próprio (16–1024 px), "Momento Studio", 1.0.0-personal (pacote 1.0.0), editor/copyright/descrições | Pacote |
| Diretórios | Dados em `~/Library/Application Support/pt.momentostudio.desktop`, registos em `~/Library/Logs/…`; projetos onde o utilizador escolhe | `INSTALLATION.md` |
| Migrations / atualização segura | Cópia antes de migrar; versão antiga recusa catálogo novo | Teste + smoke test com catálogos v3 e v7 |
| Permissões (Pictures, volumes externos, cartões, pastas escolhidas) | Sem sandbox; `Info.plist` com explicações TCC | **Não verificado em macOS** (só configuração) |
| Assinatura/notarização | Documentadas (`docs/SIGNING.md`); não bloqueiam | — |
| Smoke test | Pacote instalado, dados vazios | ✅ (relatório) |

## Não funcional / limitações

- **Build e execução em macOS não feitos neste ambiente** — primeiro passo no Mac:
  `./scripts/build-release.sh` e a secção 2 de `RELEASE_CHECKLIST.md`.
- Aplicação não assinada: confirmação na primeira abertura.
- Limitações das fases anteriores mantêm-se (ver `RELEASE_NOTES.md`).

## Arquitetura

Decisões D-104 a D-108. Novos: `scripts/build-release.sh`, `apps/desktop/src-tauri/Info.plist`,
`docs/SIGNING.md`, `RELEASE_NOTES.md`, cópia antes da migração em `momento-database`.

## Dependências novas

Nenhuma (`tracing` passou a ser usado também em `momento-database`).

## Comandos exatos

```bash
./scripts/build-release.sh
```

```bash
./scripts/build-release.sh --universal
```

## Testes realizados (2026-11-10)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (RAW reais, modelos, rostos, discos cheio/só de leitura) | ✅ 286 passaram, 0 falharam, 1 ignorado (débito 24 MP, manual) |
| `pnpm -r test` (Vitest) | ✅ 94 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy` | ✅ limpos |
| Smoke test do pacote final (Linux) | ✅ lançamento, projeto, importação, edição, reinício, exportação, recuperação, atualização v3/v7 |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Primeiro build em macOS falhar (ferramentas, LibRaw/lcms2 em Apple Silicon) | Médio | CI em macos-14 já compilava em cada envio (Fases 1–15); script verifica as ferramentas antes de compilar |
| Pedidos de acesso do macOS diferentes do esperado | Baixo | Sem sandbox; explicações no `Info.plist`; recuperação em Definições → Privacidade |

## Histórico de fases

| Fase | Estado | Data |
|---|---|---|
| 0 — Fundação técnica | ✅ | 2026-11-08 |
| 1 — Aplicação utilizável | ✅ | 2026-11-08 |
| 2 — Formatos e RAW | ✅ | 2026-11-08 |
| 3 — Seleção automática | ✅ | 2026-11-09 |
| 4 — Organização do casamento | ✅ | 2026-11-09 |
| 5 — Motor de edição | ✅ | 2026-11-09 |
| 6 — Edição automática | ✅ | 2026-11-09 |
| 7 — Presets e consistência visual | ✅ | 2026-11-09 |
| 8 — Edição localizada, retoque e retrato | ✅ | 2026-11-09 |
| 9 — Exportação | ✅ | 2026-11-09 |
| 10 — Galerias | ✅ | 2026-11-10 |
| 11 — Álbuns | ✅ | 2026-11-10 |
| 12 — Desempenho | ✅ | 2026-11-10 |
| 13 — Fiabilidade | ✅ | 2026-11-10 |
| 14 — UX/UI profissional | ✅ | 2026-11-10 |
| 15 — Estabilização | ✅ | 2026-11-10 |
| 16 — Versão instalável (1.0.0-personal) | ✅ | 2026-11-10 |
| 17 — Distribuição | ⏸ próxima | — |

## Próximo passo

**No Mac:** `./scripts/build-release.sh`, instalar o `.dmg` e correr a secção 2 de
`RELEASE_CHECKLIST.md` com um cartão real de teste antes do próximo trabalho.

**Fase 17 — Distribuição:** assinatura/notarização (`docs/SIGNING.md`), menu nativo,
GPU, atualizações automáticas.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); conta Apple Developer
(opcional); fotografias de teste; medidas do laboratório; nome definitivo.
