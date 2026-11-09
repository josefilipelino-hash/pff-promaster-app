# Estado do projeto

**Última atualização:** 2026-11-09 · **Fase atual:** 5 — Motor de edição · **Estado:** ✅ concluída
**Versão:** 0.5.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

A aplicação tem um motor de edição **100 % não destrutivo**: os ajustes ficam no catálogo
(com histórico completo e instantâneos) e numa cópia JSON na pasta do projeto. Os ficheiros
originais são abertos só para leitura (verificado por hash nos testes).

- Um só motor em Rust (`f32`, Rec. 2020 linear → sRGB) para o ecrã, as miniaturas, as
  previews e a resolução total: a cor no ecrã é a mesma dos ficheiros.
- Os RAW são editados a partir dos **dados lineares do sensor** (LibRaw), não da preview
  embutida.
- **Qualidade dinâmica:** ao arrastar um controlo, render rápido; 220 ms depois de parar,
  render final à resolução do ecrã.
- Modo **Revelar** (tecla D): painel de ajustes, histograma RGB, histórico, instantâneos,
  antes/depois, recorte, conta-gotas, copiar/colar/sincronizar e cópias virtuais.

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Luz: exposição, contraste, realces, sombras, brancos, pretos | Testes de renderização (cada controlo mexe na zona certa) + aplicação real (RAW D810) |
| Balanço: temperatura, matiz, conta-gotas | Testes (o conta-gotas neutraliza um cinzento com dominante azul) + interface |
| Cor: saturação, vibração; HSL (matiz/saturação/luminância por 8 cores) | Testes (saturação −100 = cinzento; mexer no azul não mexe no vermelho) |
| Curva paramétrica, curva de pontos RGB e por canal (R, G, B) | Testes (curva invertida, só o vermelho, zonas) + editor de curvas na interface |
| Detalhe: nitidez (quantidade, raio, detalhe, máscara), ruído de luminância e de cor | Testes (nitidez ↑ detalhe, redução ↓ ruído, máscara protege zonas lisas) |
| Efeitos: textura, clareza, remover névoa, vinheta, grão | Testes (grão determinístico; dehaze aumenta o contraste de uma imagem enevoada) |
| Geometria: recorte, proporções, 90°, espelho, endireitar, endireitar automático, perspetiva vertical/horizontal | Testes (sem cantos vazios; horizonte inclinado 4° corrigido para < 0,8°) + aplicação real |
| Objetiva: distorção, aberração cromática, vinheta | Testes (a aberração mexe sobretudo no vermelho) |
| Desfazer, refazer, histórico, repor, antes/depois | Base de dados + interface + aplicação real |
| Copiar / colar / sincronizar (geometria fora por omissão) | Sessão + interface |
| Cópias virtuais (classificação e edição independentes), instantâneos | Base de dados + sessão + interface |
| Histograma RGB atualizado a cada render | Testes + aplicação real |
| Receita neutra = original | Teste (±1 nível em 8 bits) |
| Miniaturas, previews e resolução total editadas (sem ampliar) | Teste com RAW real + grelha da aplicação real |
| Originais intactos | Teste: hash de todos os originais igual antes e depois de editar |

## Desempenho medido (Linux x86_64, 4 núcleos, release)

| Operação | Resultado |
|---|---|
| Arrastar a exposição: render rápido de 1024 px a partir de uma fonte de 6 MP | **≈ 35 ms** (~28 fps) |
| Render rápido de 1024 px com todos os ajustes ligados | ≈ 110 ms |
| Render final de 2048 px com todos os ajustes | ≈ 0,5 s (só depois de parar) |
| Abrir um RAW de 36 MP para editar (revelação linear a meia resolução) | ≈ 1 s |
| Render seguinte do RAW (fonte em memória) | ≈ 45–50 ms |
| Resolução total editada de um RAW de 36 MP (zoom 100 %) | ≈ 8 s, uma vez (fica em cache) |

Num Mac com 8–10 núcleos espera-se 2× mais rápido (rayon), **não medido**.

## Não funcional / limitações

- **Máscaras e ajustes locais:** não implementados (pedido: não avançar ainda).
- **Gestão de cor:** consistente de ponta a ponta, mas a saída é **sRGB**. Não há ainda
  conversão para o perfil do monitor (P3) nem LittleCMS: as cores fora de sRGB são cortadas
  na pré-visualização. Fica para a Fase 6 (exportação e gestão de cor).
- **Balanço de brancos relativo** (−100…+100) também nos RAW; não mostra Kelvin absolutos.
  Em RAW é aplicado depois da matriz da câmara (aproximação).
- **Correções de objetiva manuais:** sem perfis automáticos (Lensfun não integrado).
- **Perspetiva:** modelo trapezoidal (não uma homografia completa); sem guias desenhadas.
- **Zoom 100 % dentro do modo Revelar:** não há; o editor mostra a fotografia ajustada ao
  ecrã. A nitidez e o ruído avaliam-se no visualizador normal (que mostra a versão editada
  em resolução total).
- XMP compatível com outros programas: não escrito (só o sidecar JSON próprio, na pasta
  do projeto).
- macOS/Windows: compilação verificada, **não executada** nesses sistemas.

## Arquitetura

Novas decisões D-034 a D-039 em `ARCHITECTURE.md` (§5 atualizada): motor único em Rust;
RAW a partir dos dados lineares; qualidade dinâmica com guia de baixa resolução; frames
binários; receita por variante + sidecar JSON; chave de fotografia para cópias virtuais.
Catálogo v6 (`edit`, `edit_history`, `snapshot`). Nova função na camada da LibRaw:
`mlr_develop_linear`.

## Dependências novas

Nenhuma biblioteca externa nova (rayon e serde já estavam no projeto).

## Comandos exatos

```bash
git submodule update --init --recursive
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

Testar a edição com um RAW teu (nada é copiado para o repositório):

```bash
MOMENTO_SAMPLES_DIR=/pasta/com/um/raw cargo test -p momento-project-management --release --test edit_flow -- --nocapture
```

## Testes realizados (2026-11-09)

| Verificação | Resultado |
|---|---|
| `cargo test --workspace` (com amostras e referências) | ✅ 161 passados, 1 ignorado (débito de análise 24 MP) |
| `pnpm -r test` | ✅ 56 (50 da aplicação, 6 de traduções) |
| `pnpm -r typecheck`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ |
| Aplicação real (release, Linux) com RAW Nikon D810: abrir no editor, arrastar exposição, realces/sombras/vibração, histórico, recorte, antes/depois, miniatura editada na grelha | ✅ (1 bug encontrado e corrigido: atalhos ignorados com o foco num deslizador) |
| `cargo check --workspace --target x86_64-pc-windows-gnu` | ✅ |
| `cargo check` edição + análise + codecs `--target aarch64-apple-darwin` | ✅ |
| macOS / Windows reais | ⏳ não executado |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Cor só em sRGB (sem perfil do monitor) | Médio | LittleCMS e saída por perfil na Fase 6 |
| Desempenho em portáteis fracos com muitos ajustes | Médio | Render rápido mais pequeno; GPU (wgpu) na Fase 9 |
| Aspeto dos controlos diferente de outros programas | Baixo | Os valores não são compatíveis com Lightroom; sem XMP partilhado por agora |

## Histórico de fases

| Fase | Estado | Data |
|---|---|---|
| 0 — Fundação técnica | ✅ | 2026-11-08 |
| 1 — Aplicação utilizável | ✅ | 2026-11-08 |
| 2 — Formatos e RAW | ✅ | 2026-11-08 |
| 3 — Seleção automática | ✅ | 2026-11-09 |
| 4 — Organização do casamento | ✅ | 2026-11-09 |
| 5 — Motor de edição | ✅ | 2026-11-09 |
| 6 — Exportação | ⏸ próxima | — |

## Próximo passo

**Recomendado:** editar uma sequência de um casamento teu (RAW da tua câmara): ajustar uma
fotografia, sincronizar com as restantes, criar uma cópia virtual a preto e branco, e
avaliar a velocidade dos deslizadores no teu Mac/PC.

**Fase 6 — Exportação:** receitas e lotes, gestão de cor completa (perfis, soft proofing),
marca de água, metadados e privacidade.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
nome definitivo.
