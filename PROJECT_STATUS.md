# Estado do projeto

**Última atualização:** 2026-11-09 · **Fase atual:** 4 — Organização do casamento · **Estado:** ✅ concluída
**Versão:** 0.4.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

> A pedido do utilizador, a organização do casamento (planeada como Fase 5) foi feita antes
> da edição; a Edição passa a ser a Fase 5 (ver `ROADMAP.md`).

---

## Resumo

A aplicação organiza o casamento **neste computador**:

- **Pessoas:** deteta rostos, calcula a identidade facial (SFace) e agrupa as mesmas
  pessoas. Identificas noiva, noivo, pais, irmãos, padrinhos, madrinhas e pessoas
  importantes; pesquisas “todas as fotografias da noiva”, “noivos juntos”, “noiva + mãe”,
  “noivo + pai” ou qualquer pessoa.
- **Momentos:** 25 momentos de casamento, atribuídos **só quando há confiança suficiente**
  (o resto fica em “Por classificar”). Momentos personalizados, arrastar fotografias para
  um momento, e a classificação aprende com as tuas escolhas.
- **Sincronização de câmaras:** desvio por câmara (“Sony +3min42s”), sugestão automática,
  cronologia, sequências e momentos reconstruídos. A hora original nunca é alterada.

Nenhum dado biométrico sai do computador: a identidade facial fica só no catálogo do
projeto e pode ser apagada.

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Identidade facial SFace (alinhamento por 5 pontos) | tract = onnxruntime oficial (cos 1,000000); mesma pessoa 0,96–0,98, pessoas diferentes < 0,08 com fotografias reais |
| Agrupamento de rostos | Testes (grupos puros, convidados isolados não viram pessoas, dois rostos da mesma fotografia nunca juntos) + **aplicação real**: 6 pessoas de fotografias diferentes agrupadas corretamente |
| Identificar (nome + papel), juntar, “não é esta pessoa”, ocultar, reagrupar, apagar dados | Testes da base de dados, da sessão e da interface + aplicação real |
| Decisões sobre pessoas sobrevivem à reanálise e ao reagrupamento | Teste da base de dados |
| Pesquisas: noiva, noivo, noivos juntos, noiva + mãe, noivo + pai, uma pessoa | Testes + aplicação real (“Noivos juntos” = as 9 fotografias de grupo de 2 câmaras) |
| Classificador de cena EfficientNet-Lite2 | tract = TFLite oficial (diferença máx. 5,5e-7) |
| 25 momentos com confiança; “Por classificar” | Dia de casamento **sintético** todo correto; sem pistas nada é forçado (teste com fotografias reais e aplicação real) |
| Momentos personalizados, arrastar para um momento, escolha no painel, voltar ao automático | Testes + aplicação real |
| Aprendizagem com exemplos do utilizador (fotografias parecidas na mesma cena) | Testes + aplicação real (exemplos em conflito → “Por classificar”) |
| Sincronização: desvio por câmara, sugestão automática, cronologia reconstruída | Testes (Canon certa, Sony +3min42s, DJI −1min15s) + teste real + **aplicação real** (sugestão exata “+3min42s”, 7 pares, 100 % de acordo) |
| GPS do EXIF (separa cenas) | Testes |

## Não funcional / não validado

- **Precisão dos momentos num casamento real: não medida.** Não havia fotografias de
  casamento nesta sessão; as regras foram afinadas num dia sintético. É o principal ponto
  a validar com um casamento teu.
- O ImageNet não tem classes como “anel”, “bolo de casamento”, “convite” ou “vestido de
  noiva”: Alianças, Bolo, Convites e Vestido dependem de pistas aproximadas (colar,
  sobremesas, envelope/menu, “gown”) e são mais fracos. A aprendizagem com exemplos ajuda.
  Um modelo tipo CLIP melhoraria isto, mas não havia um com proveniência verificável.
- Sub-momentos da cerimónia (Entrada, Troca de alianças, Beijo) dependem de a cerimónia ser
  detetada (igreja/altar). Cerimónias ao ar livre podem não ser detetadas → ficam por
  classificar.
- A sugestão de sincronização tem a precisão do par de fotografias mais parecido
  (segundos); não funciona sem fotografias parecidas tiradas por várias câmaras, nem para
  desvios maiores que ±2 h (introduz-se à mão).
- Foto-colagens com a mesma pessoa duas vezes ficam em dois grupos (regra “dois rostos da
  mesma fotografia nunca são a mesma pessoa”); juntam-se com “Juntar”.
- macOS/Windows: compilação verificada, **não executada** nesses sistemas.

## Desempenho medido (Linux x86_64, 4 núcleos, release)

| Operação | Resultado |
|---|---|
| Agrupamento de **30 000 rostos** | ≈ 7–9 s |
| Classificação de momentos, **15 000 fotografias** | ≈ 0,2 s |
| Seleção (Fase 3), 15 000 fotografias | ≈ 25 ms |
| Análise com identidade facial e cena (24 fotografias pequenas com rostos) | ≈ 6 s (inclui carregar os modelos) |
| Mudar desvios de câmara (reconstruir cronologia, sequências, momentos) | instantâneo na aplicação |

A análise por fotografia ficou mais pesada (identidade: até 12 rostos por fotografia;
classificador de cena ~40 ms). **Não medido** ainda num conjunto grande de 24 MP com estes
modelos; estimativa: +20–40 % em relação aos ≈ 330 ms/foto da Fase 3.

## Arquitetura

Novas decisões D-029 a D-033 em `ARCHITECTURE.md`: SFace com verificação de proveniência
por SHA-256; EfficientNet-Lite2 em vez de CLIP; momentos por regras explicáveis + linha
temporal + exemplos; correção de hora aplicada na leitura (EXIF intacto); arrastar com
eventos do rato (compatível com o WebView2 do Windows). Catálogo v5.

## Dependências novas

Nenhum crate novo. Modelos novos (não versionados, incluídos no instalador): SFace
(OpenCV Zoo, Apache-2.0, 37,8 MB) e EfficientNet-Lite2 (MediaPipe, Apache-2.0, 23,8 MB).
Total de modelos: ≈ 73 MB. Ver `LICENSES.md`.

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

Testar pessoas, momentos e sincronização com fotografias reais (nada é copiado para o
repositório; a pasta tem de ter `portrait.jpg` e `lena.jpg` ou outras duas pessoas):

```bash
MOMENTO_FACES_DIR=/caminho/para/fotos cargo test -p momento-project-management --release --test organize_flow -- --nocapture
```

## Testes realizados (2026-11-09)

| Verificação | Resultado |
|---|---|
| `cargo test --workspace` (com `MOMENTO_REF_DIR`, `MOMENTO_MP_PORTRAIT`, `MOMENTO_FACES_DIR`) | ✅ 131 passados, 1 ignorado (débito 24 MP, à parte) |
| `pnpm -r test` | ✅ 51 (45 da aplicação, 6 de traduções) |
| `pnpm -r typecheck`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ |
| Aplicação real (release, Linux): analisar, pessoas, identificar, pesquisas, sincronizar câmaras, momentos, arrastar | ✅ (3 bugs encontrados e corrigidos, com testes de regressão) |
| `cargo check --workspace --target x86_64-pc-windows-gnu` | ✅ |
| `cargo check` análise + codecs `--target aarch64-apple-darwin` | ✅ |
| macOS / Windows reais | ⏳ não executado |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Momentos imprecisos num casamento real | Alto | Nunca força; “Por classificar”; aprende com exemplos; afinar regras com um casamento real |
| Classes ImageNet não cobrem objetos de casamento | Médio | Pistas aproximadas + exemplos; avaliar um modelo zero-shot com licença verificável |
| Dados biométricos | Alto (RGPD) | Só locais, apagáveis, nunca em registos; documentado em `PRIVACY.md` |
| Instalador maior (+62 MB de modelos) | Baixo | Aceitável; modelos opcionais (a aplicação funciona sem eles) |

## Histórico de fases

| Fase | Estado | Data |
|---|---|---|
| 0 — Fundação técnica | ✅ | 2026-11-08 |
| 1 — Aplicação utilizável | ✅ | 2026-11-08 |
| 2 — Formatos e RAW | ✅ | 2026-11-08 |
| 3 — Seleção automática | ✅ | 2026-11-09 |
| 4 — Organização do casamento | ✅ | 2026-11-09 |
| 5 — Edição | ⏸ próxima | — |

## Próximo passo

**Recomendado:** importar um casamento real teu (com 2+ câmaras), analisar, identificar
noiva/noivo/pais, sincronizar as câmaras e ver quantas fotografias ficam em cada momento e
em “Por classificar”. Essa medição define o afinamento das regras dos momentos.

**Fase 5 — Edição:** revelação RAW não destrutiva, presets, sincronizar ajustes.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste
(casamento real, rajadas com olhos fechados, HEIC do iPhone); nome definitivo.
