# Desempenho — Momento Studio

**Fase 12 (2026-11-10).** Objetivo: a aplicação continuar fluida com ~15 000 fotografias.
Todos os números abaixo foram **medidos** (nada estimado), com o método descrito. Onde algo
não foi medido, está escrito.

## Como medir

| Ferramenta | O que mede |
|---|---|
| `cargo run --release -p momento-project-management --example perf_bench -- <cmd>` | Caminho real da aplicação (sessão, catálogo, filas): `import`, `open`, `thumbs`, `thumbgen`, `preview`, `priority`, `nav`, `raw`, `ai`, `export` (ver o cabeçalho do ficheiro) |
| `MOMENTO_PERF=1` ao lançar a aplicação | Modo de diagnóstico na aplicação real: tempo até a interface estar pronta, abertura do projeto, FPS e frames longos durante o scroll, tempo da preview no visualizador — escritos no log (`perf: medição …`), sem dados pessoais |
| `perf record -e cpu-clock -g` | Profiling por amostragem (contadores de hardware indisponíveis no contentor) |
| `EXPLAIN QUERY PLAN` | Planos das consultas SQLite |
| `/proc/<pid>/smaps_rollup` (PSS) | RAM de todos os processos da aplicação (núcleo + WebKit) |

## Ambiente

Linux x86_64, **4 vCPU**, 16 GB de RAM, **sem GPU** (Xvfb, WebKitGTK a desenhar por
software), compilação release. Um Mac ou PC de fotógrafo típico tem mais núcleos e GPU:
os valores absolutos são pessimistas; as **comparações antes/depois** são no mesmo ambiente.
**macOS e Windows: não medidos.**

## Conjuntos de teste

| Conjunto | Conteúdo | Tamanho |
|---|---|---|
| **A — 15 000** | 14 950 JPEG 1600×1067 (24 imagens base, EXIF único em cada ficheiro: conteúdo diferente, descodificação real) + 50 cópias únicas de RAW reais de 18 câmaras | 4,0 GB |
| **B — resolução real** | 40 JPEG **6000×4000 (24 MP)** + 18 RAW (CR2, CR3, NEF D810 36 MP, ARW A7 IV 33 MP, DNG, RAF, ORF, RW2…) | 464 MB |

O conjunto A mede a **escala** (catálogo, grelha, filas, memória); o B mede o **custo por
fotografia** a resolução real. Gerador: `make_set.py` (Python/Pillow), fora do repositório;
nenhuma fotografia está no Git.

## Resultados

### Aplicação real (conjunto A, 15 000 fotografias)

| Medição | Antes | Depois |
|---|---|---|
| Lançamento até à interface pronta (a quente, 3×) | 760–815 ms | 778–800 ms |
| Lançamento com a cache de disco do sistema limpa | 1 334 ms | — (não repetido) |
| Primeiro lançamento na máquina (caches de fontes/WebKit por criar) | 5,4 s (uma vez) | — |
| RAM em repouso (PSS: núcleo + WebKit) | 225 MB | 228 MB |
| RAM com o projeto de 15 000 aberto | 297 MB | 285 MB |
| RAM depois de scroll longo (≈5 000 passos da roda) | sobe e estabiliza em 660–690 MB (cache de imagens do WebKit) | mesmo mecanismo |
| RAM com a análise de IA a correr | — | 927 MB |
| Abrir projeto: lista das 15 000 (IPC, 3,4 MB JSON) | 120 ms | 117 ms |
| Abrir projeto: **grelha pronta** (miniaturas visíveis carregadas) | 794 ms | **242 ms** |
| **Scroll** com a roda (janelas de 3 s) | 67–71 FPS, p95 22–24 ms | 67–75 FPS, p95 18–25 ms |
| Frames > 50 ms durante o scroll | 0–1 por janela de ~200 frames | 0–1 |
| Células no DOM durante o scroll | 44–48 (de 15 000) | 44 |
| Visualizador: preview no ecrã ao mudar de fotografia (sem carga) | — | 55–81 ms (p50 67) |
| Visualizador **com a análise de IA das 15 000 a correr** | — | 61–142 ms (p50 79) |

### Núcleo (benchmarks pelo caminho real)

| Medição | Conj. | Antes | Depois |
|---|---|---|---|
| Abrir a sessão do projeto | A | 8,7 ms | — |
| `list_photos` (15 000) | A | 22–25 ms | — |
| Classificar 200 fotografias (uma transação) | A | 0,4 ms | — |
| Importação (referência, cache quente) | A | 2,9 s (5 089/s) | — |
| **Servir uma miniatura em cache** (8 pedidos em paralelo, como o WebView) | A | 3,80 ms (2 021/s) | **0,10 ms (72 574/s)** |
| Gerar miniaturas 1600 px | A | 119–126/s | 101–126/s |
| Gerar miniaturas 24 MP (cache de disco quente / fria) | B | 43/s / 17/s | — |
| Preview 2560 px a frio, p50 (1600 px / 24 MP) | A / B | 51 ms / 310 ms | — |
| Preview em cache | A | 0,8 ms | — |
| **Preview da fotografia vista com IA + miniaturas em fundo**, p50 | A | 95 ms (sem carga: 64) | **60 ms** (sem carga: 68) |
| idem, 24 MP | B | 315–329 ms (sem carga: 295) | **298–305 ms** (sem carga: 294–309) |
| **Navegação rápida** (16 previews a cada 40 ms): espera da última, média de 4 | B | 844 ms | **562 ms** |
| RAW: preview p50 / máx. | B | 478 ms / 2,1 s | — |
| RAW: revelação completa p50 / máx. (pico de RAM) | B | 2,0 s / 5,8 s (678 MB) | — |
| **Análise de IA por fotografia** (parede, 2 threads, 24 MP + RAW) | B | 0,431 s | **0,233 s** |
| **Exportação JPEG alta resolução, 24 MP** (30 fotografias) | B | 19/min, pico 1,1 GB | **30–31/min**, pico 1,3 GB |
| Contar cópias virtuais de uma fotografia | A | 0,409 ms | **0,004 ms** |
| Apagar 300 fotografias (cascata) | A | 257 ms | **6 ms** |

"—" na coluna "Depois" = código não alterado nesta fase (o valor de "Antes" mantém-se).

## Bottlenecks encontrados (profiling) e o que se fez

Cada alteração partiu de uma medição; cada uma foi medida outra vez depois.

1. **Uma ligação SQLite nova por pedido de imagem** — `perf`: 72 % do tempo de servir uma
   miniatura era abrir/fechar a ligação (ler o esquema, PRAGMAs, contenção no `malloc`).
   → **Pool de ligações** (`project-management::pool`, até 8 inativas). 3,80 → 0,10 ms.
2. **A fotografia vista competia com o segundo plano** — com análise de IA e miniaturas a
   correr, a preview vista ficava 47 % mais lenta. → **Prioridade do primeiro plano**
   (`task-queue::foreground`): previews/1:1 e o editor seguram um `Interactive`; as filas
   de miniaturas, análise, edição automática e exportação cedem entre itens (no máximo 3 s,
   para nunca pararem de vez).
3. **Navegação rápida**: cada seta pedia uma preview, todas ao mesmo tempo; a última (a
   que fica no ecrã) esperava pelas outras. → **Portão LIFO** com 3 em execução (escolhido
   por medição: 1/2/3/sem limite = 575/634/562/844 ms); o que já está em cache sai logo.
4. **Exportação**: 17 % em `powf` (curvas sRGB por canal, exatas "para 16/32 bits") →
   **tabelas de 65 537 entradas** com interpolação, exatas junto aos pontos de quebra e
   perto de 0 nas curvas gama (erro máximo medido < 2,5e-7 a codificar, < 4e-7 a
   descodificar: ~1–3 % de um nível de 16 bits). `sqrt` constante por píxel na geometria
   → pré-calculado.
5. **Exportação**: a CPU estava a 215 % de 400 % — a thread da exportação descodificava e
   codificava em série. → **Pipeline em duas fases** (a fotografia seguinte é preparada
   enquanto esta é codificada e gravada; canal de capacidade 1). Para a RAM não disparar
   (1,8 GB): render linear no próprio buffer e a última receita fica com a imagem sem a
   copiar (`produce_owned`) → 1,3 GB.
6. **Análise de IA**: 51 % em `image::imageops::resize` (mais do que os modelos). →
   Redução triangular em vírgula fixa, separável e paralela (`ai-analysis::resize`; ±2
   níveis de `imageops`, testado). Os hashes percetuais continuam com `imageops` (valores
   gravados de análises anteriores continuam comparáveis). 0,431 → 0,233 s.
7. **SQLite**: `EXPLAIN QUERY PLAN` com 15 000 — o índice de `variant` era parcial (só a
   versão principal); cópias virtuais e apagar em cascata percorriam a tabela. → Migração
   v8: `variant_by_asset`, `snapshot_by_variant`.
8. **Caches em memória**: a cache de fontes do editor saía por ordem de entrada → **LRU
   verdadeira**; resultados de AI Denoise em resolução total (~400 MB a 24 MP) → no máximo
   **um** em memória.

## Já existia (verificado à escala de 15 000)

| Pedido | Como | Verificação |
|---|---|---|
| Virtualização da grelha / lazy loading | Só as linhas visíveis (+ margem) no DOM; `<img decoding=async>` | 44 células no DOM com 15 000 fotografias; miniaturas só pedidas quando prontas |
| Cache de miniaturas | Disco, por projeto, nunca limpa (≈30 KB/fotografia: 457 MB para 15 000) | — |
| Cache de previews | Disco, LRU por data de acesso, 10 GB (previews) e 5 GB (1:1) | — |
| Filas, workers, prioridades | `WorkQueue` com prioridade para o visível; N−1 threads de miniaturas; análise com metade dos núcleos | Miniaturas visíveis primeiro (`prioritize_thumbnails`) |
| Batching e transações | Escritor de miniaturas em lotes de 48 ou 150 ms numa transação; classificações numa transação; eventos agregados | 200 classificações em 0,4 ms |
| Processamento incremental | Estado por fotografia no catálogo (`thumb_state`, análise pendente, importação suspensa) — retomado na abertura | — |
| Sem milhares de RAW completos em RAM | RAW: preview embutida ou meia resolução para ecrã; resolução total só a pedido; cache de fontes do editor com 3 entradas; nenhuma fonte de resolução total em cache | Pico do benchmark RAW: 678 MB (NEF 36 MP) |

## RAM: o que se sabe

- Núcleo com o projeto aberto: ~120 MB. Gerar 15 000 miniaturas: 180 MB; numa segunda
  passagem completa, 234 MB — **não é uma fuga** (não duplica); é fragmentação do alocador.
- WebView: a cache de imagens descodificadas cresce durante o scroll e **estabiliza**
  (~540 MB no WebKitGTK; as miniaturas têm 640 px = ~1,1 MB descodificadas, para ecrãs
  retina). WKWebView (macOS) e WebView2 (Windows) têm políticas próprias: **não medido**.
- Exportação 24 MP: pico 1,3 GB (uma fotografia em preparação + uma a codificar).

## GPU e I/O

- **GPU:** o render é CPU (rayon). Este ambiente não tem GPU, por isso nada foi medido
  com GPU. Os bottlenecks encontrados (descodificar, reduzir, codificar, SQLite) não são de
  GPU. Render com wgpu fica para a fase seguinte, para medir num Mac.
- **I/O:** com a cache de disco fria, as miniaturas de 24 MP caem de 43/s para 17/s — ler
  os originais domina. A importação lê os ficheiros uma vez (hash BLAKE3) e grava em lotes.

## Limitações conhecidas

- **Revelação RAW** (LibRaw, demosaico AHD) numa só thread: 2–6 s em resolução total. A
  LibRaw paraleliza com OpenMP, mas o Apple Clang não traz OpenMP — ativar partiria a
  compilação no macOS. A revelação a 100 % já tem prioridade sobre o segundo plano.
- DNG de telemóveis/drones (4000×3000) sem preview embutida grande: preview de ecrã com
  revelação completa (~1,7–2,1 s), porque meia resolução fica abaixo dos 2560 px pedidos.
- A preview a frio de 24 MP (~300 ms) é sobretudo descodificar o JPEG original.
- Valores de macOS/Windows e de discos reais (HDD, rede, cartões) **não medidos**.
