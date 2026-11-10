import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

/**
 * Menu (Fase 14): menus contextuais e menus de botões, com o mesmo aspeto e teclado.
 * - abre num ponto (clique direito, tecla Menu/Shift+F10, ou por baixo de um botão);
 * - foca o primeiro item; ↑ ↓ Home End e a primeira letra navegam; Enter/Espaço ativam;
 * - Esc, Tab ou clique fora fecham e devolvem o foco a quem o abriu;
 * - cada ação pode mostrar o seu atalho de teclado (discreto, à direita).
 */
export type MenuEntry =
  | { kind?: "item"; label: string; onSelect: () => void; icon?: IconName; shortcut?: string; disabled?: boolean; danger?: boolean; checked?: boolean }
  | { kind: "separator" }
  | { kind: "heading"; label: string };

export function Menu({ x, y, entries, label, onClose }: { x: number; y: number; entries: MenuEntry[]; label: string; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });
  const opener = useRef<Element | null>(typeof document !== "undefined" ? document.activeElement : null);

  // Dentro da janela (abre para cima/esquerda junto às margens).
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const vw = window.innerWidth || 1024;
    const vh = window.innerHeight || 768;
    setPos({ left: Math.max(4, Math.min(x, vw - r.width - 4)), top: Math.max(4, y + r.height > vh - 4 ? y - r.height : y) });
  }, [x, y]);

  const items = () => Array.from(ref.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled), [role="menuitemcheckbox"]:not(:disabled)') ?? []);
  useEffect(() => {
    items()[0]?.focus();
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const t = setTimeout(() => window.addEventListener("mousedown", close, true), 0);
    return () => {
      clearTimeout(t);
      window.removeEventListener("mousedown", close, true);
      if (opener.current instanceof HTMLElement && document.contains(opener.current)) opener.current.focus();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onKey = (e: React.KeyboardEvent) => {
    const list = items();
    const i = list.indexOf(document.activeElement as HTMLButtonElement);
    const go = (n: number) => list[(n + list.length) % list.length]?.focus();
    e.stopPropagation();
    if (e.key === "ArrowDown") (e.preventDefault(), go(i + 1));
    else if (e.key === "ArrowUp") (e.preventDefault(), go(i - 1));
    else if (e.key === "Home") (e.preventDefault(), go(0));
    else if (e.key === "End") (e.preventDefault(), go(list.length - 1));
    else if (e.key === "Escape" || e.key === "Tab") (e.preventDefault(), onClose());
    else if (e.key.length === 1 && /\S/.test(e.key)) {
      const k = e.key.toLowerCase();
      const next = list.findIndex((b, j) => j > i && b.textContent?.trim().toLowerCase().startsWith(k));
      const first = list.findIndex((b) => b.textContent?.trim().toLowerCase().startsWith(k));
      if (next >= 0 || first >= 0) go(next >= 0 ? next : first);
    }
  };

  return (
    <div ref={ref} className="menu" role="menu" aria-label={label} style={pos} onKeyDown={onKey} onContextMenu={(e) => e.preventDefault()}>
      {entries.map((en, i) =>
        en.kind === "separator" ? (
          <div key={i} role="separator" />
        ) : en.kind === "heading" ? (
          <div key={i} className="menu-heading" role="presentation">
            {en.label}
          </div>
        ) : (
          <button
            key={i}
            type="button"
            role={en.checked === undefined ? "menuitem" : "menuitemcheckbox"}
            aria-checked={en.checked}
            className={en.danger ? "danger" : undefined}
            disabled={en.disabled}
            onClick={() => {
              onClose();
              en.onSelect();
            }}
          >
            <span className="menu-icon" aria-hidden="true">
              {en.checked ? <Icon name="check" size={14} /> : en.icon ? <Icon name={en.icon} size={14} /> : null}
            </span>
            <span className="menu-label">{en.label}</span>
            {en.shortcut && <kbd className="kbd-hint">{en.shortcut}</kbd>}
          </button>
        ),
      )}
    </div>
  );
}

/** Botão que abre um menu por baixo de si (teclado: Enter/Espaço/↓). */
export function MenuButton({ label, entries, children, className = "btn", title }: { label: string; entries: MenuEntry[]; children: ReactNode; className?: string; title?: string }) {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const ref = useRef<HTMLButtonElement>(null);
  const open = () => {
    const r = ref.current?.getBoundingClientRect();
    setAt({ x: r ? r.left : 0, y: r ? r.bottom + 4 : 0 });
  };
  return (
    <>
      <button
        ref={ref}
        type="button"
        className={className}
        aria-haspopup="menu"
        aria-expanded={at !== null}
        aria-label={label}
        title={title ?? label}
        onClick={() => (at ? setAt(null) : open())}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            open();
          }
        }}
      >
        {children}
      </button>
      {at && <Menu x={at.x} y={at.y} label={label} entries={entries} onClose={() => setAt(null)} />}
    </>
  );
}
