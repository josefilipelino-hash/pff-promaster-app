import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, imageUrl } from "../api";
import { useStore } from "../store";
import type { PhotoRow } from "../types";

const GAP = 8;
const FOOTER = 24;
const PAD = 12;
const OVERSCAN_ROWS = 3;

export interface GridLayout {
  columns: number;
  cell: number;
  rowHeight: number;
}

export function computeLayout(width: number, thumbSize: number): GridLayout {
  const inner = Math.max(0, width - PAD * 2);
  const columns = Math.max(1, Math.floor((inner + GAP) / (thumbSize + GAP)));
  // Distribui o espaço restante pelas células para preencher a largura.
  const cell = Math.floor((inner - GAP * (columns - 1)) / columns);
  return { columns, cell, rowHeight: cell + FOOTER + GAP };
}

/** Linhas visíveis para uma posição de scroll (com margem). */
export function visibleRows(scrollTop: number, height: number, rowHeight: number, rowCount: number): [number, number] {
  const first = Math.max(0, Math.floor((scrollTop - PAD) / rowHeight) - OVERSCAN_ROWS);
  const last = Math.min(rowCount - 1, Math.ceil((scrollTop + height) / rowHeight) + OVERSCAN_ROWS);
  return [first, last];
}

interface CellProps {
  photo: PhotoRow;
  size: number;
  token: number;
  version: number;
  selected: boolean;
  focused: boolean;
  onClick: (id: number, e: React.MouseEvent) => void;
  onOpen: (id: number) => void;
  failedLabel: string;
}

const Cell = memo(function Cell({ photo, size, token, version, selected, focused, onClick, onOpen, failedLabel }: CellProps) {
  return (
    <div
      className={`cell${selected ? " selected" : ""}${focused ? " focused" : ""}${photo.flag === -1 ? " rejected" : ""}`}
      style={{ width: size }}
      onMouseDown={(e) => onClick(photo.id, e)}
      onDoubleClick={() => onOpen(photo.id)}
      title={photo.fileName}
      data-id={photo.id}
    >
      <div className="cell-image" style={{ height: size }}>
        {photo.thumb === "ready" ? (
          <img src={`${imageUrl(token, "thumb", photo.id)}?v=${version}`} alt={photo.fileName} decoding="async" draggable={false} />
        ) : photo.thumb === "failed" ? (
          <span className="thumb-failed" title={failedLabel}>
            !
          </span>
        ) : (
          <span className="thumb-pending" />
        )}
        {photo.color && <span className={`cell-color c-${photo.color}`} />}
      </div>
      <div className="cell-footer">
        <span className="cell-stars">{photo.stars > 0 ? "★".repeat(photo.stars) : ""}</span>
        <span className={`cell-flag${photo.flag === 1 ? " pick" : photo.flag === -1 ? " reject" : ""}`}>{photo.flag === 1 ? "♥" : photo.flag === -1 ? "✕" : ""}</span>
      </div>
    </div>
  );
});

export function PhotoGrid({
  photos,
  onClickPhoto,
  onOpen,
  layoutRef,
}: {
  photos: PhotoRow[];
  onClickPhoto: (id: number, e: React.MouseEvent) => void;
  onOpen: (id: number) => void;
  layoutRef: React.MutableRefObject<GridLayout>;
}) {
  const { t } = useTranslation();
  const token = useStore((s) => s.project?.imageToken ?? 0);
  const thumbSize = useStore((s) => s.thumbSize);
  const selection = useStore((s) => s.selection);
  const versions = useStore((s) => s.thumbVersion);
  const scroller = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ width: 800, height: 600 });
  const [scrollTop, setScrollTop] = useState(0);
  const restored = useRef(false);

  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ width: el.clientWidth, height: el.clientHeight }));
    ro.observe(el);
    setBox({ width: el.clientWidth, height: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  const layout = useMemo(() => computeLayout(box.width, thumbSize), [box.width, thumbSize]);
  layoutRef.current = layout;
  const rowCount = Math.ceil(photos.length / layout.columns);
  const [first, last] = visibleRows(scrollTop, box.height, layout.rowHeight, rowCount);

  // Mantém o item com foco visível (navegação por teclado, restauro do estado).
  const focusIndex = useMemo(() => (selection.focus === null ? -1 : photos.findIndex((p) => p.id === selection.focus)), [photos, selection.focus]);
  useEffect(() => {
    const el = scroller.current;
    if (!el || focusIndex < 0) return;
    const row = Math.floor(focusIndex / layout.columns);
    const top = PAD + row * layout.rowHeight;
    const bottom = top + layout.rowHeight;
    if (!restored.current) {
      restored.current = true;
      el.scrollTop = Math.max(0, top - el.clientHeight / 2 + layout.rowHeight / 2);
      return;
    }
    if (top < el.scrollTop) el.scrollTop = top - GAP;
    else if (bottom > el.scrollTop + el.clientHeight) el.scrollTop = bottom - el.clientHeight + GAP;
  }, [focusIndex, layout.columns, layout.rowHeight]);

  // Miniaturas visíveis por gerar passam à frente na fila do núcleo.
  const visiblePending = useMemo(() => {
    const ids: number[] = [];
    for (let i = Math.max(0, first + OVERSCAN_ROWS - 1) * layout.columns; i < Math.min(photos.length, (last - OVERSCAN_ROWS + 2) * layout.columns); i++) {
      if (photos[i]!.thumb === "pending") ids.push(photos[i]!.id);
    }
    return ids;
  }, [first, last, layout.columns, photos]);
  const pendingKey = visiblePending.join(",");
  useEffect(() => {
    if (!pendingKey) return;
    const h = setTimeout(() => api.prioritizeThumbnails(pendingKey.split(",").map(Number)).catch(() => {}), 120);
    return () => clearTimeout(h);
  }, [pendingKey]);

  const onScroll = useCallback(() => setScrollTop(scroller.current?.scrollTop ?? 0), []);

  const rows = [];
  for (let r = first; r <= last; r++) {
    const items = photos.slice(r * layout.columns, (r + 1) * layout.columns);
    rows.push(
      <div key={r} className="grid-row" style={{ top: PAD + r * layout.rowHeight, left: PAD, gap: GAP }}>
        {items.map((p) => (
          <Cell
            key={p.id}
            photo={p}
            size={layout.cell}
            token={token}
            version={versions.get(p.id) ?? 0}
            selected={selection.ids.has(p.id)}
            focused={selection.focus === p.id}
            onClick={onClickPhoto}
            onOpen={onOpen}
            failedLabel={t("grid.thumbFailed")}
          />
        ))}
      </div>,
    );
  }

  return (
    <div className="grid-scroller" ref={scroller} onScroll={onScroll} tabIndex={-1}>
      <div className="grid-canvas" style={{ height: PAD * 2 + rowCount * layout.rowHeight }}>
        {rows}
      </div>
    </div>
  );
}
