import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as RPointerEvent } from "react";
import { useTranslation } from "react-i18next";
import { api, imageUrl } from "../api";
import { align, bleedSlot, clampSlot, cm, cropWindow, distribute, panoramaSlot, photoCount, templateBoxes, type Align } from "../logic/album";
import { errorText, useStore } from "../store";
import { EXPORT_SPACES, type Album, type AlbumExportKind, type AlbumFormat, type AlbumIssue, type AlbumPhotoInfo, type AlbumSlot, type AlbumSpec, type AlbumSummary, type LabPreset } from "../types";

const NEW_SPEC: AlbumSpec = {
  format: "30x30", widthMm: 300, heightMm: 300, pages: 40, dpi: 300, bleedMm: 3, safeMm: 10, gutterMm: 16,
  color: "srgb", iccPath: "", intent: "perceptual", lab: "", confirmed: false, exportUnit: "spreads", minPages: 0, maxPages: 0, background: "#ffffff",
};

let idSeq = 0;
const newId = () => `u${Date.now().toString(16)}${(idSeq++).toString(16)}`;

type Drag =
  | { kind: "film"; key: number; x: number; y: number }
  | { kind: "move"; start: [number, number]; orig: AlbumSlot[] }
  | { kind: "resize"; start: [number, number]; orig: AlbumSlot }
  | { kind: "pan"; start: [number, number]; orig: AlbumSlot; win: [number, number, number, number]; pxw: number; pxh: number };

/** Editor de álbuns: especificação, spreads, arrastar e largar, layouts, automático, alertas e exportação. */
export function AlbumView() {
  const { t } = useTranslation();
  const toast = useStore((s) => s.toast);
  const project = useStore((s) => s.project);
  const photos = useStore((s) => s.photos);
  const upsert = useStore((s) => s.upsertExportJob);
  const fail = useCallback((e: unknown) => toast("error", errorText(e, t)), [toast, t]);
  const [albums, setAlbums] = useState<AlbumSummary[]>([]);
  const [album, setAlbum] = useState<Album | null>(null);
  const [idx, setIdx] = useState(0);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [infos, setInfos] = useState<Map<number, AlbumPhotoInfo>>(new Map());
  const [issues, setIssues] = useState<AlbumIssue[]>([]);
  const [templates, setTemplates] = useState<string[]>([]);
  const [formats, setFormats] = useState<AlbumFormat[]>([]);
  const [labs, setLabs] = useState<LabPreset[]>([]);
  const [overlays, setOverlays] = useState(true);
  const [cropMode, setCropMode] = useState(false);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [panel, setPanel] = useState<"albums" | "spec">("albums");
  const [confirmAuto, setConfirmAuto] = useState(false);
  const [autoInfo, setAutoInfo] = useState<string | null>(null);
  const [maxPerPage, setMaxPerPage] = useState(3);
  const [exportKind, setExportKind] = useState<AlbumExportKind>("printPdf");
  const [dest, setDest] = useState("");
  const [busy, setBusy] = useState(false);
  const stageRef = useRef<HTMLDivElement>(null);
  const [stage, setStage] = useState({ w: 900, h: 480 });

  useEffect(() => {
    Promise.all([api.listAlbums(), api.albumFormats(), api.labPresets(), api.albumPhotoInfos()])
      .then(([a, f, l, i]) => {
        setAlbums(a);
        setFormats(f);
        setLabs(l);
        setInfos(new Map(i.map((p) => [p.key, p])));
        if (a[0]) api.getAlbum(a[0].id).then(setAlbum).catch(fail);
      })
      .catch(fail);
  }, [fail]);

  useEffect(() => {
    const el = stageRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setStage({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, [album?.id]);

  // Gravação e verificação (com atraso, depois de cada alteração).
  const saveTimer = useRef<number | undefined>(undefined);
  const commit = useCallback(
    (next: Album) => {
      setAlbum(next);
      window.clearTimeout(saveTimer.current);
      saveTimer.current = window.setTimeout(() => {
        api.saveAlbum(next).catch(fail);
        api.albumPreflight(next).then(setIssues).catch(() => {});
        setAlbums((as) => as.map((a) => (a.id === next.id ? { ...a, name: next.name, pages: next.spec.pages, format: next.spec.format } : a)));
      }, 400);
    },
    [fail],
  );
  useEffect(() => {
    if (album) api.albumPreflight(album).then(setIssues).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [album?.id]);

  const spread = album?.spreads[idx];
  const count = spread ? Math.max(photoCount(spread), 1) : 1;
  useEffect(() => {
    api.albumTemplates(count).then(setTemplates).catch(() => setTemplates([]));
  }, [count]);

  const used = useMemo(() => new Set(album?.spreads.flatMap((s) => s.slots.map((x) => x.photo)).filter((x): x is number => x != null) ?? []), [album]);
  const film = useMemo(() => {
    const picks = photos.filter((p) => p.flag > 0);
    const pool = picks.length > 0 ? picks : photos.filter((p) => p.flag >= 0);
    return [...pool].sort((a, b) => (a.captureTime ?? "").localeCompare(b.captureTime ?? ""));
  }, [photos]);

  if (!album) {
    return (
      <div className="album-empty">
        <h2>{t("album.title")}</h2>
        <p className="muted">{t("album.emptyHint")}</p>
        <button type="button" className="btn primary" onClick={() => void createAlbum()}>
          {t("album.new")}
        </button>
      </div>
    );
  }
  const spec = album.spec;
  const b = spec.bleedMm;
  const W = 2 * spec.widthMm + 2 * b;
  const H = spec.heightMm + 2 * b;
  const scale = Math.max(0.2, Math.min((stage.w - 24) / W, (stage.h - 24) / H));
  const px = (mm: number) => mm * scale;

  async function createAlbum() {
    try {
      const a = await api.createAlbum(project?.name ? `${t("album.title")} ${project.name}` : t("album.title"), NEW_SPEC);
      setAlbums(await api.listAlbums());
      setAlbum(a);
      setIdx(0);
      setPanel("spec");
    } catch (e) {
      fail(e);
    }
  }

  const setSlots = (slots: AlbumSlot[], template = "custom") => {
    const spreads = album.spreads.map((s, i) => (i === idx ? { ...s, slots, template } : s));
    commit({ ...album, spreads });
  };
  const setSpec = (p: Partial<AlbumSpec>) => {
    const next = { ...spec, ...p };
    const n = Math.max(1, Math.floor(next.pages / 2));
    let spreads = album.spreads;
    if (spreads.length < n) spreads = [...spreads, ...Array.from({ length: n - spreads.length }, () => ({ id: newId(), template: "", marginMm: 12, spacingMm: 4, slots: [] }))];
    while (spreads.length > n && spreads[spreads.length - 1]!.slots.every((s) => s.photo == null)) spreads = spreads.slice(0, -1);
    commit({ ...album, spec: next, spreads });
    if (idx >= spreads.length) setIdx(spreads.length - 1);
  };

  const mmAt = (e: { clientX: number; clientY: number }): [number, number] => {
    const r = stageRef.current!.querySelector(".album-canvas")!.getBoundingClientRect();
    return [(e.clientX - r.left) / scale - b, (e.clientY - r.top) / scale - b];
  };

  const applyTemplate = async (tpl: string, margin = spread!.marginMm, spacing = spread!.spacingMm) => {
    try {
      const next = await api.applyAlbumLayout(album, idx, tpl, margin, spacing, []);
      commit(next);
      setSel(new Set());
    } catch (e) {
      fail(e);
    }
  };

  const runAuto = async () => {
    setBusy(true);
    setConfirmAuto(false);
    try {
      const r = await api.autoAlbum(album.id, { maxPerPage, marginMm: 12, spacingMm: 4, keepPages: true });
      setAlbum(r.album);
      setIdx(0);
      setSel(new Set());
      setAutoInfo(t("album.autoResult", { used: r.used, candidates: r.candidates, lowRes: r.skippedLowRes, dup: r.skippedDuplicates }));
      setIssues(await api.albumPreflight(r.album));
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const doExport = async () => {
    try {
      await api.saveAlbum(album);
      const id = await api.startAlbumExport(album.id, exportKind, dest);
      const job = (await api.exportJobs()).find((j) => j.id === id);
      if (job) upsert(job);
      toast("info", t("album.exportStarted"));
    } catch (e) {
      fail(e);
    }
  };

  // ---------- interação no spread ----------
  const onSlotDown = (e: RPointerEvent, s: AlbumSlot) => {
    e.stopPropagation();
    const next = new Set(e.shiftKey ? sel : []);
    if (e.shiftKey && sel.has(s.id)) next.delete(s.id);
    else next.add(s.id);
    setSel(next);
    const info = s.photo != null ? infos.get(s.photo) : undefined;
    if ((cropMode || e.altKey) && info) {
      const win = cropWindow(info.width, info.height, s.w, s.h, s.crop);
      setDrag({ kind: "pan", start: [e.clientX, e.clientY], orig: s, win, pxw: px(s.w), pxh: px(s.h) });
    } else {
      setDrag({ kind: "move", start: mmAt(e), orig: spread!.slots.filter((x) => next.has(x.id)) });
    }
  };

  const onMove = (e: { clientX: number; clientY: number }) => {
    if (!drag) return;
    if (drag.kind === "film") return setDrag({ ...drag, x: e.clientX, y: e.clientY });
    if (drag.kind === "pan") {
      const [u0, v0, u1, v1] = drag.win;
      const dx = ((e.clientX - drag.start[0]) / drag.pxw) * (u1 - u0);
      const dy = ((e.clientY - drag.start[1]) / drag.pxh) * (v1 - v0);
      const c = { ...drag.orig.crop, cx: (u0 + u1) / 2 - dx, cy: (v0 + v1) / 2 - dy };
      return setSlots(spread!.slots.map((x) => (x.id === drag.orig.id ? { ...x, crop: c } : x)), spread!.template);
    }
    const [mx, my] = mmAt(e);
    const dx = mx - drag.start[0];
    const dy = my - drag.start[1];
    if (drag.kind === "move") {
      const moved = new Map(drag.orig.map((o) => [o.id, clampSlot(spec, { ...o, x: Math.round((o.x + dx) * 2) / 2, y: Math.round((o.y + dy) * 2) / 2 })]));
      setSlots(spread!.slots.map((x) => moved.get(x.id) ?? x));
    } else {
      const r = clampSlot(spec, { ...drag.orig, w: drag.orig.w + dx, h: drag.orig.h + dy });
      setSlots(spread!.slots.map((x) => (x.id === r.id ? r : x)));
    }
  };

  const onUp = (e: { clientX: number; clientY: number }) => {
    if (drag?.kind === "film") {
      const target = document.elementFromPoint(e.clientX, e.clientY);
      const slotEl = target?.closest<HTMLElement>("[data-slot]");
      const canvas = target?.closest(".album-canvas");
      if (slotEl) {
        const id = slotEl.dataset.slot!;
        const info = infos.get(drag.key);
        setSlots(spread!.slots.map((x) => (x.id === id ? { ...x, photo: drag.key, crop: { zoom: 1, cx: faceCx(info), cy: faceCy(info) } } : x)), spread!.template);
        setSel(new Set([id]));
      } else if (canvas) {
        const [mx, my] = mmAt(e);
        const info = infos.get(drag.key);
        const ar = info ? info.width / info.height : 1.5;
        const h = spec.heightMm / 3;
        const s = clampSlot(spec, { id: newId(), x: mx - (h * ar) / 2, y: my - h / 2, w: h * ar, h, photo: drag.key, crop: { zoom: 1, cx: 0.5, cy: 0.5 } });
        setSlots([...spread!.slots, s]);
        setSel(new Set([s.id]));
      }
    }
    setDrag(null);
  };

  const selected = spread?.slots.filter((s) => sel.has(s.id)) ?? [];
  const one = selected.length === 1 ? selected[0]! : null;
  const mapSel = (f: (s: AlbumSlot) => AlbumSlot) => setSlots(spread!.slots.map((s) => (sel.has(s.id) ? f(s) : s)));
  const spreadIssues = issues.filter((i) => i.spread === idx);
  const issueOf = (id: string) => spreadIssues.filter((i) => i.slot === id);
  const albumIssues = issues.filter((i) => i.spread == null);
  const errors = issues.filter((i) => i.severity === "error").length;

  const onKey = (e: React.KeyboardEvent) => {
    if (!one && selected.length === 0) return;
    const step = e.shiftKey ? 10 : 1;
    const d: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (d[e.key]) {
      e.preventDefault();
      const [dx, dy] = d[e.key]!;
      mapSel((s) => clampSlot(spec, { ...s, x: s.x + dx, y: s.y + dy }));
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      setSlots(spread!.slots.filter((s) => !sel.has(s.id)));
      setSel(new Set());
    }
  };

  const thumb = (key: number, level: "thumb" | "preview" = "thumb") => (project ? imageUrl(project.imageToken, level, key) : "");

  return (
    <div className="album-view" onPointerMove={onMove} onPointerUp={onUp} onPointerLeave={() => drag?.kind !== "film" && setDrag(null)}>
      <aside className="album-side">
        <div className="gallery-tabs" role="tablist">
          {(["albums", "spec"] as const).map((k) => (
            <button key={k} type="button" role="tab" aria-selected={panel === k} className={panel === k ? "active" : ""} onClick={() => setPanel(k)}>
              {t(`album.panel.${k}`)}
            </button>
          ))}
        </div>
        {panel === "albums" ? (
          <>
            <ul className="album-list" aria-label={t("album.list")}>
              {albums.map((a) => (
                <li key={a.id}>
                  <button type="button" className={`side-item${a.id === album.id ? " active" : ""}`} onClick={() => api.getAlbum(a.id).then((x) => (setAlbum(x), setIdx(0), setSel(new Set()))).catch(fail)}>
                    <span>{a.name}</span>
                    <span className="count">{a.format} · {a.pages}p</span>
                  </button>
                </li>
              ))}
            </ul>
            <button type="button" className="btn small" onClick={() => void createAlbum()}>
              + {t("album.new")}
            </button>
            <label className="field compact">
              <span>{t("album.name")}</span>
              <input value={album.name} onChange={(e) => commit({ ...album, name: e.target.value })} aria-label={t("album.name")} />
            </label>
            <button
              type="button"
              className="btn small ghost"
              onClick={() =>
                api
                  .deleteAlbum(album.id)
                  .then(async () => {
                    const rest = await api.listAlbums();
                    setAlbums(rest);
                    setAlbum(rest[0] ? await api.getAlbum(rest[0].id) : null);
                  })
                  .catch(fail)
              }
            >
              {t("album.delete")}
            </button>
          </>
        ) : (
          <SpecForm spec={spec} formats={formats} labs={labs} setSpec={setSpec} onSaveLab={async (name) => {
            try {
              const p = await api.saveLabPreset({ id: "", name, notes: "", dpi: spec.dpi, bleedMm: spec.bleedMm, safeMm: spec.safeMm, gutterMm: spec.gutterMm, color: spec.color, iccPath: spec.iccPath, intent: spec.intent, exportUnit: spec.exportUnit, minPages: spec.minPages, maxPages: spec.maxPages });
              setLabs(await api.labPresets());
              setSpec({ lab: p.name, confirmed: true });
              toast("info", t("album.labSaved", { name: p.name }));
            } catch (e) {
              fail(e);
            }
          }} />
        )}
      </aside>

      <section className="album-main">
        <div className="album-tools" role="toolbar" aria-label={t("album.tools")}>
          <button type="button" className="btn small" disabled={idx === 0} onClick={() => (setIdx(idx - 1), setSel(new Set()))} aria-label={t("album.prevSpread")}>
            ◀
          </button>
          <span className="small">{t("album.spreadOf", { n: idx + 1, total: album.spreads.length, pages: `${idx * 2 + 1}–${idx * 2 + 2}` })}</span>
          <button type="button" className="btn small" disabled={idx >= album.spreads.length - 1} onClick={() => (setIdx(idx + 1), setSel(new Set()))} aria-label={t("album.nextSpread")}>
            ▶
          </button>
          <span className="sep" />
          <div className="album-templates" aria-label={t("album.layouts")}>
            {templates.map((tpl) => (
              <button key={tpl} type="button" className={`tpl${spread?.template === tpl ? " active" : ""}`} title={t("album.layoutHint", { id: tpl })} aria-label={t("album.layoutHint", { id: tpl })} onClick={() => void applyTemplate(tpl)}>
                <svg viewBox="0 0 40 20" aria-hidden="true">
                  <rect x="0.5" y="0.5" width="39" height="19" className="tpl-page" />
                  <line x1="20" y1="0" x2="20" y2="20" className="tpl-fold" />
                  {templateBoxes(tpl).map((r, i) => (
                    <rect key={i} x={r[0] * 40} y={r[1] * 20} width={r[2] * 40} height={r[3] * 20} className="tpl-box" />
                  ))}
                </svg>
              </button>
            ))}
          </div>
          <label className="field inline">
            <span>{t("album.margin")}</span>
            <input type="number" min={0} max={60} value={spread?.marginMm ?? 12} onChange={(e) => spread?.template && spread.template !== "custom" && void applyTemplate(spread.template, Number(e.target.value), spread.spacingMm)} aria-label={t("album.margin")} />
          </label>
          <label className="field inline">
            <span>{t("album.spacing")}</span>
            <input type="number" min={0} max={40} value={spread?.spacingMm ?? 4} onChange={(e) => spread?.template && spread.template !== "custom" && void applyTemplate(spread.template, spread.marginMm, Number(e.target.value))} aria-label={t("album.spacing")} />
          </label>
        </div>
        <div className="album-tools" role="toolbar" aria-label={t("album.arrange")}>
          {(["left", "hcenter", "right", "top", "vcenter", "bottom"] as Align[]).map((a) => (
            <button key={a} type="button" className="btn small" disabled={selected.length < 2} onClick={() => setSlots(align(spread!.slots, sel, a))} title={t(`album.align.${a}`)} aria-label={t(`album.align.${a}`)}>
              {{ left: "⇤", hcenter: "↔", right: "⇥", top: "⤒", vcenter: "↕", bottom: "⤓" }[a]}
            </button>
          ))}
          <button type="button" className="btn small" disabled={selected.length < 3} onClick={() => setSlots(distribute(spread!.slots, sel, "h"))}>
            {t("album.distributeH")}
          </button>
          <button type="button" className="btn small" disabled={selected.length < 3} onClick={() => setSlots(distribute(spread!.slots, sel, "v"))}>
            {t("album.distributeV")}
          </button>
          <span className="sep" />
          <button type="button" className="btn small" disabled={!one} onClick={() => mapSel((s) => bleedSlot(spec, s))}>
            {t("album.fullBleed")}
          </button>
          <button type="button" className="btn small" disabled={!one} onClick={() => mapSel((s) => panoramaSlot(spec, s))}>
            {t("album.panorama")}
          </button>
          <button type="button" className={`btn small${cropMode ? " active" : ""}`} aria-pressed={cropMode} onClick={() => setCropMode(!cropMode)} title={t("album.cropHint")}>
            {t("album.crop")}
          </button>
          {one?.photo != null && (
            <label className="field inline">
              <span>{t("album.zoom")}</span>
              <input type="range" min={1} max={3} step={0.05} value={one.crop.zoom} onChange={(e) => mapSel((s) => ({ ...s, crop: { ...s.crop, zoom: Number(e.target.value) } }))} aria-label={t("album.zoom")} />
            </label>
          )}
          <button type="button" className="btn small" disabled={selected.length === 0} onClick={() => mapSel((s) => ({ ...s, photo: null }))}>
            {t("album.removePhoto")}
          </button>
          <button type="button" className="btn small" disabled={selected.length === 0} onClick={() => (setSlots(spread!.slots.filter((s) => !sel.has(s.id))), setSel(new Set()))}>
            {t("album.deleteSlot")}
          </button>
          <label className="check small">
            <input type="checkbox" checked={overlays} onChange={(e) => setOverlays(e.target.checked)} /> {t("album.guides")}
          </label>
        </div>

        <div className="album-stage" ref={stageRef} tabIndex={0} onKeyDown={onKey} onPointerDown={() => setSel(new Set())} aria-label={t("album.spread")}>
          <div className="album-canvas" style={{ width: px(W), height: px(H), background: spec.background }} role="application" aria-label={t("album.canvas")}>
            {spread?.slots.map((s) => {
              const info = s.photo != null ? infos.get(s.photo) : undefined;
              const own = issueOf(s.id);
              let img = null;
              if (s.photo != null) {
                const [u0, v0, u1, v1] = info ? cropWindow(info.width, info.height, s.w, s.h, s.crop) : [0, 0, 1, 1];
                const iw = px(s.w) / (u1 - u0);
                const ih = px(s.h) / (v1 - v0);
                img = <img src={thumb(s.photo, "preview")} alt="" draggable={false} style={{ width: iw, height: ih, left: -u0 * iw, top: -v0 * ih }} />;
              }
              return (
                <div
                  key={s.id}
                  data-slot={s.id}
                  className={`album-slot${sel.has(s.id) ? " selected" : ""}${s.photo == null ? " empty" : ""}${own.some((i) => i.severity === "error") ? " has-error" : own.length ? " has-warning" : ""}`}
                  style={{ left: px(s.x + b), top: px(s.y + b), width: px(s.w), height: px(s.h) }}
                  onPointerDown={(e) => onSlotDown(e, s)}
                  aria-label={t("album.slot")}
                  aria-selected={sel.has(s.id)}
                >
                  {img}
                  {s.photo == null && <span className="album-drop">{t("album.dropHere")}</span>}
                  {own.length > 0 && (
                    <span className="album-badge" title={own.map((i) => t(i.code, { value: i.value })).join("\n")}>
                      {own.some((i) => i.severity === "error") ? "!" : "⚠"}
                    </span>
                  )}
                  {one?.id === s.id && (
                    <span
                      className="album-handle"
                      onPointerDown={(e) => {
                        e.stopPropagation();
                        setDrag({ kind: "resize", start: mmAt(e), orig: s });
                      }}
                      aria-label={t("album.resize")}
                    />
                  )}
                </div>
              );
            })}
            {overlays && (
              <svg className="album-guides" width={px(W)} height={px(H)} aria-hidden="true">
                <rect x={0} y={0} width={px(W)} height={px(b)} className="g-bleed" />
                <rect x={0} y={px(H - b)} width={px(W)} height={px(b)} className="g-bleed" />
                <rect x={0} y={0} width={px(b)} height={px(H)} className="g-bleed" />
                <rect x={px(W - b)} y={0} width={px(b)} height={px(H)} className="g-bleed" />
                <rect x={px(b)} y={px(b)} width={px(W - 2 * b)} height={px(H - 2 * b)} className="g-trim" />
                <rect x={px(b + spec.safeMm)} y={px(b + spec.safeMm)} width={px(W - 2 * b - 2 * spec.safeMm)} height={px(H - 2 * b - 2 * spec.safeMm)} className="g-safe" />
                <rect x={px(b + spec.widthMm - spec.gutterMm / 2)} y={0} width={px(spec.gutterMm)} height={px(H)} className="g-gutter" />
                <line x1={px(b + spec.widthMm)} y1={0} x2={px(b + spec.widthMm)} y2={px(H)} className="g-fold" />
              </svg>
            )}
          </div>
        </div>

        <div className="album-spreads" aria-label={t("album.spreads")}>
          {album.spreads.map((sp, i) => (
            <button key={sp.id} type="button" className={`album-mini${i === idx ? " active" : ""}${issues.some((x) => x.spread === i && x.severity === "error") ? " has-error" : ""}`} onClick={() => (setIdx(i), setSel(new Set()))} aria-label={t("album.spreadN", { n: i + 1 })} aria-current={i === idx}>
              {sp.slots.map((s) => (
                <span key={s.id} style={{ left: `${((s.x + b) / W) * 100}%`, top: `${((s.y + b) / H) * 100}%`, width: `${(s.w / W) * 100}%`, height: `${(s.h / H) * 100}%`, backgroundImage: s.photo != null ? `url("${thumb(s.photo)}")` : undefined }} />
              ))}
              <em>{i + 1}</em>
            </button>
          ))}
        </div>

        <div className="album-film" aria-label={t("album.film")}>
          {film.map((p) => (
            <button
              key={p.id}
              type="button"
              className={`album-film-item${used.has(p.id) ? " used" : ""}`}
              onPointerDown={(e) => {
                e.preventDefault();
                setDrag({ kind: "film", key: p.id, x: e.clientX, y: e.clientY });
              }}
              aria-label={`${p.fileName}${used.has(p.id) ? ` — ${t("album.used")}` : ""}`}
              title={p.fileName}
            >
              <img src={thumb(p.id)} alt="" draggable={false} />
            </button>
          ))}
        </div>
        {drag?.kind === "film" && <img className="album-ghost" src={thumb(drag.key)} alt="" style={{ left: drag.x + 8, top: drag.y + 8 }} />}
      </section>

      <aside className="album-side right">
        <h4>{t("album.auto")}</h4>
        <p className="small muted">{t("album.autoHint")}</p>
        <label className="field compact">
          <span>{t("album.maxPerPage")}</span>
          <select value={maxPerPage} onChange={(e) => setMaxPerPage(Number(e.target.value))} aria-label={t("album.maxPerPage")}>
            {[1, 2, 3, 4].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>
        {confirmAuto ? (
          <div className="dev-row wrap">
            <span className="small warn">{t("album.autoReplace")}</span>
            <button type="button" className="btn small" onClick={() => setConfirmAuto(false)}>
              {t("common.cancel")}
            </button>
            <button type="button" className="btn small primary" onClick={() => void runAuto()}>
              {t("album.autoConfirm")}
            </button>
          </div>
        ) : (
          <button type="button" className="btn primary" disabled={busy} onClick={() => (used.size > 0 ? setConfirmAuto(true) : void runAuto())}>
            ✦ {t("album.autoButton")}
          </button>
        )}
        {autoInfo && <p className="small" role="status">{autoInfo}</p>}
        {album.notes.length > 0 && <p className="small muted">{album.notes.map((n) => { const [c, k] = n.split(":"); return `${t(`album.chapter.${c}`)} (${k})`; }).join(" → ")}</p>}

        <h4>{t("album.checks")}</h4>
        <p className="small">{errors > 0 ? <strong className="warn">{t("album.errors", { count: errors })}</strong> : t("album.noErrors")}</p>
        <ul className="album-issues" aria-label={t("album.checks")}>
          {albumIssues.map((i, k) => (
            <li key={`a${k}`} className={i.severity}>
              {t(i.code, { value: i.value })}
            </li>
          ))}
          {issues
            .filter((i) => i.spread != null)
            .slice(0, 60)
            .map((i, k) => (
              <li key={k} className={i.severity}>
                <button type="button" className="link-button" onClick={() => (setIdx(i.spread!), setSel(new Set(i.slot ? [i.slot] : [])))}>
                  {t("album.spreadN", { n: i.spread! + 1 })}: {t(i.code, { value: i.value })}
                </button>
              </li>
            ))}
        </ul>

        <h4>{t("album.export")}</h4>
        <label className="field compact">
          <span>{t("album.exportKind")}</span>
          <select value={exportKind} onChange={(e) => setExportKind(e.target.value as AlbumExportKind)} aria-label={t("album.exportKind")}>
            {(["printPdf", "proofPdf", "jpeg", "tiff"] as const).map((k) => (
              <option key={k} value={k}>
                {t(`album.kind.${k}`)}
              </option>
            ))}
          </select>
        </label>
        <div className="dev-row">
          <input className="grow" readOnly value={dest} placeholder={t("export.destinationNone")} aria-label={t("export.destination")} />
          <button type="button" className="btn small" onClick={async () => { const d = await open({ directory: true, multiple: false, title: t("export.destination") }); if (typeof d === "string") setDest(d); }}>
            {t("export.choose")}
          </button>
        </div>
        <button type="button" className="btn" disabled={!dest || errors > 0} onClick={() => void doExport()} title={errors > 0 ? t("album.fixErrors") : undefined}>
          {t("album.exportButton")}
        </button>
      </aside>
    </div>
  );
}

function faceCx(i?: AlbumPhotoInfo): number {
  if (!i || i.faces.length === 0) return 0.5;
  return i.faces.reduce((t, f) => t + f[0] + f[2] / 2, 0) / i.faces.length;
}

function faceCy(i?: AlbumPhotoInfo): number {
  if (!i || i.faces.length === 0) return 0.5;
  return Math.min(0.95, i.faces.reduce((t, f) => t + f[1] + f[3] / 2, 0) / i.faces.length + 0.08);
}

/** Especificação: formato, medidas, orientação, páginas, DPI, sangria, zona segura, dobra, cor e laboratório. */
function SpecForm({ spec, formats, labs, setSpec, onSaveLab }: { spec: AlbumSpec; formats: AlbumFormat[]; labs: LabPreset[]; setSpec: (p: Partial<AlbumSpec>) => void; onSaveLab: (name: string) => void }) {
  const { t } = useTranslation();
  const [labName, setLabName] = useState("");
  const square = Math.abs(spec.widthMm - spec.heightMm) < 0.5;
  const orientation = square ? "square" : spec.widthMm > spec.heightMm ? "landscape" : "portrait";
  const num = (label: string, value: number, onChange: (v: number) => void, step = 1, min = 0) => (
    <label className="field compact">
      <span>{label}</span>
      <input type="number" step={step} min={min} value={value} onChange={(e) => onChange(Number(e.target.value))} aria-label={label} />
    </label>
  );
  return (
    <div className="album-spec">
      <label className="field compact">
        <span>{t("album.format")}</span>
        <select
          value={formats.some((f) => f.name === spec.format) ? spec.format : "custom"}
          onChange={(e) => {
            const f = formats.find((x) => x.name === e.target.value);
            if (!f) return setSpec({ format: "custom" });
            const [a, b] = [f.widthCm * 10, f.heightCm * 10];
            const portrait = orientation === "portrait";
            setSpec({ format: f.name, widthMm: portrait ? Math.min(a, b) : Math.max(a, b), heightMm: portrait ? Math.max(a, b) : Math.min(a, b) });
          }}
          aria-label={t("album.format")}
        >
          {formats.map((f) => (
            <option key={f.name} value={f.name}>
              {f.name} cm
            </option>
          ))}
          <option value="custom">{t("album.custom")}</option>
        </select>
      </label>
      <div className="dev-row">
        {num(t("album.width"), spec.widthMm / 10, (v) => setSpec({ widthMm: v * 10, format: "custom" }), 0.1, 5)}
        {num(t("album.height"), spec.heightMm / 10, (v) => setSpec({ heightMm: v * 10, format: "custom" }), 0.1, 5)}
      </div>
      <label className="field compact">
        <span>{t("album.orientation")}</span>
        <select value={orientation} disabled={square} onChange={(e) => e.target.value !== orientation && setSpec({ widthMm: spec.heightMm, heightMm: spec.widthMm })} aria-label={t("album.orientation")}>
          <option value="square">{t("album.orient.square")}</option>
          <option value="landscape">{t("album.orient.landscape")}</option>
          <option value="portrait">{t("album.orient.portrait")}</option>
        </select>
      </label>
      <div className="dev-row">
        {num(t("album.pages"), spec.pages, (v) => setSpec({ pages: Math.max(2, Math.round(v / 2) * 2) }), 2, 2)}
        {num("DPI", spec.dpi, (v) => setSpec({ dpi: v }), 10, 72)}
      </div>
      <div className="dev-row">
        {num(t("album.bleed"), spec.bleedMm, (v) => setSpec({ bleedMm: v, confirmed: false }), 0.5)}
        {num(t("album.safe"), spec.safeMm, (v) => setSpec({ safeMm: v, confirmed: false }), 0.5)}
        {num(t("album.gutter"), spec.gutterMm, (v) => setSpec({ gutterMm: v, confirmed: false }), 0.5)}
      </div>
      <label className="field compact">
        <span>{t("export.colorSpace")}</span>
        <select value={spec.color} onChange={(e) => setSpec({ color: e.target.value as AlbumSpec["color"] })} aria-label={t("export.colorSpace")}>
          {EXPORT_SPACES.map((c) => (
            <option key={c} value={c}>
              {t(`export.space.${c}`)}
            </option>
          ))}
        </select>
      </label>
      {spec.color === "icc" && (
        <div className="dev-row">
          <span className="small ellipsis grow">{spec.iccPath || t("album.noIcc")}</span>
          <button type="button" className="btn small" onClick={async () => { const p = await open({ multiple: false, filters: [{ name: "ICC", extensions: ["icc", "icm"] }] }); if (typeof p === "string") setSpec({ iccPath: p }); }}>
            {t("export.choose")}
          </button>
        </div>
      )}
      <label className="field compact">
        <span>{t("album.unit")}</span>
        <select value={spec.exportUnit} onChange={(e) => setSpec({ exportUnit: e.target.value as AlbumSpec["exportUnit"] })} aria-label={t("album.unit")}>
          <option value="spreads">{t("album.unitSpreads")}</option>
          <option value="pages">{t("album.unitPages")}</option>
        </select>
      </label>
      <label className="field compact">
        <span>{t("album.background")}</span>
        <input type="color" value={spec.background} onChange={(e) => setSpec({ background: e.target.value })} aria-label={t("album.background")} />
      </label>
      <h4>{t("album.lab")}</h4>
      <label className="field compact">
        <span>{t("album.labPreset")}</span>
        <select
          value=""
          onChange={(e) => {
            const l = labs.find((x) => x.id === e.target.value);
            if (l) setSpec({ lab: l.name, dpi: l.dpi || spec.dpi, bleedMm: l.bleedMm, safeMm: l.safeMm, gutterMm: l.gutterMm, ...(l.color ? { color: l.color, iccPath: l.iccPath } : {}), intent: l.intent, exportUnit: l.exportUnit, minPages: l.minPages, maxPages: l.maxPages, confirmed: true });
          }}
          aria-label={t("album.labPreset")}
        >
          <option value="">{spec.lab || t("album.labChoose")}</option>
          {labs.map((l) => (
            <option key={l.id} value={l.id}>
              {l.name}
            </option>
          ))}
        </select>
      </label>
      <div className="dev-row">
        <input className="grow" value={labName} onChange={(e) => setLabName(e.target.value)} placeholder={t("album.labName")} aria-label={t("album.labName")} />
        <button type="button" className="btn small" disabled={!labName.trim()} onClick={() => (onSaveLab(labName.trim()), setLabName(""))}>
          {t("album.saveLab")}
        </button>
      </div>
      <label className="check small">
        <input type="checkbox" checked={spec.confirmed} onChange={(e) => setSpec({ confirmed: e.target.checked })} /> {t("album.confirmed")}
      </label>
      {!spec.confirmed && <p className="small warn">{t("album.issue.unconfirmed")}</p>}
      <p className="small muted">
        {t("album.summary", { w: cm(spec.widthMm), h: cm(spec.heightMm), spreads: Math.floor(spec.pages / 2) })}
      </p>
    </div>
  );
}
