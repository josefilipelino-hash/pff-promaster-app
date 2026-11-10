# Estado do projeto

**Última atualização:** 2026-11-10 · **Fase atual:** 14 — UX/UI profissional · **Estado:** ✅ concluída
**Versão:** 0.14.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

A interface passa a ter um sistema de design próprio: tema escuro profissional (principal)
e claro opcional, tokens de cor/tipografia/espaço, 48 ícones próprios, estados
uniformes (hover, focus, selecionado, desativado, progresso), menus contextuais com os
atalhos visíveis, navegação por teclado completa, tira de fotografias, barras compactas que
se adaptam à largura e estados claros (a carregar, a processar, offline, falhou,
concluído). Nenhuma funcionalidade foi substituída por maquetas e o desempenho com 15 000
fotografias não regrediu. Referência: **`DESIGN.md`**.

## Funcional (verificado)

| Pedido | Como | Verificação |
|---|---|---|
| Tema escuro principal + claro opcional | Tokens em `packages/ui`, aplicados como variáveis CSS; "como o sistema" | Aplicação real nos dois temas; teste de troca |
| Tipografia, espaçamento, forma | Escalas únicas (6 tamanhos, 6 espaços, 3 raios, controlos 28/24 px) | Revisão de todos os ecrãs |
| Iconografia | 48 ícones SVG próprios, `currentColor`, traço constante | Aplicação real a 1× e 2× |
| Menus | Importar ▾, Mais ⋯ (atalhos, tema, fechar), menu contextual da fotografia | Testes (rato + teclado); aplicação real |
| Estados hover/focus/selected/disabled/progress | Regras únicas; foco só com teclado; desativado 45 % | Revisão; testes de foco |
| Estados loading/processing/offline/failed/completed | Classe `.state` na barra de estado, fila de exportação e sobre a fotografia | Teste da barra de estado; aplicação real |
| Atalhos visíveis sem poluir | Nos menus (à direita), dicas `kbd` na comparação, títulos dos botões; símbolos do sistema | Testes |
| Menus contextuais | Clique direito, tecla Menu, Shift+F10 — ações reais | Testes; aplicação real |
| Keyboard navigation completa | F6 entre regiões, setas nos separadores, menus, foco devolvido | Testes |
| Filmstrip | Virtualizada no visualizador e Develop, atual centrada, marcas, offline | Teste com 3000; aplicação real |
| Comparação | Barra compacta, dicas de teclado, estados de carregamento | Aplicação real |
| Retina/HiDPI | Vetores; imagens e histograma em píxeis físicos | Aplicação real com `GDK_SCALE=2` (3200×2000) |
| Acessibilidade | axe-core WCAG 2.x A/AA em 11 ecrãs/estados: 0 violações; contraste testado | Testes |
| Sem animações desnecessárias | Só cor 120 ms; shimmer removido; `prefers-reduced-motion` | Revisão |
| Performance | 15 000 fotografias: scroll 59–97 FPS (p95 16–18 ms, 0 frames longos), grelha pronta 247–291 ms | `MOMENTO_PERF=1`, aplicação real |

## Problemas encontrados (corrigidos)

1. Barra superior cortada (título e ações escondidos) com o painel de informação aberto: a
   adaptação usava a largura da janela → *container queries* pela largura real da barra.
2. Barra do visualizador a transbordar ("Ecrã completo" cortado) → ícones.
3. "Edição automática concluída: 0 fotografias" ao fechar um projeto sem fila → o núcleo
   já não anuncia nada sem fila (teste novo) e a interface ignora filas vazias.
4. Tema claro: estrelas vazias pareciam preenchidas, avisos amarelos ilegíveis em branco,
   cores fixas em avisos e notificações → tokens semânticos.
5. Acessibilidade (axe): tira sem `listitem`, `aria-selected` inválido nas molduras do
   álbum, `dt/dd` fora de `<dl>`.
6. Texto em inglês na interface portuguesa ("Amount (%)") e termos com equivalente.
7. Margem a mais no painel "Seleção automática"; opacidades de "desativado" diferentes.

## Não funcional / limitações

- macOS e Windows **não executados**: fontes do sistema (SF / Segoe UI Variable), Retina e
  escalas 150/200 % do Windows por verificar.
- Leitores de ecrã reais (VoiceOver, Narrador, NVDA) não testados; a auditoria é
  automática.
- Editor de álbum: arrastar e largar sem alternativa completa por teclado.
- Menus na webview; o menu nativo da barra do macOS fica para a distribuição (D-096).
- Caixas de verificação e listas pendentes nativas do sistema (cor de destaque aplicada).

## Arquitetura

Decisões D-093 a D-098. Novos: `packages/ui/src/tokens.ts` (tokens + `contrast()`),
`logic/{theme, keys, keyboardNav}.ts`, `components/{Icon, Menu, Filmstrip}.tsx`. Sem
mudanças de arquitetura nem de funcionalidades principais.

## Dependências novas

`axe-core` 4.10.2 (MPL-2.0), só em testes — não é distribuído. Ver `LICENSES.md`.

## Comandos exatos

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Só a auditoria de acessibilidade:

```bash
pnpm --filter @momento/desktop exec vitest run -t acessibilidade
```

## Testes realizados (2026-11-10)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (com amostras RAW, modelos e rostos) | ✅ 268 passaram, 0 falharam, 1 ignorado (débito 24 MP, manual) |
| `pnpm -r test` (Vitest) | ✅ 88 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy` | ✅ limpos |
| Aplicação real (release, Linux/Xvfb) | ✅ todos os ecrãs nos dois temas, menus, tira, HiDPI 2×, desempenho com 15 000 |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Aspeto diferente em macOS/Windows (fontes, controlos nativos) | Médio | Fontes do sistema na pilha; tokens independentes da plataforma; verificar na CI com capturas |
| Contraste real diferente do calculado (perfis de ecrã) | Baixo | Margens acima do mínimo WCAG |

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
| 15 — Distribuição | ⏸ próxima | — |

## Próximo passo

**Recomendado:** num Mac (Retina), abrir um casamento real e experimentar o tema escuro e o
claro, o menu contextual (clique direito / Shift+F10), a tira no Develop e F6 entre
regiões.

**Fase 15 — Distribuição:** GPU (wgpu), Apple Silicon, menu nativo do macOS, instaladores
assinados.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
medidas do laboratório; nome definitivo.
