# Estado do projeto

**Última atualização:** 2026-11-09 · **Fase atual:** 3 — Seleção automática · **Estado:** ✅ concluída
**Versão:** 0.3.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

A aplicação analisa as fotografias **neste computador** e recomenda, com explicação, as
melhores de cada momento: **MELHOR / ALTERNATIVA / REVER / REJEITAR SUGERIDO /
DUPLICADA**. A IA só recomenda: nunca apaga, nunca altera favoritas/rejeitadas; aceitar
uma sugestão é sempre uma ação explícita e reversível.

- Métricas clássicas + IA local (modelos MediaPipe, Apache-2.0) executada com
  **tract-onnx** (Rust puro, igual em macOS e Windows, sem serviços externos).
- Grupos: RAW+JPEG, duplicados, sequências/rajadas (tempo, semelhança visual, rostos,
  várias câmaras), com classificação dentro do grupo e piscadelas relativas a cada pessoa.
- Modos conservador / equilibrado / rigoroso e objetivo de entrega aproximado, recalculados
  em milissegundos sem reanalisar.
- Interface: painel “Seleção automática”, etiquetas e “1/12” na grelha, vista
  **Sequências**, **comparação lado a lado** (2–4) com zoom e deslocamento sincronizados,
  explicação em frase e pontuações individuais no painel de informação.

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Nitidez global / pico de foco / centro, tremido, exposição, realces queimados, sombras bloqueadas, ruído, inclinação, pHash/dHash | 7 testes de métricas + teste de hash |
| Rostos (deteção multiescala), 478 pontos, 52 blendshapes (olhos fechados, sorriso, boca), nitidez do rosto e dos olhos, rosto cortado | Teste contra os **resultados de referência publicados pela Google** (`portrait.jpg`): caixa a ±5 px, pontos a ±0,002, eyeBlink 0,28/0,27 vs 0,26/0,30, sorriso 0,94 vs 0,92; fotografias reais dlib/OpenCV com boa deteção |
| Embedding visual (semelhança) | Teste: semelhante 0,91 vs diferente 0,18 |
| Grupos: rajada de 14 (beijo), duas câmaras, RAW+JPEG, duplicados, momentos afastados não agrupados | 8 testes do motor |
| Recomendação explicada (“Melhor fotografia desta sequência: maior nitidez facial e menos tremido.”) | Testes + aplicação real |
| Pontuações individuais guardadas (foco, tremido, exposição, ruído, olhos, expressão, composição, técnica, pessoas, global) | Catálogo v4 + painel na aplicação real |
| Modos e objetivo de entrega | Testes do motor, da sessão e da interface + aplicação real (objetivo 8 → 7 sugeridas + 1 favorita do utilizador) |
| Comparação lado a lado: zoom/deslocamento sincronizados, estrelas, aceitar, rejeitar, “selecionar outra”, teclado no painel ativo | Testes de interface + **aplicação real** (4 painéis) |
| Aceitar sugestões (fotografia ou grupo) sem tocar em decisões do utilizador | Testes da base de dados, da sessão e da interface |
| Análise em segundo plano, retomável após fechar; sem modelos → só métricas técnicas com aviso | Teste de integração (fechar/reabrir) |
| Aplicação real (release, Linux): rajada de 12 com desfocagens 0–4 px **ordenada exatamente** pela nitidez (1/12 = nítida, 12/12 = mais desfocada); rajada de 6 de grupo idem | Capturas no ecrã virtual |

## Não funcional / não validado

- **Olhos fechados com fotografias reais:** sem acesso a imagens reais de olhos fechados
  nesta sessão. Validado indiretamente (coeficientes iguais à referência da Google para
  olhos abertos) e com valores sintéticos no motor. **É o primeiro ponto a testar com as
  tuas fotografias.**
- **Obstruções:** só detetadas como rosto cortado/oculto (presença baixa). Não há deteção
  de objetos à frente das pessoas (ex.: convidado com telemóvel).
- “Pessoas importantes fora de foco” é avaliado pelos rostos **maiores** da fotografia
  (rostos principais); o reconhecimento de quem são os noivos chega na Fase 5 (Pessoas).
- O tremido é estimado pela coerência dos gradientes e também sobe com desfocagem forte;
  é usado como critério secundário.
- Sem aceleração por Neural Engine/GPU (D-024); os modelos são pequenos e correm na CPU.
- macOS/Windows: compilação verificada (Windows: workspace completo; macOS arm64: análise
  e codecs, incluindo os kernels NEON do tract), **não executada** nesses sistemas.
- HEIC/AVIF (Fase 2) continuam por validar em macOS/Windows reais.

## Desempenho medido (Linux x86_64, **4 núcleos**, release)

| Operação | Resultado |
|---|---|
| Motor de seleção (grupos + pontuações + etiquetas) para **15 000 fotografias** | ≈ 32 ms (2 668 grupos) |
| Mudar modo/objetivo (recalcular a partir do catálogo) | instantâneo na aplicação |
| Análise completa por fotografia (JPEG 24 MP, rostos multiescala, embedding), 2 threads de análise com as miniaturas a correr em paralelo | **≈ 330 ms/foto** → 15 000 fotografias ≈ 80 min nesta máquina |
| Análise de fotografias pequenas (≤ 2 MP) | ≈ 120–200 ms/foto |
| Inferência por modelo | deteção 5,6 ms · pontos 23 ms · blendshapes 1,5 ms · embedding 7 ms |

Num Mac/PC com 8–12 núcleos espera-se 2–3× mais rápido (mais threads de análise), **não
medido**. Os RAW usam a preview já gerada na importação (sem nova revelação).

## Arquitetura

Novas decisões D-024 a D-028 em `ARCHITECTURE.md` (§11 reescrita): tract-onnx em vez de
ONNX Runtime; modelos MediaPipe convertidos por `scripts/prepare-models.py` e incluídos no
instalador; imagem de trabalho ≤ 2048 px; recomendações calculadas a partir das métricas
gravadas; a IA só recomenda.

**Desvio ao plano da Fase 0:** o plano previa ONNX Runtime (`ort`). Não foi usado porque
o `ort` 2.0 só existe em versões RC e depende de binários nativos descarregados no build
(macOS Intel incerto). Impacto: sem Neural Engine/GPU por agora; ganho: o mesmo código
puro Rust em todas as plataformas.

## Dependências novas

tract-onnx 0.23.9 (MIT/Apache-2.0), anyhow 1. Modelos (não versionados): BlazeFace, Face
landmarks, Face blendshapes, MobileNetV3-small — Google MediaPipe, Apache-2.0, ≈ 11 MB no
total. Ver `LICENSES.md`.

## Comandos exatos

```bash
git submodule update --init --recursive
```

```bash
python3 -m venv .venv-models
```

```bash
.venv-models/bin/pip install tf2onnx==1.17.0 tensorflow-cpu==2.21.0 onnx==1.23.2
```

```bash
.venv-models/bin/python scripts/prepare-models.py
```

```bash
pnpm install
```

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Testar a seleção com as tuas fotografias (nada é copiado para o repositório):

```bash
MOMENTO_FACES_DIR=/caminho/para/fotos cargo test -p momento-project-management --release --test analysis_flow -- --nocapture
```

```bash
MOMENTO_PERF_N=300 cargo test -p momento-project-management --release --test analysis_flow -- --ignored --nocapture
```

## Testes realizados (2026-11-09)

| Verificação | Resultado |
|---|---|
| `cargo test --workspace` (com `MOMENTO_MP_PORTRAIT` e `MOMENTO_FACES_DIR`) | ✅ 109 passados, 1 ignorado (débito 24 MP, executado à parte) |
| `pnpm -r test` | ✅ 44/44 (38 da aplicação, 6 de traduções) |
| `pnpm -r typecheck`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ |
| Débito 24 MP (200 fotografias) | ✅ 328 ms/foto |
| Aplicação real (release, Linux): analisar, etiquetas, painel, comparação com zoom/deslocamento sincronizados, teclado, sequências, objetivo | ✅ |
| `cargo check --workspace --target x86_64-pc-windows-gnu` | ✅ |
| `cargo check` análise + codecs `--target aarch64-apple-darwin` | ✅ |
| macOS / Windows reais | ⏳ não executado |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Olhos fechados não validados com fotografias reais | Alto | Testar com uma rajada real; limiares ajustáveis no motor (`selection.rs`) |
| 15 000 fotografias ≈ 80 min numa máquina de 4 núcleos | Médio | Análise em segundo plano e retomável; usa a preview já existente; aceleração (Core ML/DirectML) se necessário |
| tensorflow-cpu necessário para preparar modelos (só CI/desenvolvimento) | Baixo | Feito num único trabalho Linux do CI, com cache; SHA-256 fixados em `models.lock.json` |
| HEIC/AVIF e a aplicação inteira não executados em macOS/Windows | Alto | Primeira verificação manual |

## Histórico de fases

| Fase | Estado | Data |
|---|---|---|
| 0 — Fundação técnica | ✅ | 2026-11-08 |
| 1 — Aplicação utilizável | ✅ | 2026-11-08 |
| 2 — Formatos e RAW | ✅ | 2026-11-08 |
| 3 — Seleção automática | ✅ | 2026-11-09 |
| 4 — Edição | ⏸ próxima | — |

## Próximo passo

**Recomendado antes da Fase 4:** no teu Mac/PC, preparar os modelos, `pnpm dev`, importar
um casamento real (incluindo rajadas com piscadelas) e analisar. Confirmar: olhos fechados
assinalados, rajadas bem agrupadas, a melhor de cada sequência, tempo de análise.

**Fase 4 — Edição:** revelação RAW não destrutiva (exposição, balanço de brancos, tons,
cor), presets, cópia de ajustes entre fotografias.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste
(rajadas com olhos fechados, RAW das tuas câmaras, HEIC do iPhone); nome definitivo.
