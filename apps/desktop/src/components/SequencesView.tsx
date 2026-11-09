import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { imageUrl } from "../api";
import { formatCaptureTime } from "../logic/format";
import { useStore } from "../store";
import type { GroupRow, PhotoRow } from "../types";
import { LabelChip } from "./LabelChip";

const ROW_H = 178;
const THUMB = 120;
const OVERSCAN = 4;

/** Vista “Sequências”: um grupo por linha (melhor primeiro), com comparar e aceitar. */
export function SequencesView({
  onSelect,
  onOpen,
  onCompare,
  onAcceptGroup,
}: {
  onSelect: (id: number, e: React.MouseEvent) => void;
  onOpen: (id: number) => void;
  onCompare: (ids: number[]) => void;
  onAcceptGroup: (g: GroupRow) => void;
}) {
  const { t, i18n } = useTranslation();
  const groups = useStore((s) => s.groups);
  const photos = useStore((s) => s.photos);
  const index = useStore((s) => s.index);
  const recs = useStore((s) => s.recs);
  const selection = useStore((s) => s.selection);
  const token = useStore((s) => s.project?.imageToken ?? 0);
  const versions = useStore((s) => s.thumbVersion);
  const [showPairs, setShowPairs] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ h: 600, top: 0 });

  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox((b) => ({ ...b, h: el.clientHeight })));
    ro.observe(el);
    setBox((b) => ({ ...b, h: el.clientHeight }));
    return () => ro.disconnect();
  }, []);

  const list = useMemo(() => groups.filter((g) => showPairs || g.kind !== "raw_jpeg"), [groups, showPairs]);
  const photo = (id: number): PhotoRow | undefined => photos[index.get(id) ?? -1];
  const first = Math.max(0, Math.floor(box.top / ROW_H) - OVERSCAN);
  const last = Math.min(list.length - 1, Math.ceil((box.top + box.h) / ROW_H) + OVERSCAN);

  if (groups.length === 0) {
    return (
      <div className="empty-state">
        <p className="muted">{t("sequences.empty")}</p>
      </div>
    );
  }

  return (
    <div className="sequences">
      <div className="sequences-bar">
        <span className="muted small">{t("sequences.count", { count: list.length })}</span>
        <span className="spacer" />
        <label className="small">
          <input type="checkbox" checked={showPairs} onChange={(e) => setShowPairs(e.target.checked)} /> {t("sequences.showPairs")}
        </label>
      </div>
      <div className="sequences-scroll" ref={scroller} onScroll={(e) => setBox({ h: e.currentTarget.clientHeight, top: e.currentTarget.scrollTop })}>
        <div style={{ height: list.length * ROW_H, position: "relative" }}>
          {list.slice(first, last + 1).map((g, k) => {
            const i = first + k;
            const firstPhoto = photo(g.members[0]!);
            const shortlist = g.members.filter((m) => ["best", "alternative"].includes(recs.get(m)?.label ?? "")).slice(0, 4);
            const toCompare = shortlist.length >= 2 ? shortlist : g.members.slice(0, Math.min(4, g.members.length));
            return (
              <section key={g.id} className="sequence-row" style={{ top: i * ROW_H, height: ROW_H - 8 }} aria-label={t(`sequences.kind.${g.kind}`)}>
                <header>
                  <strong>{t(`sequences.kind.${g.kind}`)}</strong>
                  <span className="muted small">
                    {t("sequences.photos", { count: g.members.length })}
                    {firstPhoto?.captureTime ? ` · ${formatCaptureTime(firstPhoto.captureTime, i18n.language)}` : ""}
                  </span>
                  <span className="spacer" />
                  <button type="button" className="btn small" onClick={() => onCompare(toCompare)} title={t("sequences.compareHint")}>
                    {t("compare.open")}
                  </button>
                  <button type="button" className="btn small" onClick={() => onAcceptGroup(g)} title={t("sequences.acceptHint")}>
                    {t("sequences.accept")}
                  </button>
                </header>
                <div className="sequence-strip">
                  {g.members.map((id) => {
                    const p = photo(id);
                    const r = recs.get(id);
                    if (!p) return null;
                    return (
                      <div
                        key={id}
                        className={`strip-item${selection.ids.has(id) ? " selected" : ""}${selection.focus === id ? " focused" : ""}${p.flag === -1 ? " rejected" : ""}`}
                        style={{ width: THUMB }}
                        onMouseDown={(e) => onSelect(id, e)}
                        onDoubleClick={() => onOpen(id)}
                        title={p.fileName}
                      >
                        <div className="strip-image" style={{ height: THUMB * 0.75 }}>
                          {p.thumb === "ready" ? <img src={`${imageUrl(token, "thumb", id)}?v=${versions.get(id) ?? 0}`} alt={p.fileName} draggable={false} decoding="async" /> : <span className="thumb-pending" />}
                          {p.flag !== 0 && <span className={`strip-flag${p.flag === 1 ? " pick" : " reject"}`}>{p.flag === 1 ? "♥" : "✕"}</span>}
                        </div>
                        {r && <LabelChip label={r.label} compact />}
                      </div>
                    );
                  })}
                </div>
              </section>
            );
          })}
        </div>
      </div>
    </div>
  );
}
