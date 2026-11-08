/**
 * Seleção na grelha (lógica pura, sem React):
 * - clique: seleciona só esse item;
 * - Cmd/Ctrl+clique: alterna o item;
 * - Shift+clique: intervalo desde a âncora.
 */
export interface Selection {
  ids: Set<number>;
  /** Item com foco (alvo do teclado e do painel de informação). */
  focus: number | null;
  /** Âncora para seleções em intervalo. */
  anchor: number | null;
}

export const EMPTY_SELECTION: Selection = { ids: new Set(), focus: null, anchor: null };

export function clickSelect(
  sel: Selection,
  order: number[],
  id: number,
  mods: { toggle: boolean; range: boolean },
): Selection {
  if (mods.range && sel.anchor !== null) {
    const a = order.indexOf(sel.anchor);
    const b = order.indexOf(id);
    if (a >= 0 && b >= 0) {
      const [lo, hi] = a < b ? [a, b] : [b, a];
      const range = order.slice(lo, hi + 1);
      const ids = mods.toggle ? new Set([...sel.ids, ...range]) : new Set(range);
      return { ids, focus: id, anchor: sel.anchor };
    }
  }
  if (mods.toggle) {
    const ids = new Set(sel.ids);
    if (ids.has(id)) ids.delete(id);
    else ids.add(id);
    return { ids, focus: id, anchor: id };
  }
  return { ids: new Set([id]), focus: id, anchor: id };
}

/** Move o foco `delta` posições; com `extend`, estende a seleção desde a âncora. */
export function moveFocus(sel: Selection, order: number[], delta: number, extend: boolean): Selection {
  if (order.length === 0) return EMPTY_SELECTION;
  const cur = sel.focus === null ? -1 : order.indexOf(sel.focus);
  const start = cur < 0 ? (delta > 0 ? -1 : order.length) : cur;
  const next = Math.max(0, Math.min(order.length - 1, start + delta));
  const id = order[next]!;
  if (extend) return clickSelect(sel.anchor === null ? { ...sel, anchor: id } : sel, order, id, { toggle: false, range: true });
  return { ids: new Set([id]), focus: id, anchor: id };
}

export function selectAll(order: number[], focus: number | null): Selection {
  return { ids: new Set(order), focus: focus ?? order[0] ?? null, anchor: focus ?? order[0] ?? null };
}

/** IDs aos quais aplicar uma classificação: a seleção, se o foco fizer parte dela. */
export function targets(sel: Selection): number[] {
  if (sel.focus !== null && !sel.ids.has(sel.focus)) return [sel.focus];
  return [...sel.ids];
}

/** Mantém apenas IDs visíveis (após mudar o filtro). */
export function restrict(sel: Selection, visible: Set<number>): Selection {
  const ids = new Set([...sel.ids].filter((i) => visible.has(i)));
  const focus = sel.focus !== null && visible.has(sel.focus) ? sel.focus : null;
  return { ids, focus, anchor: focus };
}
