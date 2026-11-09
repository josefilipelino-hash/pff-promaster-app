# Estado do projeto

**Última atualização:** 2026-11-09 · **Fase atual:** 8 — Edição localizada, retoque e retrato · **Estado:** ✅ concluída
**Versão:** 0.8.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

Edição **localizada e não destrutiva**: máscaras (pincel, gradientes, intervalos de
luminância/cor e máscaras de IA local — sujeito, pessoa, rosto, pele, cabelo, roupa, fundo,
objeto — e céu) combinadas por somar/subtrair/intersetar, com 13 ajustes locais; retoque
(corrigir, clonar, remoção simples de objetos, pó do sensor, olhos vermelhos) e retrato
subtil por rosto (pele, olheiras, brilho, dentes, olhos). Tudo fica na receita e pode ser
alterado ou removido a qualquer momento; os originais nunca são escritos.

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Pincel (tamanho, suavidade, fluxo, apagar), gradiente linear e radial | Testes (fluxo acumula, apagar, borda suave) + interface |
| Intervalo de luminância e de cor | Testes (cinzento não entra na cor) + interface (cor amostrada no clique) |
| Sujeito, pessoa, rosto, pele, cabelo, roupa, fundo, objeto (clique) | Teste com retrato real (coberturas coerentes; sujeito = pessoa; objeto = pessoa clicada) + aplicação real (máscara "Pessoa" com bordas limpas) |
| Céu (heurística) | Testes; RAW Canon 7D com céu nublado: 38 % (árvore e edifício fora); interior: 0 % |
| Somar, subtrair, intersetar, inverter (parte e máscara), densidade, suavidade | Testes + interface |
| 13 ajustes locais (exposição … ruído) | Testes de render (só dentro da máscara) + aplicação real (+1 EV na pessoa, fundo igual) |
| Máscaras acompanham recorte/endireitar; ver a máscara a vermelho (O) sem mexer no histograma | Testes |
| Corrigir (heal) e clonar com origem automática, remoção de objetos, pó do sensor, olhos vermelhos | Testes (heal iguala o fundo onde o clone traria o escuro; objeto removido) + interface |
| Retrato: pele (separação de frequências, mantém poros), olheiras, brilho, dentes, olhos | Testes (contorno e fora do rosto iguais; 0 = original) + retrato real (`examples/portrait_eval`) + aplicação real |
| Tudo removível/alterável; originais intactos | Testes (remover = render idêntico ao anterior; hash dos originais) |

## Desempenho medido (Linux x86_64, 4 núcleos, release)

| Operação | Resultado |
|---|---|
| Máscara de IA (pessoa/cabelo/…; segmentação + refinamento 1024 px) | ≈ 0,5–1 s, uma vez (fica guardada) |
| Máscara de objeto por clique | ≈ 1 s |
| Deteção de rostos para o retrato | ≈ 0,3–0,6 s |
| Render com várias máscaras | +10–30 % por máscara com ajustes espaciais; as restantes quase sem custo |

Mac/Windows: **não medido**.

## Não funcional / limitações

- **Máscaras de IA:** o modelo multiclasse trabalha a 256 px (bordas refinadas pela
  fotografia); em grupos grandes, pessoas pequenas podem ficar de fora (corrigir com pincel).
  "Sujeito" = pessoas; sem pessoas, o objeto ao centro.
- **Céu:** heurística (sem modelo): falha em céus noturnos, muito escuros, ou que não tocam
  no topo; nesses casos fica vazio e pode-se pintar.
- **Remoção de objetos:** copia uma zona vizinha (não gera conteúdo novo): só objetos
  pequenos em fundos simples.
- **Edição das formas já desenhadas:** gradientes e traços não têm pegas para mover depois
  de criados (apaga-se a parte e desenha-se de novo); a origem de cada mancha é automática.
- **Retrato:** precisa dos modelos de rostos; perfis e rostos muito pequenos podem não ser
  detetados. Sem deformações (por desenho).
- Ainda: saída sRGB; macOS/Windows não executados.

## Arquitetura

Decisões D-052 a D-056. Novos módulos: `develop::{local, portrait, sky}`,
`face-analysis::segment`, `project-management::local_edit`; blocos de receita `masks` e
`portrait`, `retouch.patches`, heal nas manchas. Modelos novos: `segment_multiclass` e
`segment_object` (MediaPipe, Apache-2.0).

## Dependências novas

Nenhuma biblioteca nova (blake3 já existia). Dois modelos novos (ver `LICENSES.md`).

## Comandos exatos

```bash
.venv-models/bin/python scripts/prepare-models.py --only segment_multiclass segment_object
```

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Avaliar o retrato numa fotografia tua (nada é escrito no original):

```bash
cargo run --release -p momento-project-management --example portrait_eval -- foto.jpg rosto.jpg 35 40 40 40 30
```

## Testes realizados (2026-11-09)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (com amostras RAW, modelos e rostos) | ✅ 201 passaram, 0 falharam, 1 ignorado (débito 24 MP, manual) |
| `pnpm -r test` (Vitest) | ✅ 60 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ limpos |
| Aplicação real (build release, Linux/Xvfb) | ✅ menu de máscaras, máscara "Pessoa" + 1 EV só na pessoa, céu no RAW 7D, retrato detetado e aplicado, ver máscara (O) |
| Avaliação visual do retrato (`portrait_eval`) | ✅ subtil; poros e rugas mantidos; contorno intacto |

Novos testes: `develop::local` (formas, combinação, pincel), `develop/local_tests.rs` (5),
`develop::sky`, `face-analysis::segment`, `tests/local_edit_flow.rs`.

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Máscaras de IA imperfeitas em grupos/cenas complexas | Médio | Somar/subtrair com pincel; refinamento pelas arestas |
| Retoque de pele percebido como artificial | Médio | Limites rígidos, poros mantidos, valores por omissão 0, avaliação visual |
| Desempenho com muitas máscaras espaciais em portáteis | Baixo | Só no render final; GPU na fase de otimização |

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
| 9 — Exportação | ⏸ próxima | — |

## Próximo passo

**Recomendado:** num casamento teu, experimentar máscara "Pessoa" + exposição, "Céu" nas
fotografias de exterior e o retrato com valores baixos (20–40) em retratos; dizer-me onde as
máscaras falham.

**Fase 9 — Exportação:** receitas e lotes, gestão de cor completa (perfis, soft proofing),
marca de água, metadados e privacidade.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
nome definitivo.
