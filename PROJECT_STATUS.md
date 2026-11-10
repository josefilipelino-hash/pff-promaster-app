# Estado do projeto

**Última atualização:** 2026-11-10 · **Fase atual:** 12 — Desempenho · **Estado:** ✅ concluída
**Versão:** 0.12.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

Fase dedicada a desempenho, medida com um projeto de **15 000 fotografias** e com
fotografias a resolução real (24 MP e RAW de 18 câmaras). Profiling com `perf` em vez de
intuição; cada alteração medida antes e depois. Detalhes e método: **`PERFORMANCE.md`**.

## Resultados principais (medidos)

| Medição | Antes | Depois |
|---|---|---|
| Lançamento até à interface pronta (a quente) | 760–815 ms | 778–800 ms |
| RAM em repouso / com 15 000 abertas (PSS, núcleo + WebKit) | 225 / 297 MB | 228 / 285 MB |
| Abrir projeto de 15 000: grelha pronta | 794 ms | **242 ms** |
| Scroll da grelha (15 000) | 67–71 FPS | 67–75 FPS (44 células no DOM) |
| Servir miniatura em cache | 3,80 ms | **0,10 ms** |
| Preview da fotografia vista com IA em fundo (p50) | 95 ms (sem carga 64) | **60 ms** |
| Navegação rápida: espera da fotografia em que se para | 844 ms | **562 ms** |
| Visualizador na aplicação, com a análise das 15 000 a correr | — | p50 79 ms (sem carga 67) |
| Miniaturas (1600 px / 24 MP) | 119/s / 43/s | sem alteração |
| Abrir RAW: preview p50 / revelação completa p50 | 478 ms / 2,0 s | sem alteração |
| IA por fotografia (24 MP, 2 threads) | 0,431 s | **0,233 s** |
| Exportação JPEG 24 MP | 19/min | **30–31/min** |

## Funcional (verificado)

| Pedido | Estado |
|---|---|
| Lazy loading, virtualização da grelha | Já existia; verificado com 15 000 (44 células no DOM) |
| Cache de miniaturas e de previews (disco, LRU) | Já existia; verificado |
| LRU em memória | Corrigida (era FIFO) + limite de AI Denoise em resolução total |
| Processamento paralelo controlado, filas, workers | Miniaturas N−1, análise N/2, previews 3 (LIFO), exportação em pipeline com 1 adiantada |
| Prioridades: "a fotografia 3421 primeiro" | **Novo:** primeiro plano global + portão LIFO; medido no núcleo e na aplicação real |
| Batching e transações | Já existia (lotes de 48/150 ms); verificado |
| Índices SQLite | **2 novos** (migração v8), por `EXPLAIN QUERY PLAN` |
| Processamento incremental | Já existia (estado por fotografia, retoma) |
| Não carregar milhares de RAW completos | Verificado: preview embutida/meia resolução; resolução total só a pedido, nunca em cache |
| Medições pedidas | Todas em `PERFORMANCE.md` (lançamento, RAM, FPS, miniaturas, RAW, IA, exportações/min) |

## Não funcional / limitações

- **GPU:** não medida nem usada (ambiente sem GPU); render em CPU.
- **macOS e Windows:** nenhuma medição. O WebView do Linux (WebKitGTK) usa até ~540 MB de
  cache de imagens durante scroll longo (estabiliza); WKWebView/WebView2 por medir.
- **RAW em resolução total** numa só thread (LibRaw sem OpenMP, D-085): 2–6 s.
- DNG de telemóveis/drones sem preview grande: ~2 s para a preview de ecrã.
- Conjunto de 15 000 é sintético (JPEG 1600 px); o custo a resolução real foi medido à
  parte com 58 ficheiros. Discos lentos (HDD, rede, cartões) não medidos.

## Arquitetura

Decisões D-078 a D-085. Novos: `task-queue::foreground` (primeiro plano + portão LIFO),
`project-management::pool`, `ai-analysis::resize`, `color_management::EncodeLut`,
`develop::color::eotf_hq`, `Prepared::produce_owned`, migração v8, modo de diagnóstico
(`perf_mode`/`perf_log`, `logic/perfProbe.ts`).

## Dependências novas

Nenhuma (a `rayon`, já usada, passa a ser dependência do `ai-analysis`).

## Comandos exatos

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Benchmarks (ver o cabeçalho do ficheiro para todos os comandos):

```bash
cargo run --release -p momento-project-management --example perf_bench -- open <projeto>
```

Aplicação com o diagnóstico de desempenho no log:

```bash
MOMENTO_PERF=1 pnpm dev
```

## Testes realizados (2026-11-10)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (com amostras RAW, modelos e rostos) | ✅ 255 passaram, 0 falharam, 1 ignorado (débito 24 MP, manual) |
| `pnpm -r test` (Vitest) | ✅ 73 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ limpos |
| Aplicação real (release, Linux/Xvfb, 15 000 fotografias) | ✅ lançamento, RAM, abrir projeto, scroll, visualizador com/sem análise em fundo |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| WebView de macOS/Windows com outra política de memória | Médio | Medir com `MOMENTO_PERF=1` num Mac/PC real |
| RAW de 45–60 MP em resolução total lentos | Médio | Prioridade sobre o segundo plano; OpenMP/GPU numa fase futura |
| Exportação a 45 MP: pico de RAM ~2× o de 24 MP | Médio | Pipeline com 1 adiantada; medir em 8 GB |

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
| 13 — Distribuição | ⏸ próxima | — |

## Próximo passo

**Recomendado:** abrir um casamento teu grande com `MOMENTO_PERF=1` num Mac e enviar-me
as linhas `perf: medição` do log, para ter números reais de macOS (WKWebView, Apple
Silicon, GPU).

**Fase 13 — Distribuição:** GPU (wgpu), Apple Silicon, recuperação, instaladores
assinados.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
medidas do laboratório; nome definitivo.
