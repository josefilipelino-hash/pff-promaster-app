/**
 * Navegação completa por teclado (Fase 14), além dos atalhos de cada vista:
 * - F6 / Shift+F6: passa entre as regiões (barra superior, barra lateral, área principal,
 *   painel de informação, editor) — como nas aplicações nativas;
 * - setas/Home/End em separadores (`role="tablist"`): muda de separador e ativa-o.
 */
const REGIONS = [".toolbar", ".sidebar", ".grid-scroller, .viewer, .develop-main, .album-stage, .main", ".info-panel, .develop-panel, .album-side.right"];

const FOCUSABLE = 'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

function visible(el: Element): el is HTMLElement {
  return el instanceof HTMLElement && el.getClientRects().length > 0 && !el.closest("[hidden]");
}

/** Regiões presentes, pela ordem de leitura. */
export function regions(root: ParentNode = document): HTMLElement[] {
  const out: HTMLElement[] = [];
  for (const sel of REGIONS) {
    const all = Array.from(root.querySelectorAll(sel));
    // Sem motor de layout (testes), nenhum elemento tem caixas: usa-se o primeiro.
    const el = all.find(visible) ?? (all.every((e) => e.getClientRects().length === 0) ? all[0] : undefined);
    if (el instanceof HTMLElement && !out.includes(el)) out.push(el);
  }
  return out;
}

export function focusRegion(dir: 1 | -1, root: Document = document): HTMLElement | null {
  const rs = regions(root);
  if (rs.length === 0) return null;
  const cur = rs.findIndex((r) => r.contains(root.activeElement));
  const next = rs[(cur + dir + rs.length) % rs.length]!;
  const target = (next.matches(FOCUSABLE) ? next : next.querySelector<HTMLElement>('[aria-current="true"], .active, [aria-selected="true"]')) ?? next.querySelector<HTMLElement>(FOCUSABLE);
  if (target) target.focus();
  else {
    next.tabIndex = -1;
    next.focus();
  }
  return next;
}

function onTabs(e: KeyboardEvent): boolean {
  const tab = (e.target as HTMLElement | null)?.closest?.('[role="tab"]');
  const list = tab?.closest('[role="tablist"]');
  if (!tab || !list) return false;
  const tabs = Array.from(list.querySelectorAll<HTMLElement>('[role="tab"]:not(:disabled)'));
  const i = tabs.indexOf(tab as HTMLElement);
  const to = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: tabs.length - 1 }[e.key];
  if (to === undefined) return false;
  const t = tabs[(to + tabs.length) % tabs.length]!;
  t.focus();
  t.click();
  return true;
}

export function installKeyboardNav(): void {
  window.addEventListener(
    "keydown",
    (e) => {
      if (e.key === "F6" && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        focusRegion(e.shiftKey ? -1 : 1);
      } else if (onTabs(e)) {
        e.preventDefault();
        e.stopPropagation();
      }
    },
    true,
  );
}
