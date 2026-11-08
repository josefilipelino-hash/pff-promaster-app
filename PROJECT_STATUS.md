# Estado do projeto

**Última atualização:** 2026-11-08 · **Fase atual:** 0 — Fundação técnica · **Estado:** ✅ concluída, aguarda aprovação para a Fase 1
**Versão:** 0.0.1 · **Branch:** `main`

---

## Arquitetura escolhida

**Tauri 2.12 (desktop) + React 19 / TypeScript 5.9 (interface) + Rust (núcleo) + SQLite
(catálogo por projeto)**, com LibRaw (RAW), LittleCMS 2 (cor), ONNX Runtime + Core ML/Vision
(IA local) e wgpu (GPU, Fase 9). Comparação técnica e justificação completas em
[`ARCHITECTURE.md`](ARCHITECTURE.md) §1.

## Decisões tomadas

| ID | Decisão |
|---|---|
| D-001 | Tauri 2 em vez de Electron (memória, núcleo nativo), SwiftUI (exclui Windows) e Flutter (ecossistema fraco para imagem/IA). Tauri 3 está em alpha → fixado 2.12.5 |
| D-002 | Um catálogo SQLite por projeto (`<nome>.momento/catalog.db`) + biblioteca global pequena |
| D-003 | Espaço de trabalho linear Rec.2020 em `f32`; um único pipeline para preview e exportação |
| D-004 | LibRaw compilada a partir do código-fonte, usada sob licença CDDL-1.0 |
| D-005 | Metadados lidos em Rust nativo; ExifTool só como ferramenta externa opcional |
| D-006 | Codecs como adaptadores compilados (features), escolhidos por prioridade; sem plugins dinâmicos na v1 |
| D-007 | ONNX Runtime (CoreML/DirectML EP) + backends Apple Vision/Core ML opcionais atrás do mesmo contrato |
| D-008 | Imagens entregues à interface por protocolo URI próprio (sem base64, sem servidor HTTP) |
| D-009 | pnpm com `minimumReleaseAge` de 7 dias; só `esbuild` executa scripts de instalação |
| D-010 | Nome provisório “Momento Studio”, identificador `pt.momentostudio.desktop` (confirmar antes do fim da Fase 1) |

Incompatibilidades verificadas e as respetivas decisões: `ARCHITECTURE.md` §19 (16 itens).

## Funcional (verificado)

| Funcionalidade | Como foi verificada |
|---|---|
| Aplicação Tauri abre e mostra o ecrã de diagnóstico em pt-PT | Executada em Linux (Xvfb) — captura em `docs/screenshots/fase0-linux.png` |
| Ponte interface ↔ núcleo Rust (comando `app_info`) | Captura mostra versões e plataforma vindas do Rust; teste unitário |
| Catálogo SQLite: criação, WAL, migrações, recusa de versões futuras | 3 testes |
| Deteção de 32 formatos/contentores pelo conteúdo, independente da extensão | 8 testes (inclui 5 000 entradas aleatórias) |
| Registo de codecs com prioridades e recurso | 1 teste |
| Modelo da fila: prioridades, transições, recuperação após falha | 4 testes |
| Receita de edição: envelope versionado, preserva campos futuros | 2 testes |
| i18n: textos fora do código, pt-PT e en com chaves idênticas | 6 testes |
| Regra “sem botões sem funcionalidade” no ecrã atual | 1 teste |

## Não funcional (ainda não implementado)

Tudo o que é funcionalidade de utilizador: criar/abrir projetos, importar, thumbnails,
grelha, classificação, RAW, análise, edição, presets, exportação, galerias, álbuns.
Os crates `project-management`, `metadata`, `import-engine`, `raw-engine`,
`image-processing`, `color-management`, `ai-analysis`, `face-analysis`, `presets`,
`export-engine`, `gallery`, `album` estão **reservados e vazios** (só documentação), de
propósito.

**Não verificado nesta sessão:** execução no macOS. O ambiente de desenvolvimento desta
sessão é Linux; o CI está configurado para macOS 14 (Apple Silicon), mas só corre quando o
repositório for publicado no GitHub.

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Fidelidade de cor/HDR na WebView | Médio | Previews com ICC do monitor; plano B: superfície Metal nativa (§1.3) |
| Qualidade da revelação RAW vs. Lightroom | Alto | LibRaw + perfis DCP; afinação com ficheiros reais do utilizador |
| Denoise por IA de qualidade em RAW | Alto | Denoise clássico primeiro; modelos em RGB linear avaliados com comparação visual |
| LibRaw (C++) a terminar o processo com ficheiros malformados | Médio | Processo auxiliar (validar na Fase 2) |
| ONNX Runtime sem binários para macOS Intel | Baixo-médio | Validar na Fase 3; recurso CPU/build próprio |
| Precisão da organização por momentos | Médio | Tempo + CLIP + regras; correção manual fácil |
| Catálogo colocado em disco de rede | Médio | Aviso e bloqueio na Fase 1 |
| Licença LGPL da porta Rust do Lensfun | Baixo | Implementação própria + base de dados (decisão na Fase 4) |
| Testes macOS dependem do CI ou da máquina do utilizador | Médio | Workflow `macos-14` pronto; verificação manual documentada |

## Dependências

Lista completa com versões e licenças em [`LICENSES.md`](LICENSES.md). Em uso: tauri
2.12.5, rusqlite 0.40.2 (SQLite incluído), serde, thiserror; React 19.3.1, i18next 26.5,
Vite 7.3, Vitest 4.1, TypeScript 5.9.3. Ferramentas: Rust estável (testado 1.97), Node 22,
pnpm 10.28.

## Comandos exatos

Preparação do macOS (uma vez): ver [`INSTALLATION.md`](INSTALLATION.md).

```bash
pnpm install
```

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

```bash
pnpm build
```

## Testes realizados (2026-11-08, Linux x86_64)

| Verificação | Resultado |
|---|---|
| `pnpm typecheck` (3 pacotes) | ✅ sem erros |
| `pnpm test` | ✅ 9/9 (localization 6, desktop 3) |
| `cargo fmt --all --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ sem avisos |
| `cargo test --workspace` | ✅ 20/20 |
| `pnpm --filter @momento/desktop build` (Vite) | ✅ |
| `cargo build -p momento-desktop` + execução real em Xvfb | ✅ janela aberta, diagnóstico preenchido pelo núcleo |
| Execução em macOS | ⏳ não verificado (ver acima) |

## Histórico de fases

| Fase | Estado | Data |
|---|---|---|
| 0 — Fundação técnica | ✅ concluída | 2026-11-08 |
| 1 — Aplicação utilizável | ⏸ aguarda aprovação | — |

## Próximo passo

**Fase 1 — Aplicação utilizável**, começando por:
1. Esquema v2 do catálogo (`project`, `source_root`, `asset`, `variant`, `rating`,
   `import_session`, `import_item`, `preview`, `job`) e repositórios.
2. Executor da fila de tarefas (tokio + rayon) persistente.
3. Adaptador `image-rs` (JPEG/PNG/TIFF) e geração de thumbnails com cache e protocolo
   `momento://`.
4. Interface: ecrã inicial, criar projeto, importação, grelha virtualizada, loupe,
   classificação.

**Critério de conclusão:** os 10 critérios da Fase 1 em [`ROADMAP.md`](ROADMAP.md),
verificados e registados aqui.

**Pendente do utilizador:** aprovação desta arquitetura; nome definitivo da aplicação;
criação do repositório GitHub (para CI em macOS); modelo do Mac; fotografias RAW de teste
(fora do Git) para a Fase 2.
