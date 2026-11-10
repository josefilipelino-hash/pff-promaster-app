import { memo, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { imageUrl } from "../api";
import { useStore } from "../store";
import type { PhotoRow } from "../types";

/** Largura de cada miniatura + espaço (px). */
export const STRIP_ITEM = 76;
const OVERSCAN = 6;

/** Índices visíveis da tira para uma posição de scroll (exportado para os testes). */
export function stripRange(scrollLeft: number, width: number, count: number): [number, number] {
  const first = Math.max(0, Math.floor(scrollLeft / STRIP_ITEM) - OVERSCAN);
  const last = Math.min(count - 1, Math.ceil((scrollLeft + width) / STRIP_ITEM) + OVERSCAN);
  return [first, last];
}

const Item = memo(function Item({ p, current, token, version, onSelect, onMenu }: { p: PhotoRow; current: boolean; token: number; version: number; onSelect: (id: number) => void; onMenu: (id: number, x: number, y: number) => void }) {
  return (
    <button
      type="button"
      className={`strip-cell${current ? " current" : ""}${p.flag === -1 ? " rejected" : ""}`}
      aria-label={p.fileName}
      aria-current={current ? "true" : undefined}
      tabIndex={current ? 0 : -1}
      onClick={() => onSelect(p.id)}
      onContextMenu={(e) => {
        e.preventDefault();
        onMenu(p.id, e.clientX, e.clientY);
      }}
      title={p.fileName}
    >
      {p.thumb === "ready" ? <img src={`${imageUrl(token, "thumb", p.id)}?v=${version}`} alt="" decoding="async" draggable={false} /> : <span className="strip-pending" />}
      {(p.stars > 0 || p.flag !== 0) && (
        <span className="strip-meta" aria-hidden="true">
          {p.flag === 1 ? "♥ " : p.flag === -1 ? "✕ " : ""}
          {p.stars > 0 ? "★".repeat(p.stars) : ""}
        </span>
      )}
      {p.availability && p.availability !== "ok" && <span className="strip-offline" aria-hidden="true" />}
    </button>
  );
});

/**
 * Tira de fotografias (Fase 14): virtualizada (só as visíveis existem no DOM — 15 000
 * fotografias não criam 15 000 elementos), a atual centrada sem animação, marcas e
 * classificação visíveis, menu contextual.
 */
export function Filmstrip({ photos, current, onSelect, onMenu }: { photos: PhotoRow[]; current: number | null; onSelect: (id: number) => void; onMenu: (id: number, x: number, y: number) => void }) {
  const { t } = useTranslation();
  const token = useStore((s) => s.project?.imageToken ?? 0);
  const versions = useStore((s) => s.thumbVersion);
  const ref = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState(0);
  const [width, setWidth] = useState(1200);
  const index = current === null ? -1 : photos.findIndex((p) => p.id === current);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(el.clientWidth || 1200));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  // A atual fica centrada (instantâneo: sem animações).
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || index < 0) return;
    const target = Math.max(0, index * STRIP_ITEM - (el.clientWidth - STRIP_ITEM) / 2);
    if (Math.abs(el.scrollLeft - target) > 1) el.scrollLeft = target;
    setScroll(el.scrollLeft);
  }, [index, width]);

  const [first, last] = stripRange(scroll, width, photos.length);
  const items = [];
  for (let i = first; i <= last; i++) {
    const p = photos[i]!;
    items.push(
      <div key={p.id} className="strip-slot" role="listitem" style={{ left: i * STRIP_ITEM }}>
        <Item p={p} current={i === index} token={token} version={versions.get(p.id) ?? 0} onSelect={onSelect} onMenu={onMenu} />
      </div>,
    );
  }
  return (
    <div ref={ref} className="filmstrip" role="list" aria-label={t("filmstrip.label", { count: photos.length })} onScroll={(e) => setScroll(e.currentTarget.scrollLeft)}>
      <div className="filmstrip-track" style={{ width: photos.length * STRIP_ITEM }}>
        {items}
      </div>
    </div>
  );
}
