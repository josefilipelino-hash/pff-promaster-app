# Estado do projeto

**Última atualização:** 2026-11-09 · **Fase atual:** 7 — Presets e consistência visual · **Estado:** ✅ concluída
**Versão:** 0.7.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

Sistema de **presets** completo (biblioteca global, componentes, Amount, 13 internos, formato
próprio com LUT, importação XMP aproximada com relatório, LUT `.cube`) e **consistência
visual**: grupos por luz semelhante, iluminação, câmara, cenário, horário, balanço de brancos
e flash/natural, e **sincronização inteligente** com uma fotografia de referência — cada
fotografia recebe o aspeto da referência e é ajustada **medindo o resultado** (exposição,
balanço, pele, contraste, cor), em vez de copiar os mesmos valores. Tudo não destrutivo.

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Criar preset (componentes escolhidos), guardar, renomear, organizar (grupos e ordem), duplicar (incl. internos), eliminar (com confirmação; internos protegidos) | Testes do armazenamento + interface + aplicação real |
| Importar/exportar `.momentopreset` (com as LUT), importar XMP e `.cube` | Testes (ida e volta noutra biblioteca; LUT log e `.lrtemplate` recusados) |
| Pré-visualizar ao passar o rato; aplicar a uma fotografia, seleção, momento, casamento completo | Interface + teste de integração (6 fotografias) + aplicação real ("Preset: Fine Art" no histórico) |
| Amount 0–150 % | Testes (0 = nada, 50 = a meio, 150 = exagero; curvas e LUT incluídas) |
| 13 presets internos com os nomes pedidos | Testes (todos diferentes; mantêm exposição e balanço; P&B só nos P&B) + folha de contactos numa fotografia real |
| Tonalização, P&B com mistura por cor, LUT `.cube` 1D/3D | Testes de render |
| XMP: interpretação aproximada com relatório (interpretado/ignorado e porquê) | Teste com um XMP do Lightroom; ver `docs/COMPATIBILIDADE-PRESETS.md` |
| Agrupamento: luz semelhante, iluminação (EV), câmara, cenário (momento), horário, balanço, flash/natural | Testes (7 critérios) + integração (exterior vs. interior quente separados) + aplicação real |
| Referência + sincronização (exposição, balanço, pele, contraste, cor, aspeto), em fila com progresso/pausa/cancelar | Teste: fotografia 1,3 EV mais escura com tungsténio fica igual à referência e **copiar os valores não chegava**; aplicação real com retratos em sombra fria e tungsténio |
| "Sincronizar" do editor: inteligente ou copiar valores | Interface |
| Originais intactos | Teste de integração (hash) |

## Desempenho medido (Linux x86_64, 4 núcleos, release)

| Operação | Resultado |
|---|---|
| Aplicar um preset a 6 fotografias (gravação) | instantâneo (miniaturas editadas em segundo plano) |
| Medir a luz de uma fotografia (miniatura) | ≈ 10–20 ms |
| Sincronizar uma fotografia com a referência (JPEG 1–2 MP) | ≈ 1–2 s (inclui carregar a fonte e ~10 renders de 384 px) |

Num RAW soma-se a revelação linear (≈ 1 s). Mac/Windows: **não medido**.

## Não funcional / limitações

- **XMP:** só importação e **aproximada** (motores diferentes); não exportamos XMP; perfis de
  câmara, máscaras, Kelvin absolutos, meios-tons do color grading e perfis de objetiva são
  ignorados (o relatório diz quais). `.lrtemplate` não suportado.
- **LUT:** só LUT criativas sRGB (domínio 0–1); LUT log/HDR recusadas.
- **Tons de pele na sincronização:** precisam dos rostos da análise; ajustam o HSL laranja e
  vermelho (também afetam outros objetos dessas cores, de forma limitada a ±20/±25).
- **Agrupamento por iluminação:** usa o EV do EXIF; fotografias sem EXIF ficam em "Sem dados
  de exposição". O flash vem do EXIF no início do ficheiro (CR3/RAF podem ficar "desconhecido").
  A dominante de cor é medida na miniatura **depois** do balanço da câmara (dominante residual).
- **Cenário** = momento (Fase 4); sem momentos atribuídos fica "Por classificar".
- Ainda: saída sRGB; sem máscaras; macOS/Windows não executados.

## Arquitetura

Decisões D-046 a D-051 em `ARCHITECTURE.md`. Novos módulos: `momento-presets` (componentes,
Amount, internos, ficheiro, XMP), `develop::lut`, `develop::consistency`,
`project-management::{consistency, preset_store}`; fila de fundo generalizada (auto/sync).
Catálogo v7 (`look_feature`), biblioteca v2 (`preset`), EXIF `Flash`. Nova permissão
`dialog:allow-save` (exportar presets).

## Dependências novas

Nenhuma biblioteca externa nova (o `blake3` já existia no projeto).

## Comandos exatos

```bash
pnpm install
```

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Ver os presets internos numa fotografia tua:

```bash
cargo run --release -p momento-presets --example preset_sheet -- foto.jpg folha.jpg
```

## Testes realizados (2026-11-09)

| Verificação | Resultado |
|---|---|
| `cargo test --workspace --release` (com RAW reais e referências) | ✅ 189 passados, 1 ignorado (débito de análise 24 MP) |
| `pnpm -r test` | ✅ 63 (57 da aplicação, 6 de traduções) |
| `pnpm -r typecheck`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ |
| Aplicação real (release, Linux): presets no editor (pré-visualização, aplicar, histórico), vista Consistência (medir, grupos, referência), sincronização em fila de 5 fotografias | ✅ — 4 defeitos encontrados e corrigidos: título do grupo combinado não era o da maioria; barra de ferramentas cortava "Fechar projeto"; aviso de pré-visualização por cima dos botões; "Exportação · Fase 7" na barra lateral |
| macOS / Windows reais | ⏳ não executado |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Expectativa de que XMP do Lightroom dê o mesmo aspeto | Médio | Relatório na importação, documento de compatibilidade, sem exportação XMP |
| Sincronização em fotografias muito diferentes (outra cena) | Médio | Agrupamento prévio por luz/cenário; cada sincronização pode ser desfeita |
| Pele em cenas sem análise | Baixo | Ignorada sem rostos (indicado na interface) |

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
| 8 — Exportação | ⏸ próxima | — |

## Próximo passo

**Recomendado:** num casamento teu, abrir **Consistência → Luz semelhante**, editar uma
fotografia de referência por grupo (ou aplicar um preset interno) e sincronizar; depois rever
a cerimónia e a festa e dizer-me onde a sincronização falha.

**Fase 8 — Exportação:** receitas e lotes, gestão de cor completa (perfis, soft proofing),
marca de água, metadados e privacidade.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
nome definitivo.
