import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, events, isMac } from "../../api";
import { ASPECTS, cloneSettings, DEFAULT_COPY_BLOCKS, defaultSettings, fitCropToAspect, parseFrame, sameSettings, type Frame } from "../../logic/develop";
import { errorText, useStore } from "../../store";
import { SETTINGS_BLOCKS, type AutoOptions, type AutoReport, type DevelopSettings, type EditView } from "../../types";
import { AutoOptionsForm, AutoReportView } from "./AutoPanel";
import { CreatePresetDialog, ImportReportDialog, importPresets, PresetList, PresetManager } from "../Presets";
import { defaultMatchOptions } from "../../logic/develop";
import type { MatchOptions, Preset, PresetImportResult } from "../../types";
import { Modal } from "../Modal";
import { DevelopPanel, type SetField } from "./DevelopPanel";
import { Slider } from "./Slider";

/** Opções para só detetar olhos vermelhos ou manchas (o resto da receita fica igual). */
function onlyRetouch(kind: "redEye" | "dust"): AutoOptions {
  return {
    intensity: "custom",
    strength: 100,
    tone: false,
    whiteBalance: false,
    color: false,
    sharpen: false,
    denoise: false,
    aiDenoise: "off",
    straighten: false,
    crop: false,
    lens: false,
    redEye: kind === "redEye",
    dust: kind === "dust",
  };
}

/** "Antes" alinhado com o "depois" na vista dividida: original com o mesmo enquadramento. */
export function beforeForSplit(s: DevelopSettings): DevelopSettings {
  const b = defaultSettings();
  b.geometry = { ...s.geometry, crop: { ...s.geometry.crop } };
  return b;
}

type Quality = "fast" | "final";
/** Depois de parar de mexer, o render final chega após este intervalo. */
export const SETTLE_MS = 220;

/** Etiqueta do histórico → texto. */
export function historyLabel(label: string, t: (k: string, o?: Record<string, unknown>) => string): string {
  if (label.startsWith("preset:")) return t("develop.history.preset", { name: label.slice(7) });
  if (label.startsWith("reset.")) return t("develop.history.resetSection", { section: t(`develop.section.${label.slice(6)}`) });
  for (const p of ["develop.history.", "develop.field."]) {
    const k = p + label;
    const v = t(k);
    if (v !== k) return v;
  }
  if (label.startsWith("hsl.")) return t("develop.section.hsl");
  return label;
}

/**
 * Renderização com qualidade dinâmica: um pedido de cada vez; durante a interação, render
 * rápido (resolução menor, sem nitidez/ruído); ao parar, render final à resolução do ecrã.
 */
function useRenderer(key: number, settings: DevelopSettings, before: boolean, uncropped: boolean, box: { w: number; h: number }, onFrame: (f: Frame) => void, fail: (e: unknown) => void) {
  const st = useRef({ inFlight: false, dirty: false, lastChange: 0, timer: 0 as unknown as ReturnType<typeof setTimeout> | 0 });
  const latest = useRef({ key, settings, before, uncropped, box });
  latest.current = { key, settings, before, uncropped, box };
  const run = useCallback(async () => {
    const r = st.current;
    if (r.inFlight) {
      r.dirty = true;
      return;
    }
    if (r.timer) clearTimeout(r.timer);
    r.timer = 0;
    r.inFlight = true;
    r.dirty = false;
    const L = latest.current;
    const quality: Quality = Date.now() - r.lastChange < SETTLE_MS ? "fast" : "final";
    const dpr = window.devicePixelRatio || 1;
    const scale = quality === "fast" ? 0.6 : 1;
    const cap = quality === "fast" ? 1200 : 2560;
    const w = Math.max(64, Math.min(cap, Math.round(L.box.w * dpr * scale)));
    const h = Math.max(64, Math.min(cap, Math.round(L.box.h * dpr * scale)));
    try {
      const buf = await api.renderEdit(L.key, L.before ? defaultSettings() : L.settings, w, h, quality, L.uncropped && !L.before);
      onFrame(parseFrame(buf));
    } catch (e) {
      fail(e);
    }
    r.inFlight = false;
    if (r.dirty) void run();
    else if (quality === "fast") r.timer = setTimeout(() => void run(), SETTLE_MS + 20);
  }, [onFrame, fail]);
  return {
    /** `interactive`: durante o arrasto (render rápido). */
    request: (interactive: boolean) => {
      st.current.lastChange = interactive ? Date.now() : 0;
      void run();
    },
  };
}

function CopyDialog({
  title,
  action,
  onClose,
  onConfirm,
  onSmart,
}: {
  title: string;
  action: string;
  onClose: () => void;
  onConfirm: (blocks: string[]) => void;
  /** Sincronizar: modo "ajustar cada fotografia à referência". */
  onSmart?: (o: MatchOptions) => void;
}) {
  const { t } = useTranslation();
  const [sel, setSel] = useState<string[]>(DEFAULT_COPY_BLOCKS);
  const [smart, setSmart] = useState(!!onSmart);
  const [opts, setOpts] = useState<MatchOptions>(defaultMatchOptions());
  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose} data-close>
            {t("common.cancel")}
          </button>
          <button
            type="button"
            className="btn primary"
            disabled={smart ? !Object.values(opts).some(Boolean) : sel.length === 0}
            onClick={() => (smart && onSmart ? onSmart(opts) : onConfirm(sel))}
          >
            {action}
          </button>
        </>
      }
    >
      {onSmart && (
        <fieldset className="options">
          <label>
            <input type="radio" name="sync-mode" checked={smart} onChange={() => setSmart(true)} /> {t("sync.smart")}
          </label>
          <label>
            <input type="radio" name="sync-mode" checked={!smart} onChange={() => setSmart(false)} /> {t("sync.copyValues")}
          </label>
        </fieldset>
      )}
      {smart && onSmart ? (
        <>
          <p className="small">{t("sync.smartIntro")}</p>
          <div className="copy-blocks">
            {(["look", "exposure", "whiteBalance", "skin", "contrast", "color"] as const).map((k) => (
              <label key={k}>
                <input type="checkbox" checked={opts[k]} onChange={(e) => setOpts({ ...opts, [k]: e.target.checked })} /> {t(`sync.option.${k}`)}
              </label>
            ))}
          </div>
          <p className="small muted">{t("sync.smartNote")}</p>
        </>
      ) : (
        <>
          <div className="copy-blocks">
            {SETTINGS_BLOCKS.map((b) => (
              <label key={b}>
                <input type="checkbox" checked={sel.includes(b)} onChange={(e) => setSel(e.target.checked ? [...sel, b] : sel.filter((x) => x !== b))} /> {t(`develop.section.${b}`)}
              </label>
            ))}
          </div>
          <p className="small muted">{t("develop.copyNote")}</p>
        </>
      )}
    </Modal>
  );
}

export function DevelopView({ photoKey, order, onNavigate }: { photoKey: number; order: number[]; onNavigate: (key: number) => void }) {
  const { t } = useTranslation();
  const toast = useStore((s) => s.toast);
  const photo = useStore((s) => s.photos[s.index.get(photoKey) ?? -1]);
  const selection = useStore((s) => s.selection);
  const clipboard = useStore((s) => s.clipboard);
  const setClipboard = useStore((s) => s.setClipboard);
  const reloadPhotos = useStore((s) => s.reloadPhotos);
  const [view, setView] = useState<EditView | null>(null);
  const [settings, setSettings] = useState<DevelopSettings>(defaultSettings());
  const [frame, setFrame] = useState<Frame | null>(null);
  const [before, setBefore] = useState(false);
  const [cropMode, setCropMode] = useState(false);
  const [wbPicking, setWbPicking] = useState(false);
  const [dialog, setDialog] = useState<"copy" | "sync" | null>(null);
  const [snapName, setSnapName] = useState("");
  const autoOptions = useStore((s) => s.autoOptions);
  const setAutoOptions = useStore((s) => s.setAutoOptions);
  const [report, setReport] = useState<AutoReport | null>(null);
  const [autoBusy, setAutoBusy] = useState(false);
  const [aiAvailable, setAiAvailable] = useState(false);
  const [aiProgress, setAiProgress] = useState<number | null>(null);
  /** Vista dividida antes/depois e posição da divisória (0–1). */
  const [split, setSplit] = useState(false);
  const [splitX, setSplitX] = useState(0.5);
  const beforeCanvas = useRef<HTMLCanvasElement>(null);
  // Presets: pré-visualização ao passar o rato, Amount e diálogos.
  const presets = useStore((s) => s.presets);
  const reloadPresets = useStore((s) => s.reloadPresets);
  const [preview, setPreview] = useState<DevelopSettings | null>(null);
  const [presetAmount, setPresetAmount] = useState(100);
  const [presetDialog, setPresetDialog] = useState<"create" | "manage" | null>(null);
  const [importReport, setImportReport] = useState<PresetImportResult | null>(null);
  const hoverSeq = useRef(0);
  useEffect(() => {
    if (presets.length === 0) reloadPresets().catch(() => {});
  }, [presets.length, reloadPresets]);
  const stage = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [box, setBox] = useState({ w: 900, h: 600 });
    const committed = useRef<DevelopSettings>(defaultSettings());
  /** A última alteração foi interativa (arrasto em curso) → render rápido. */
  const interactive = useRef(false);
  const fail = useCallback((e: unknown) => toast("error", errorText(e, t)), [toast, t]);
  const mod = isMac() ? "⌘" : "Ctrl";

  useLayoutEffect(() => {
    const el = stage.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth || 900, h: el.clientHeight || 600 }));
    ro.observe(el);
    setBox({ w: el.clientWidth || 900, h: el.clientHeight || 600 });
    return () => ro.disconnect();
  }, []);

  const applyView = useCallback((v: EditView) => {
    setView(v);
    setSettings(v.settings);
    committed.current = v.settings;
  }, []);

  useEffect(() => {
    api.aiDenoiseAvailable().then(setAiAvailable).catch(() => setAiAvailable(false));
  }, []);

  // Fotografia nova: carrega o estado de edição.
  useEffect(() => {
    setFrame(null);
    setCropMode(false);
    setBefore(false);
    setReport(null);
    setAiProgress(null);
    api.editView(photoKey).then(applyView).catch(fail);
  }, [photoKey, applyView, fail]);

  const onFrame = useCallback((f: Frame) => {
    setFrame(f);
    const c = canvas.current;
    const ctx = c?.getContext?.("2d");
    if (!c || !ctx) return;
    c.width = f.width;
    c.height = f.height;
    try {
      ctx.putImageData(new ImageData(new Uint8ClampedArray(f.rgba), f.width, f.height), 0, 0);
    } catch {
      /* ImageData indisponível (testes) */
    }
  }, []);
  const renderer = useRenderer(photoKey, preview ?? settings, before, cropMode, box, onFrame, fail);
  // A pré-visualização de um preset pede um render quando muda.
  useEffect(() => {
    if (view) renderer.request(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preview]);
  const loaded = view !== null && view.key === photoKey;
  // AI Denoise calculado em segundo plano: progresso e novo render quando acaba.
  useEffect(() => {
    let un: (() => void) | undefined;
    let alive = true;
    void events
      .aiDenoise((p) => {
        if (p.key !== photoKey) return;
        if (p.finished) {
          setAiProgress(null);
          renderer.request(false);
        } else setAiProgress(p.total > 0 ? p.done / p.total : 0);
      })
      .then((u) => (alive ? (un = u) : u()))
      .catch(() => {});
    return () => {
      alive = false;
      un?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [photoKey]);
  // Pedido de render quando muda algo que não é interação contínua.
  useEffect(() => {
    if (loaded) renderer.request(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loaded, before, cropMode, box.w, box.h, photoKey]);

  const commit = useCallback(
    async (label: string, s: DevelopSettings) => {
      if (sameSettings(s, committed.current)) return;
      committed.current = s;
      try {
        const v = await api.commitEdit(photoKey, label, s);
        setView(v);
        void reloadPhotos();
      } catch (e) {
        fail(e);
      }
    },
    [photoKey, fail, reloadPhotos],
  );

    const set: SetField = useCallback(
    (label, mutate, done) => {
      interactive.current = !done;
      setSettings((cur) => {
        const next = cloneSettings(cur);
        mutate(next);
        if (done) void commit(label, next);
        return next;
      });
    },
    [commit],
  );
  // Cada alteração dos ajustes pede um render (depois de o estado estar atualizado).
    // No modo de recorte a imagem mostrada não depende do retângulo: arrastá-lo não re-renderiza.
  const lastNoCrop = useRef("");
  useEffect(() => {
    const noCrop = JSON.stringify({ ...settings, geometry: { ...settings.geometry, crop: null } });
    const onlyCrop = cropMode && noCrop === lastNoCrop.current;
    lastNoCrop.current = noCrop;
    if (loaded && !onlyCrop) renderer.request(interactive.current);
    interactive.current = false;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings]);

    const replace = (v: EditView) => {
    applyView(v);
    void reloadPhotos();
  };
  const runAuto = async (options: AutoOptions, keepReport = true) => {
    setAutoBusy(true);
    try {
      const r = await api.autoEditPhoto(photoKey, options);
      replace(r.view);
      if (keepReport) setReport(r.report);
      return r;
    } catch (e) {
      fail(e);
      return null;
    } finally {
      setAutoBusy(false);
    }
  };
  const retouchOnly = async (kind: "redEye" | "dust") => {
    const r = await runAuto(onlyRetouch(kind), false);
    if (!r) return;
    const n = kind === "redEye" ? r.report.redEyes : r.report.spots;
    toast("info", n > 0 ? t(`develop.retouch.found.${kind}`, { count: n }) : t(`develop.retouch.none.${kind}`));
  };
  const hoverPreset = (p: Preset | null) => {
    const seq = ++hoverSeq.current;
    if (!p) return setPreview(null);
    api
      .presetPreviewSettings(settings, p.id, presetAmount)
      .then((s) => seq === hoverSeq.current && setPreview(s))
      .catch(fail);
  };
  const applyPresetHere = async (p: Preset) => {
    try {
      const s = await api.presetPreviewSettings(settings, p.id, presetAmount);
      hoverSeq.current++;
      setPreview(null);
      set(`preset:${p.name}`, (cur) => void Object.assign(cur, s), true);
    } catch (e) {
      fail(e);
    }
  };
  const smartSync = async (o: MatchOptions) => {
    setDialog(null);
    const keys = [...selection.ids].filter((k) => k !== photoKey);
    if (keys.length === 0) return toast("info", t("develop.syncNeedSelection"));
    try {
      const total = await api.startSync(photoKey, keys, o);
      useStore.getState().setAutoProgress({ kind: "sync", total, done: 0, failed: 0, paused: false, cancelled: false, finished: false, current: null });
    } catch (e) {
      fail(e);
    }
  };
  const undo = () => api.undoEdit(photoKey).then(replace).catch(fail);
  const redo = () => api.redoEdit(photoKey).then(replace).catch(fail);
  const reset = () => api.resetEdit(photoKey).then(replace).catch(fail);

  // Atalhos do editor.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
            // Só os campos de texto “prendem” o teclado; deslizadores e caixas não (depois de
      // arrastar um deslizador, o foco fica nele e os atalhos têm de continuar a funcionar).
      const el = e.target as HTMLInputElement | null;
      const typing = !!el && (el.tagName === "TEXTAREA" || el.tagName === "SELECT" || (el.tagName === "INPUT" && !["range", "checkbox", "radio", "button"].includes(el.type)));
      if (typing) return;
      if (useStore.getState().helpOpen || dialog) return;
      const m = isMac() ? e.metaKey : e.ctrlKey;
      const k = e.key.toLowerCase();
      if (m && k === "z" && !e.shiftKey) return e.preventDefault(), void undo();
      if ((m && k === "z" && e.shiftKey) || (m && k === "y")) return e.preventDefault(), void redo();
      if (m && e.shiftKey && k === "c") return e.preventDefault(), setDialog("copy");
      if (m && e.shiftKey && k === "v") return e.preventDefault(), void paste();
      if (m && k === "'") return e.preventDefault(), void virtualCopy();
      if (!m && !e.altKey && e.key === "\\") return e.preventDefault(), setBefore((b) => !b);
      if (!m && !e.altKey && k === "r") return e.preventDefault(), setCropMode((c) => !c);
      if (!m && !e.altKey && k === "w") return e.preventDefault(), setWbPicking((w) => !w);
      if (!m && !e.altKey && k === "y") return e.preventDefault(), setSplit((v) => !v);
      if (m && !e.shiftKey && k === "u") return e.preventDefault(), void runAuto(autoOptions);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const paste = async () => {
    if (!clipboard) return toast("info", t("develop.clipboardEmpty"));
    try {
      await api.pasteSettings([photoKey], clipboard.settings, clipboard.blocks);
      replace(await api.editView(photoKey));
      toast("info", t("develop.pasted", { count: 1 }));
    } catch (e) {
      fail(e);
    }
  };
  const sync = async (blocks: string[]) => {
    setDialog(null);
    const keys = [...selection.ids].filter((k) => k !== photoKey);
    if (keys.length === 0) return toast("info", t("develop.syncNeedSelection"));
    try {
      const n = await api.pasteSettings(keys, settings, blocks);
      void reloadPhotos();
      toast("info", t("develop.pasted", { count: n }));
    } catch (e) {
      fail(e);
    }
  };
  const virtualCopy = async () => {
    try {
      const k = await api.createVirtualCopy(photoKey);
      await reloadPhotos();
      onNavigate(k);
      toast("info", t("develop.virtualCopyCreated"));
    } catch (e) {
      fail(e);
    }
  };

  // Vista dividida: o "antes" (original com o mesmo enquadramento) ao mesmo tamanho do frame.
  const geoKey = JSON.stringify(settings.geometry);
  useEffect(() => {
    if (!split || !frame || before || cropMode) return;
    let alive = true;
    api
      .renderEdit(photoKey, beforeForSplit(settings), frame.width, frame.height, "final", false)
      .then((buf) => {
        const f = parseFrame(buf);
        const c = beforeCanvas.current;
        const ctx = c?.getContext?.("2d");
        if (!alive || !c || !ctx) return;
        c.width = f.width;
        c.height = f.height;
        try {
          ctx.putImageData(new ImageData(new Uint8ClampedArray(f.rgba), f.width, f.height), 0, 0);
        } catch {
          /* testes */
        }
      })
      .catch(fail);
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [split, photoKey, geoKey, frame?.width, frame?.height, before, cropMode]);
  const dragSplit = useRef(false);

  // Posição de um clique no canvas em coordenadas normalizadas da imagem.
  const uvOf = (e: { clientX: number; clientY: number }) => {
    const r = canvas.current!.getBoundingClientRect();
    return [(e.clientX - r.left) / (r.width || 1), (e.clientY - r.top) / (r.height || 1)] as const;
  };
  const pickWb = async (e: React.MouseEvent) => {
    if (!wbPicking) return;
    const [u, v] = uvOf(e);
    setWbPicking(false);
    try {
      const r = await api.pickWhiteBalance(photoKey, settings, u, v);
      if (!r) return toast("info", t("develop.wbNoSample"));
      set("wb.picker", (s) => {
        s.wb.temperature = Math.round(r[0]);
        s.wb.tint = Math.round(r[1]);
      }, true);
    } catch (err) {
      fail(err);
    }
  };

  // Tamanho visível da imagem dentro do palco (para o recorte sobreposto).
  const shown = frame ? (() => {
    const k = Math.min(box.w / frame.width, box.h / frame.height);
    return { w: frame.width * k, h: frame.height * k };
  })() : null;

  // ---- recorte
  const crop = settings.geometry.crop;
  const aspectRatio = (): number | null => {
    const a = settings.geometry.aspect;
    if (a === "free") return null;
    if (a === "original") return frame && cropMode ? frame.width / frame.height : null;
    return ASPECTS.find((x) => x[0] === a)?.[1] ?? null;
  };
  const dragCrop = useRef<{ mode: string; x: number; y: number; c: typeof crop } | null>(null);
  const onCropDown = (mode: string) => (e: React.PointerEvent) => {
    e.stopPropagation();
    (e.target as Element).setPointerCapture?.(e.pointerId);
    dragCrop.current = { mode, x: e.clientX, y: e.clientY, c: { ...crop } };
  };
  const onCropMove = (e: React.PointerEvent) => {
    const d = dragCrop.current;
    if (!d || !shown) return;
    const dx = (e.clientX - d.x) / shown.w;
    const dy = (e.clientY - d.y) / shown.h;
    let { x, y, w, h } = d.c;
    if (d.mode === "move") {
      x = Math.min(Math.max(x + dx, 0), 1 - w);
      y = Math.min(Math.max(y + dy, 0), 1 - h);
    } else {
      if (d.mode.includes("l")) {
        const nx = Math.min(Math.max(x + dx, 0), x + w - 0.05);
        w += x - nx;
        x = nx;
      }
      if (d.mode.includes("r")) w = Math.min(Math.max(w + dx, 0.05), 1 - x);
      if (d.mode.includes("t")) {
        const ny = Math.min(Math.max(y + dy, 0), y + h - 0.05);
        h += y - ny;
        y = ny;
      }
      if (d.mode.includes("b")) h = Math.min(Math.max(h + dy, 0.05), 1 - y);
      const ratio = aspectRatio();
      if (ratio && frame) ({ x, y, w, h } = fitCropToAspect({ x, y, w, h }, ratio, frame.width, frame.height));
    }
    setSettings((s) => ({ ...s, geometry: { ...s.geometry, crop: { x, y, w, h } } }));
  };
  const onCropUp = () => {
    if (!dragCrop.current) return;
    dragCrop.current = null;
    void commit("geometry.crop", settings);
  };

  const geometry = (
    <div className="dev-geometry">
      <div className="dev-row">
        <button type="button" className={`btn small${cropMode ? " active" : ""}`} aria-pressed={cropMode} onClick={() => setCropMode(!cropMode)} title={`${t("develop.cropTool")} (R)`}>
          ⛶ {t("develop.cropTool")}
        </button>
        <label className="small">
          <span className="sr-only">{t("develop.aspect")}</span>
          <select
            aria-label={t("develop.aspect")}
            value={settings.geometry.aspect}
            onChange={(e) => {
              const a = e.target.value;
              set("geometry.aspect", (s) => {
                s.geometry.aspect = a;
                const ratio = ASPECTS.find((x) => x[0] === a)?.[1];
                const fw = frame?.width ?? 3;
                const fh = frame?.height ?? 2;
                const r = a === "original" ? fw / fh : ratio;
                if (r) s.geometry.crop = fitCropToAspect(s.geometry.crop, r, fw, fh);
              }, true);
            }}
          >
            {ASPECTS.map(([a]) => (
              <option key={a} value={a}>
                {t(`develop.aspects.${a}`, { defaultValue: a })}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="dev-row">
        <button type="button" className="btn small" onClick={() => set("geometry.rotateLeft", (s) => void (s.geometry.quarterTurns = (s.geometry.quarterTurns + 3) % 4), true)} title={t("develop.rotateLeft")}>
          ⟲
        </button>
        <button type="button" className="btn small" onClick={() => set("geometry.rotateRight", (s) => void (s.geometry.quarterTurns = (s.geometry.quarterTurns + 1) % 4), true)} title={t("develop.rotateRight")}>
          ⟳
        </button>
        <button type="button" className={`btn small${settings.geometry.flipHorizontal ? " active" : ""}`} onClick={() => set("geometry.flip", (s) => void (s.geometry.flipHorizontal = !s.geometry.flipHorizontal), true)}>
          ⇋ {t("develop.flip")}
        </button>
        <button
          type="button"
          className="btn small"
          onClick={async () => {
            try {
              const a = await api.autoStraighten(photoKey, settings);
              if (a === null) return toast("info", t("develop.autoStraightenNone"));
              set("geometry.autoStraighten", (s) => void (s.geometry.straighten = Math.round(a * 10) / 10), true);
            } catch (e) {
              fail(e);
            }
          }}
        >
          {t("develop.autoStraighten")}
        </button>
      </div>
      <Slider label={t("develop.field.geometry.straighten")} value={settings.geometry.straighten} min={-45} max={45} step={0.1} digits={1} onChange={(v, done) => set("geometry.straighten", (s) => void (s.geometry.straighten = v), done)} />
      <Slider label={t("develop.field.geometry.vertical")} value={settings.geometry.vertical} min={-100} max={100} onChange={(v, done) => set("geometry.vertical", (s) => void (s.geometry.vertical = v), done)} />
      <Slider label={t("develop.field.geometry.horizontal")} value={settings.geometry.horizontal} min={-100} max={100} onChange={(v, done) => set("geometry.horizontal", (s) => void (s.geometry.horizontal = v), done)} />
      <button type="button" className="btn small ghost" onClick={() => set("reset.geometry", (s) => void (s.geometry = defaultSettings().geometry), true)}>
        ↺ {t("develop.resetGeometry")}
      </button>
    </div>
  );

  const idx = order.indexOf(photoKey);
  return (
    <div className="develop">
      <aside className="develop-side" aria-label={t("develop.historyTitle")}>
        <h3 className="sidebar-title">{t("develop.historyTitle")}</h3>
        <ol className="history-list">
          {(view?.history ?? []).map((h) => (
            <li key={h.seq}>
              <button
                type="button"
                className={`side-item${view?.position === h.seq ? " active" : ""}${view && h.seq > view.position ? " future" : ""}`}
                onClick={() => api.gotoHistory(photoKey, h.seq).then(replace).catch(fail)}
              >
                {historyLabel(h.label, t)}
              </button>
            </li>
          ))}
          {view && view.history.length === 0 && <li className="small muted side-note">{t("develop.noHistory")}</li>}
        </ol>
        <h3 className="sidebar-title">{t("develop.snapshots")}</h3>
        <ul>
          {(view?.snapshots ?? []).map((sn) => (
            <li key={sn.id} className="snapshot-item">
              <button type="button" className="side-item" onClick={() => api.applySnapshot(photoKey, sn.id).then(replace).catch(fail)} title={t("develop.applySnapshot")}>
                {sn.name}
              </button>
              <button type="button" className="btn small ghost" aria-label={t("develop.deleteSnapshot", { name: sn.name })} onClick={() => api.deleteSnapshot(photoKey, sn.id).then(setView).catch(fail)}>
                ×
              </button>
            </li>
          ))}
        </ul>
        <form
          className="snapshot-form"
          onSubmit={(e) => {
            e.preventDefault();
            if (!snapName.trim()) return;
            api
              .createSnapshot(photoKey, snapName)
              .then((v) => {
                setView(v);
                setSnapName("");
              })
              .catch(fail);
          }}
        >
          <input value={snapName} onChange={(e) => setSnapName(e.target.value)} placeholder={t("develop.snapshotName")} aria-label={t("develop.snapshotName")} />
          <button type="submit" className="btn small" disabled={!snapName.trim()}>
            +
          </button>
        </form>
        {photo?.virtualCopy && (
          <button
            type="button"
            className="btn small ghost"
            onClick={async () => {
              try {
                await api.deleteVirtualCopy(photoKey);
                await reloadPhotos();
                onNavigate(photo.asset);
              } catch (e) {
                fail(e);
              }
            }}
          >
            {t("develop.deleteVirtualCopy")}
          </button>
        )}
      </aside>
      <section className="develop-main">
        <div className="develop-toolbar">
          <span className="viewer-title" title={photo?.fileName}>
            {photo?.fileName}
            {photo?.virtualCopy && <span className="badge">{t("develop.virtualCopy", { name: photo.virtualCopy })}</span>}
            <span className="muted"> · {t("viewer.position", { index: idx + 1, total: order.length })}</span>
          </span>
          <span className="spacer" />
          <button type="button" className="btn small" disabled={idx <= 0} onClick={() => onNavigate(order[idx - 1]!)} title={`${t("viewer.prev")} (←)`}>
            ‹
          </button>
          <button type="button" className="btn small" disabled={idx >= order.length - 1} onClick={() => onNavigate(order[idx + 1]!)} title={`${t("viewer.next")} (→)`}>
            ›
          </button>
          <span className="sep" />
          <button type="button" className="btn small" disabled={!view?.canUndo} onClick={() => void undo()} title={`${t("develop.undo")} (${mod}+Z)`}>
            ↶ {t("develop.undo")}
          </button>
          <button type="button" className="btn small" disabled={!view?.canRedo} onClick={() => void redo()} title={`${t("develop.redo")} (${mod}+Shift+Z)`}>
            ↷ {t("develop.redo")}
          </button>
          <button type="button" className="btn small" onClick={() => void reset()}>
            {t("develop.reset")}
          </button>
          <button type="button" className={`btn small${before ? " active" : ""}`} aria-pressed={before} onClick={() => setBefore(!before)} title={`${t("develop.beforeAfter")} (\\)`}>
            {before ? t("develop.before") : t("develop.after")}
          </button>
          <button type="button" className={`btn small${split ? " active" : ""}`} aria-pressed={split} onClick={() => setSplit(!split)} title={`${t("develop.splitHint")} (Y)`}>
            ◧ {t("develop.split")}
          </button>
          <span className="sep" />
          <button type="button" className="btn small" onClick={() => setDialog("copy")} title={`${mod}+Shift+C`}>
            {t("develop.copy")}
          </button>
          <button type="button" className="btn small" disabled={!clipboard} onClick={() => void paste()} title={`${mod}+Shift+V`}>
            {t("develop.paste")}
          </button>
          <button type="button" className="btn small" disabled={selection.ids.size < 2} onClick={() => setDialog("sync")} title={t("develop.syncHint")}>
            {t("develop.sync")}
          </button>
          <button type="button" className="btn small" onClick={() => void virtualCopy()} title={`${mod}+'`}>
            {t("develop.makeVirtualCopy")}
          </button>
        </div>
        <div ref={stage} className={`develop-stage${wbPicking ? " picking" : ""}`}>
          <div className="develop-image" style={shown ? { width: shown.w, height: shown.h } : undefined} onClick={pickWb}>
            <canvas ref={canvas} aria-label={t("develop.image")} role="img" style={shown ? { width: shown.w, height: shown.h } : undefined} />
            {split && !before && !cropMode && (
              <div
                className="split-layer"
                onPointerMove={(e) => {
                  if (!dragSplit.current) return;
                  const r = e.currentTarget.getBoundingClientRect();
                  setSplitX(Math.min(1, Math.max(0, (e.clientX - r.left) / (r.width || 1))));
                }}
                onPointerUp={() => (dragSplit.current = false)}
              >
                <canvas
                  ref={beforeCanvas}
                  className="split-before"
                  aria-label={t("develop.before")}
                  role="img"
                  style={{ clipPath: `inset(0 ${(1 - splitX) * 100}% 0 0)` }}
                />
                <div
                  className="split-handle"
                  role="slider"
                  aria-label={t("develop.splitHint")}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={Math.round(splitX * 100)}
                  tabIndex={0}
                  style={{ left: `${splitX * 100}%` }}
                  onPointerDown={(e) => {
                    e.stopPropagation();
                    (e.target as Element).setPointerCapture?.(e.pointerId);
                    dragSplit.current = true;
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "ArrowLeft") setSplitX((x) => Math.max(0, x - 0.05));
                    if (e.key === "ArrowRight") setSplitX((x) => Math.min(1, x + 0.05));
                  }}
                />
                <span className="split-label left">{t("develop.before")}</span>
                <span className="split-label right">{t("develop.after")}</span>
              </div>
            )}
            {cropMode && !before && (
              <div className="crop-layer" onPointerMove={onCropMove} onPointerUp={onCropUp}>
                <div
                  className="crop-rect"
                  role="group"
                  aria-label={t("develop.cropArea")}
                  style={{ left: `${crop.x * 100}%`, top: `${crop.y * 100}%`, width: `${crop.w * 100}%`, height: `${crop.h * 100}%` }}
                  onPointerDown={onCropDown("move")}
                >
                  {["tl", "tr", "bl", "br"].map((h) => (
                    <span key={h} className={`crop-handle h-${h}`} onPointerDown={onCropDown(h)} />
                  ))}
                </div>
              </div>
            )}
          </div>
          {!frame && <div className="viewer-loading">{t("develop.loading")}</div>}
          {before && <div className="before-badge">{t("develop.before")}</div>}
          {preview && <div className="preview-badge">{t("presets.previewing")}</div>}
          {aiProgress !== null && settings.detail.aiDenoise > 0 && (
            <div className="ai-badge" role="status">
              {t("develop.aiRunning", { pct: Math.round(aiProgress * 100) })}
            </div>
          )}
        </div>
      </section>
      <DevelopPanel
        settings={settings}
        set={set}
        histogram={frame?.histogram ?? null}
        wbPicking={wbPicking}
        onPickWb={() => setWbPicking(!wbPicking)}
        geometry={geometry}
        presets={
          <div className="dev-presets">
            <Slider label={t("presets.amount")} value={presetAmount} min={0} max={150} def={100} onChange={(v) => setPresetAmount(v)} />
            <PresetList presets={presets} onPick={(p) => void applyPresetHere(p)} onHover={hoverPreset} />
            <div className="dev-row">
              <button type="button" className="btn small" onClick={() => setPresetDialog("create")}>
                + {t("presets.create")}
              </button>
              <button type="button" className="btn small" onClick={() => void importPresets(t, setImportReport)}>
                {t("presets.import")}
              </button>
              <button type="button" className="btn small" onClick={() => setPresetDialog("manage")}>
                {t("presets.manage")}
              </button>
            </div>
          </div>
        }
        ai={{ available: aiAvailable, progress: settings.detail.aiDenoise > 0 ? aiProgress : null }}
        auto={
          <div className="dev-auto">
            <AutoOptionsForm options={autoOptions} onChange={setAutoOptions} aiAvailable={aiAvailable} />
            <button type="button" className="btn primary" disabled={autoBusy || !loaded} onClick={() => void runAuto(autoOptions)} title={`${t("auto.runHint")} (${mod}+U)`}>
              {autoBusy ? t("auto.running") : `✦ ${t("auto.run")}`}
            </button>
            {report && (
              <AutoReportView
                report={report}
                onApplyAi={aiAvailable && settings.detail.aiDenoise === 0 ? () => set("detail.aiDenoise", (s) => void (s.detail.aiDenoise = 50), true) : undefined}
              />
            )}
          </div>
        }
        retouch={
          <div className="dev-retouch">
            {(["redEye", "dust"] as const).map((kind) => {
              const n = kind === "redEye" ? settings.retouch.redEyes.length : settings.retouch.spots.length;
              return (
                <div key={kind} className="dev-row">
                  <span className="small">{t(`develop.retouch.${kind}`, { count: n })}</span>
                  <span className="spacer" />
                  <button type="button" className="btn small" disabled={autoBusy} onClick={() => void retouchOnly(kind)}>
                    {t("develop.retouch.detect")}
                  </button>
                  <button
                    type="button"
                    className="btn small ghost"
                    disabled={n === 0}
                    onClick={() =>
                      set(`retouch.${kind}`, (s) => {
                        if (kind === "redEye") s.retouch.redEyes = [];
                        else s.retouch.spots = [];
                      }, true)
                    }
                  >
                    {t("develop.retouch.remove")}
                  </button>
                </div>
              );
            })}
            <p className="small muted dev-note">{t("develop.retouch.note")}</p>
          </div>
        }
      />
      {dialog === "copy" && (
        <CopyDialog
          title={t("develop.copyTitle")}
          action={t("develop.copy")}
          onClose={() => setDialog(null)}
          onConfirm={(blocks) => {
            setClipboard({ settings: cloneSettings(settings), blocks });
            setDialog(null);
            toast("info", t("develop.copied"));
          }}
        />
      )}
      {dialog === "sync" && (
        <CopyDialog
          title={t("develop.syncTitle", { count: selection.ids.size - 1 })}
          action={t("develop.sync")}
          onClose={() => setDialog(null)}
          onConfirm={(b) => void sync(b)}
          onSmart={(o) => void smartSync(o)}
        />
      )}
      {presetDialog === "create" && <CreatePresetDialog settings={settings} onClose={() => setPresetDialog(null)} />}
      {presetDialog === "manage" && <PresetManager onClose={() => setPresetDialog(null)} />}
      {importReport && <ImportReportDialog result={importReport} onClose={() => setImportReport(null)} />}

    </div>
  );
}
