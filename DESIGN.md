# Design e UX — Momento Studio

Referência do sistema de design (Fase 14). Objetivo: interface comparável ao software
profissional atual de fotografia — **premium, minimalista, rápida, elegante, sem poluição
visual, fácil de aprender** — sem trocar nenhuma funcionalidade real por maquetas e sem
custo de desempenho.

## Princípios

1. **A fotografia é o centro.** Cinzentos neutros (sem dominante de cor) para não
   influenciar a perceção da cor; a área da fotografia é sempre escura, mesmo no tema claro.
2. **Destaque raro.** O dourado (`--accent`) só marca a ação principal, a seleção e o foco.
3. **Mostrar o necessário, revelar o resto.** Ações secundárias em menus (Importar, Mais,
   menu contextual); rótulos de botões encolhem para ícones quando falta espaço; ações de
   cartão aparecem ao passar/focar.
4. **Teclado em primeiro lugar.** Tudo o que se faz com o rato faz-se com o teclado; os
   atalhos estão visíveis onde a ação está (menus, dicas), não num painel permanente.
5. **Sem animações decorativas.** Só transições de cor de 120 ms em hover/focus;
   `prefers-reduced-motion` desliga-as. Nada de shimmer, deslizes ou zooms animados.
6. **Estados explícitos.** Carregar, processar, offline, falhou, concluído têm sempre a
   mesma forma (ponto colorido + texto) e são anunciados aos leitores de ecrã.

## Tokens (`packages/ui/src/tokens.ts`)

Aplicados como variáveis CSS no `:root` por `logic/theme.ts`; nenhum componente usa cores
fixas da interface (exceções intencionais: simulação de papel e guias de impressão do
álbum, e as camadas sobre a fotografia, que é sempre escura).

| Grupo | Tokens |
|---|---|
| Superfícies | `--bg`, `--surface`, `--surface-2`, `--surface-3`, `--canvas` (fotografia), `--border`, `--border-strong` |
| Texto | `--text`, `--text-muted`, `--text-faint` |
| Destaque | `--accent`, `--accent-strong`, `--accent-soft`, `--on-accent`, `--focus` |
| Semânticas | `--danger`, `--warn`, `--success`, `--info` |
| Interação | `--hover`, `--pressed`, `--overlay`, `--shadow-pop` |
| Tipografia | `--font` (fonte do sistema: SF no macOS, Segoe UI Variable no Windows); `--fs-xs` 11 · `--fs-sm` 12 · `--fs-md` 13 · `--fs-lg` 15 · `--fs-xl` 18 · `--fs-2xl` 24 px |
| Espaço | `--sp-1`…`--sp-6`: 4 · 8 · 12 · 16 · 24 · 32 px |
| Forma | `--r-sm` 4 · `--r-md` 6 · `--r-lg` 10 px; controlos 28 px (24 px pequenos); barra 48 px |
| Movimento | `--dur` 120 ms (só cor/borda) |

**Contraste (WCAG 2.x, testado em `logic.test.ts` nos dois temas):** texto ≥ 7:1;
texto secundário e terciário ≥ 4,5:1 sobre `--bg`, `--surface` e `--surface-2`; texto
sobre o destaque ≥ 4,5:1; destaque sobre o fundo ≥ 3:1 (elementos gráficos).

## Temas

- **Escuro (principal, por omissão).** Fundo #161616; superfícies em degraus discretos (#1e1e1e, #262626, #303030).
- **Claro (opcional).** Mesmos papéis, destaque escurecido (#8a6424) para manter contraste;
  dourado de classificação e cinzento de rejeição próprios.
- **Como o sistema.** Segue `prefers-color-scheme` em tempo real.
- Escolha em **Mais (⋯) → Tema**; guardada localmente (`momento.theme`), aplicada antes do
  primeiro desenho (sem clarão).

## Componentes e estados

| Elemento | Repouso | Hover | Focus (teclado) | Selecionado / ativo | Desativado |
|---|---|---|---|---|---|
| Botão | superfície 2 + borda | `--hover` | anel 2 px `--focus` | `aria-pressed`: superfície 3, texto forte | 45 % de opacidade, sem cursor |
| Botão principal | `--accent` | `--accent-strong` | anel | — | 45 % |
| Botão fantasma | transparente | `--hover` | anel | — | 45 % |
| Item de menu | transparente | `--hover` | `--accent-soft` | ✓ (checkbox) | 45 % |
| Célula da grelha | — | — | anel | selecionada: borda clara + fundo; atual (foco): borda `--accent` | — |
| Campo | `--surface` + borda | borda forte | borda `--accent` + halo suave | — | 45 % |

O foco só aparece com teclado (`:focus-visible`); com o rato não há anéis.

**Iconografia** (`components/Icon.tsx`): 48 ícones próprios em SVG, traço de 1,5 px numa
grelha de 16 (traço visual constante em tamanhos maiores), `currentColor`, decorativos
(`aria-hidden`) — o nome acessível está sempre no botão. Sem bibliotecas externas.

**Estados claros** (classe `.state` + variante): `loading` e `processing` (azul),
`offline` (âmbar), `failed` (vermelho), `completed` (verde). Usados na barra de estado
(importar, procurar ficheiros, miniaturas, fotografias offline e em falta), na fila de
exportação (na fila, a exportar, em pausa, concluída, falhou) e sobre a fotografia
(a carregar resolução total, a preparar o editor, não foi possível mostrar). A barra de
estado é uma região `role="status"` (anunciada sem interromper). Na grelha: miniatura
pendente (anel estático), falhada, offline e em falta.

**Estados vazios:** ícone em círculo + título + uma frase + (se houver) a ação seguinte —
projeto sem fotografias, filtro sem resultados, sequências, pessoas, consistência, álbum.

## Ecrãs

| Ecrã | O que mudou |
|---|---|
| Início | Cabeçalho alinhado; ações com ícone; "Remover da lista" só ao passar/focar |
| Barra superior | Uma linha: projeto · vistas (segmentado com ícones) · Comparar, Presets, Auto, **Exportar** · Importar ▾ · Mais ⋯. Adapta-se à largura com *container queries* (≤ 1240 px só ícones nas vistas; ≤ 980 px só ícones nas ações) |
| Culling / Library (grelha) | Menu contextual; atalhos visíveis no menu; estados offline/falha |
| Visualizador | Barra compacta (← → como ícones, Ajustar/100 % segmentado, − % +, ecrã completo); **tira de fotografias** |
| Comparação | Barra fantasma; dicas de teclado discretas (`← →` painel, `P X 0–5` classificar) |
| Develop | Barra de ícones com rótulos adaptativos; tira de fotografias; histograma nítido em HiDPI |
| People, Gallery, Album, Export | Tokens, estados vazios, avisos com cores semânticas legíveis nos dois temas |

**Tira de fotografias** (`Filmstrip.tsx`): virtualizada (só as visíveis existem no DOM —
15 000 fotografias não criam 15 000 elementos), fotografia atual centrada sem animação,
marcas (♥ ✕ ★) e ponto de offline, clique muda de fotografia, menu contextual.

## Teclado

| Tecla | Ação |
|---|---|
| F6 / Shift+F6 | Saltar entre regiões: barra superior → barra lateral → área principal → painel |
| ← → ↑ ↓ | Navegar fotografias; em listas de separadores muda de separador (Home/End) |
| Enter · Esc | Abrir · voltar |
| Menu / Shift+F10 | Menu contextual da fotografia com foco |
| P · X · U · 0–5 | Favorita · rejeitar · sem marca · estrelas |
| D · C · G · Z · F | Revelar · comparar · grelha · zoom · ecrã completo |
| ⌘/Ctrl+Shift+I · ⌘/Ctrl+Shift+E | Importar · exportar |
| Delete | Remover… (sempre com confirmação) |
| ? | Todos os atalhos |

Menus: ↑ ↓ Home End, primeira letra, Enter/Espaço, Esc/Tab fecham e devolvem o foco a
quem os abriu. Os atalhos mostram-se com os símbolos de cada sistema (⇧⌘E no macOS,
Ctrl+Shift+E no Windows).

## Retina / HiDPI

- Ícones e interface em vetor/CSS: nítidos em qualquer densidade.
- Imagens: o visualizador e o editor calculam o tamanho em píxeis físicos
  (`devicePixelRatio`); o histograma redimensiona o buffer do canvas para
  tamanho no ecrã × `devicePixelRatio`.
- Miniaturas de 640 px: nítidas a 2× em células até 320 px.
- **Verificado** na aplicação real (Linux, `GDK_SCALE=2`, ecrã 3200×2000): disposição igual
  à de 1×, texto, ícones e histograma nítidos. Por verificar em ecrãs Retina (macOS) e
  Windows com escala 150/200 %.

## Acessibilidade — análise

| Área | Estado |
|---|---|
| Auditoria automática (axe-core 4.10, regras WCAG 2.0/2.1 A e AA) | Início, grelha, menu contextual, visualizador com tira, Develop, comparação, pessoas, álbum, ajuda, exportar, galeria: **0 violações** (teste `acessibilidade (Fase 14)`) |
| Problemas que a auditoria encontrou (corrigidos) | Tira com `role="list"` sem `listitem`; molduras do álbum com `aria-selected` sem papel válido; `dt/dd` fora de `<dl>` no painel de informação |
| Contraste | Testado nos tokens (ver acima); o jsdom não calcula cores renderizadas, por isso a regra `color-contrast` do axe está desligada e coberta pelo teste dos tokens |
| Nomes acessíveis | Todos os botões só com ícone têm `aria-label`; ícones decorativos `aria-hidden` |
| Papéis | `toolbar`, `menu`/`menuitem`/`menuitemcheckbox`, `dialog`, `list`, `region`, `status`, `alert`, `slider` |
| Foco | Visível só com teclado; menus e diálogos devolvem o foco; regiões por F6 |
| Movimento | Sem animações; `prefers-reduced-motion` respeitado |
| Limitações conhecidas | Não testado com VoiceOver (macOS) nem Narrador/NVDA (Windows); o editor de álbum é sobretudo por rato (setas+Shift movem a moldura selecionada); arrastar e largar sem alternativa por teclado em todos os casos |

## Desempenho

A interface não pode custar desempenho. Medido na aplicação real com o projeto de 15 000
fotografias (`MOMENTO_PERF=1`, ver `PERFORMANCE.md`):

| Medição | Fase 12 | Fase 14 |
|---|---|---|
| Lançamento até à interface pronta | 778–800 ms | 767–804 ms |
| Grelha pronta ao abrir o projeto (a quente) | 242 ms | 247–291 ms |
| Scroll (janelas de 3 s) | 67–75 FPS, p95 18–25 ms | 59–97 FPS, p95 16–18 ms (uma janela no fim do scroll: p95 27) |
| Frames > 50 ms no scroll | 0–1 | 0 |

Sem regressão. O que o garante: nenhuma animação contínua, nenhum filtro/blur CSS, tira
virtualizada, menus montados só quando abertos, ícones inline sem pedidos de rede.
