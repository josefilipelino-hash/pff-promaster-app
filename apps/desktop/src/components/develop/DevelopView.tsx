import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, events, isMac } from "../../api";
import { Icon } from "../Icon";
import { ASPECTS, cloneSettings, DEFAULT_COPY_BLOCKS, defaultSettings, fitCropToAspect, parseFrame, sameSettings, type Frame } from "../../logic/develop";
import { errorText, useStore } from "../../store";
import { SETTINGS_BLOCKS, UPRIGHT_MODES, type UprightOutcome, type AutoOptions, type AutoReport, type DevelopSettings, type EditView } from "../../types";
import { AutoOptionsForm, AutoReportView } from "./AutoPanel";
import { CreatePresetDialog, ImportReportDialog, importPresets, PresetList, PresetManager } from "../Presets";
import { MasksPanel, PortraitPanel, RetouchTools, type BrushSettings, type Tool } from "./LocalPanels";
import { longDist, newMask, unitScale } from "../../logic/develop";
import { RASTER_KINDS, type MaskComponent, type MaskKind, type MaskOp } from "../../types";
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
  for (const prefix of ["masks", "portrait", "retouch"]) if (label.startsWith(`${prefix}.`)) return t(`develop.history.${prefix}`);
  return label;
}

/**
 * Renderização com qualidade dinâmica: um pedido de cada vez; durante a interação, render
 * rápido (resolução menor, sem nitidez/ruído); ao parar, render final à resolução do ecrã.
 */
function useRenderer(key: number, settings: DevelopSettings, before: boolean, uncropped: boolean, box: { w: number; h: number }, onFrame: (f: Frame) => void, fail: (e: unknown) => void, overlay: number | null = null) {
  const st = useRef({ inFlight: false, dirty: false, lastChange: 0, timer: 0 as unknown as ReturnType<typeof setTimeout> | 0 });
  const latest = useRef({ key, settings, before, uncropped, box, overlay });
  latest.current = { key, settings, before, uncropped, box, overlay };
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
      const buf = await api.renderEdit(L.key, L.before ? defaultSettings() : L.settings, w, h, quality, L.uncropped && !L.before, L.before ? null : L.overlay);
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

/** Blocos sincronizáveis que mudaram (os próprios de cada fotografia nunca vão). */
const SYNC_EXCLUDED = new Set(["geometry", "retouch", "masks", "portrait"]);
export function changedSyncBlocks(a: DevelopSettings, b: DevelopSettings): string[] {
  const ra = a as unknown as Record<string, unknown>;
  const rb = b as unknown as Record<string, unknown>;
  return SETTINGS_BLOCKS.filter((k) => !SYNC_EXCLUDED.has(k) && JSON.stringify(ra[k]) !== JSON.stringify(rb[k]));
}

export function DevelopView({ photoKey, order, onNavigate, filmstrip }: { photoKey: number; order: number[]; onNavigate: (key: number) => void; filmstrip?: React.ReactNode }) {
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
  // RAW: temperatura do balanço da câmara (K), pedida depois do primeiro render (a fonte já
  // está em memória) — a temperatura passa a ser mostrada em Kelvin.
  const [asShot, setAsShot] = useState<{ key: number; k: number | null } | null>(null);
  const hasFrame = frame !== null;
  useEffect(() => {
    if (!hasFrame) return;
    let live = true;
    api.asShotKelvin(photoKey)
      .then((k) => live && setAsShot({ key: photoKey, k }))
      .catch(() => live && setAsShot({ key: photoKey, k: null }));
    return () => {
      live = false;
    };
  }, [photoKey, hasFrame]);
  const [before, setBefore] = useState(false);
  const [cropMode, setCropMode] = useState(false);
  const [wbPicking, setWbPicking] = useState(false);
  const [dialog, setDialog] = useState<"copy" | "sync" | null>(null);
  const [uprightBusy, setUprightBusy] = useState(false);
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
  // Edição localizada: máscara selecionada, ferramenta sobre a imagem, pincel.
  const [selMask, setSelMask] = useState<number | null>(null);
  const [tool, setTool] = useState<Tool>(null);
  const [brush, setBrush] = useState<BrushSettings>({ size: 3, feather: 50, flow: 100 });
  const [maskOverlay, setMaskOverlay] = useState(true);
  const [localBusy, setLocalBusy] = useState(false);
  /** Parte em criação: operação e máscara de destino (para pincel/gradientes/cliques). */
  const pendingOp = useRef<{ op: MaskOp; mask: number }>({ op: "add", mask: 0 });
  const [drag, setDrag] = useState<{ pts: [number, number][] } | null>(null);
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

  // Fase 15 — nunca escrever os ajustes de uma fotografia noutra. O editor não é recriado
  // ao mudar de fotografia: um resultado assíncrono (máscara IA, retrato, Auto, conta-gotas,
  // desfazer…) que chegue depois de o utilizador avançar pertencia à fotografia anterior.
  // `photoKeyRef` é a fotografia mostrada agora; `viewKeyRef` aquela cujos ajustes estão
  // em `settings`.
  const photoKeyRef = useRef(photoKey);
  photoKeyRef.current = photoKey;
  const viewKeyRef = useRef<number | null>(null);
  const commitChain = useRef<Promise<void>>(Promise.resolve());
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const applyView = useCallback((v: EditView) => {
    if (v.key !== photoKeyRef.current) return; // resposta atrasada de outra fotografia
    setView(v);
    setSettings(v.settings);
    committed.current = v.settings;
    viewKeyRef.current = v.key;
  }, []);
  // Ao sair de uma fotografia (seguinte, outra vista, fechar): o último ajuste ainda por
  // gravar (ex.: rato largado fora do controlo) é gravado NA SUA fotografia.
  useEffect(() => {
    const key = photoKey;
    return () => {
      if (viewKeyRef.current === key && !sameSettings(settingsRef.current, committed.current)) {
        const s = settingsRef.current;
        committed.current = s;
        api.commitEdit(key, "adjust", s).catch(() => {});
      }
    };
  }, [photoKey]);

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
    setSelMask(null);
    setTool(null);
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
  // Upright guiado: enquanto se desenham as guias, a fotografia aparece sem perspetiva nem
  // recorte — as coordenadas da vista são as do plano onde as guias são definidas.
  const [guiding, setGuiding] = useState(false);
  const [guideDrag, setGuideDrag] = useState<[number, number][] | null>(null);
  const guideView = useMemo(() => {
    if (!guiding) return null;
    const s = cloneSettings(settings);
    Object.assign(s.geometry, { vertical: 0, horizontal: 0, straighten: 0, aspectAdjust: 0, scale: 100, offsetX: 0, offsetY: 0 });
    return s;
  }, [guiding, settings]);
  const renderer = useRenderer(photoKey, preview ?? guideView ?? settings, before, cropMode || guiding, box, onFrame, fail, maskOverlay && selMask !== null && selMask < settings.masks.length ? selMask : null);
  useEffect(() => {
    if (view) renderer.request(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selMask, maskOverlay]);
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
  }, [loaded, before, cropMode, guiding, box.w, box.h, photoKey]);

  // Sincronização automática (como o "Auto Sync" do Lightroom): com várias fotografias
  // selecionadas, cada alteração gravada aqui vai também para as outras selecionadas — só os
  // blocos que mudaram e nunca os próprios de cada fotografia (recorte, máscaras, retoque).
  const [autoSync, setAutoSyncState] = useState(() => {
    try {
      return localStorage.getItem("pff.autoSync") === "1";
    } catch {
      return false;
    }
  });
  const setAutoSync = (on: boolean) => {
    setAutoSyncState(on);
    try {
      localStorage.setItem("pff.autoSync", on ? "1" : "0");
    } catch {
      /* sem armazenamento: só nesta sessão */
    }
  };
  const syncTargets = useMemo(
    () => (selection.ids.has(photoKey) ? [...selection.ids].filter((k) => k !== photoKey) : []),
    [selection, photoKey],
  );
  const syncRef = useRef({ on: autoSync, targets: syncTargets, skipOnce: false });
  syncRef.current.on = autoSync;
  syncRef.current.targets = syncTargets;

  const commit = useCallback(
    async (label: string, s: DevelopSettings) => {
      // Só para a fotografia mostrada e com os SEUS ajustes carregados.
      if (photoKey !== photoKeyRef.current || viewKeyRef.current !== photoKey) return;
      if (sameSettings(s, committed.current)) return;
      const before = committed.current;
      committed.current = s;
      const sync = syncRef.current;
      const skip = sync.skipOnce;
      sync.skipOnce = false;
      if (sync.on && !skip && sync.targets.length > 0) {
        const blocks = changedSyncBlocks(before, s);
        if (blocks.length > 0) {
          api.pasteSettings(sync.targets, s, blocks).then(() => void reloadPhotos()).catch(fail);
        }
      }
      // Gravações em série: duas seguidas (ex.: nova máscara + pincel) nunca chegam ao
      // catálogo pela ordem inversa (Fase 15).
      const prev = commitChain.current;
      let release = () => {};
      commitChain.current = new Promise<void>((r) => (release = r));
      try {
        await prev;
        const v = await api.commitEdit(photoKey, label, s);
        if (v.key === photoKeyRef.current) setView(v);
        void reloadPhotos();
      } catch (e) {
        fail(e);
      } finally {
        release();
      }
    },
    [photoKey, fail, reloadPhotos],
  );

    const set: SetField = useCallback(
    (label, mutate, done) => {
      // Alteração vinda de uma operação começada noutra fotografia, ou antes de os ajustes
      // desta estarem carregados: ignorada (antes misturava-se com a fotografia atual).
      if (photoKey !== photoKeyRef.current || viewKeyRef.current !== photoKey) return;
      interactive.current = !done;
      setSettings((cur) => {
        const next = cloneSettings(cur);
        mutate(next);
        if (done) void commit(label, next);
        return next;
      });
    },
    [commit, photoKey],
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
      // Várias selecionadas: o preset é aplicado a cada uma (relativo ao que cada uma tem),
      // não copiado desta.
      const others = syncRef.current.targets;
      if (others.length > 0) {
        syncRef.current.skipOnce = true;
        const n = await api.applyPreset(others, p.id, presetAmount);
        void reloadPhotos();
        toast("info", t("presets.applied", { name: p.name, count: n + 1 }));
      }
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
  // ---- edição localizada
  const [ru, rv] = unitScale(photo?.width ?? 3, photo?.height ?? 2);
  /** Acrescenta uma parte à máscara (ou cria uma máscara nova). */
  const addKind = async (kind: MaskKind, op: MaskOp, mask: number | null) => {
    let idx = mask;
    const pushComp = (c: MaskComponent) =>
      set(`masks.add.${kind}`, (s) => {
        if (idx === null || idx >= s.masks.length) {
          s.masks.push({ ...newMask(t(`masks.kind.${kind}`)), components: [c] });
          idx = s.masks.length - 1;
        } else s.masks[idx]!.components.push(c);
      }, true);
    const target = () => (idx === null ? settings.masks.length : idx);
    if (RASTER_KINDS.includes(kind) && kind !== "object") {
      setLocalBusy(true);
      try {
        const r = await api.createMaskRaster(photoKey, kind, null);
        if (r.coverage < 0.002) toast("info", t("masks.emptyResult", { kind: t(`masks.kind.${kind}`) }));
        const i = target();
        pushComp({ op, invert: false, shape: { type: "raster", kind, raster: r.raster, feather: 0 } });
        setSelMask(i);
      } catch (e) {
        fail(e);
      } finally {
        setLocalBusy(false);
      }
      return;
    }
    if (kind === "luminance") {
      const i = target();
      pushComp({ op, invert: false, shape: { type: "luminance", min: 0.5, max: 1, smooth: 0.1 } });
      return setSelMask(i);
    }
    // Pincel, gradientes, cor e objeto: a parte é criada na imagem.
    if (idx === null) {
      const m = newMask(t(`masks.kind.${kind}`));
      set("masks.new", (s) => void s.masks.push(m), true);
      idx = settings.masks.length;
    }
    setSelMask(idx);
    pendingOp.current = { op, mask: idx };
    if (kind === "brush") {
      set("masks.brush", (s) => void s.masks[idx!]?.components.push({ op, invert: false, shape: { type: "brush", strokes: [] } }), true);
      setTool({ kind: "brush", erase: false });
    } else if (kind === "linear") setTool({ kind: "linear" });
    else if (kind === "radial") setTool({ kind: "radial" });
    else setTool({ kind: "point", purpose: kind === "color" ? "color" : "object" });
    toast("info", t(`masks.hint.${kind === "object" || kind === "color" ? "point" : kind}`));
  };

  const viewUv = (e: { clientX: number; clientY: number }): [number, number] => {
    const r = canvas.current!.getBoundingClientRect();
    return [Math.min(1, Math.max(0, (e.clientX - r.left) / (r.width || 1))), Math.min(1, Math.max(0, (e.clientY - r.top) / (r.height || 1)))];
  };
  const addGuide = async (a: [number, number], b: [number, number]) => {
    if (Math.hypot(b[0] - a[0], b[1] - a[1]) < 0.03) return;
    const guides = [...(settings.geometry.guides ?? []), [a[0], a[1], b[0], b[1]] as [number, number, number, number]].slice(-4);
    const next = cloneSettings(settings);
    next.geometry.guides = guides;
    let r: UprightOutcome | null = null;
    if (guides.length >= 2) {
      try {
        r = await api.upright(photoKey, next, "guided");
      } catch (e) {
        fail(e);
      }
    }
    set("geometry.upright", (s) => {
      s.geometry.guides = guides;
      s.geometry.upright = "guided";
      if (r) {
        s.geometry.vertical = Math.round(r.vertical * 10) / 10;
        s.geometry.horizontal = Math.round(r.horizontal * 10) / 10;
        s.geometry.straighten = Math.round(r.straighten * 100) / 100;
        s.geometry.focal35 = r.focal35;
      }
    }, true);
  };
  const toSource = async (pts: [number, number][]) => (await api.mapPoints(photoKey, settings, pts, true, false)).map((p, i) => p ?? pts[i]!);

  const finishDrag = async (pts: [number, number][]) => {
    setDrag(null);
    if (!tool || pts.length === 0) return;
    try {
      const src = await toSource(pts);
      const size = brush.size / 100;
      const stroke = { points: src, size, feather: brush.feather, flow: brush.flow, erase: tool.kind === "brush" && tool.erase };
      const { op, mask } = pendingOp.current;
      if (tool.kind === "brush" && selMask !== null) {
        set("masks.stroke", (s) => {
          const m = s.masks[selMask];
          if (!m) return;
          let c = [...m.components].reverse().find((x) => x.shape.type === "brush");
          if (!c) {
            c = { op, invert: false, shape: { type: "brush", strokes: [] } };
            m.components.push(c);
          }
          (c.shape as { strokes: typeof stroke[] }).strokes.push(stroke);
        }, true);
      } else if (tool.kind === "linear" && src.length >= 2) {
        const [a, b] = [src[0]!, src[src.length - 1]!];
        set("masks.linear", (s) => void s.masks[mask]?.components.push({ op, invert: false, shape: { type: "linear", x0: a[0], y0: a[1], x1: b[0], y1: b[1] } }), true);
        setTool(null);
      } else if (tool.kind === "radial" && pts.length >= 2) {
        const c = pts[0]!;
        const e = pts[pts.length - 1]!;
        const [cs, xs, ys] = await toSource([c, [e[0], c[1]], [c[0], e[1]]]);
        const rx = Math.max(0.005, longDist(cs!, xs!, ru, rv));
        const ry = Math.max(0.005, longDist(cs!, ys!, ru, rv));
        const angle = (Math.atan2((xs![1] - cs![1]) * rv, (xs![0] - cs![0]) * ru) * 180) / Math.PI;
        set("masks.radial", (s) => void s.masks[mask]?.components.push({ op, invert: false, shape: { type: "radial", cx: cs![0], cy: cs![1], rx, ry, angle, feather: 50 } }), true);
        setTool(null);
      } else if (tool.kind === "patch") {
        const cx = src.reduce((a, p) => a + p[0], 0) / src.length;
        const cy = src.reduce((a, p) => a + p[1], 0) / src.length;
        const rad = Math.max(...src.map((p) => longDist(p, [cx, cy], ru, rv))) + size;
        const [sx, sy] = await api.suggestHealSource(photoKey, cx, cy, rad);
        set("retouch.object", (s) => void s.retouch.patches.push({ stroke: { ...stroke, erase: false, flow: 100 }, dx: sx - cx, dy: sy - cy, heal: true, opacity: 100 }), true);
      }
    } catch (err) {
      fail(err);
    }
  };

  const clickTool = async (e: React.MouseEvent) => {
    if (!tool || (tool.kind !== "point" && tool.kind !== "spot")) return;
    const uv = viewUv(e);
    try {
      const [p] = await toSource([uv]);
      if (tool.kind === "spot") {
        const r = brush.size / 100;
        const [sx, sy] = await api.suggestHealSource(photoKey, p![0], p![1], r);
        set(tool.heal ? "retouch.heal" : "retouch.clone", (s) => void s.retouch.spots.push({ x: p![0], y: p![1], r, sx, sy, heal: tool.heal, feather: 45, opacity: 100 }), true);
        return;
      }
      const { op, mask } = pendingOp.current;
      if (tool.purpose === "color") {
        // Cor amostrada no canvas (o que se vê).
        const c = canvas.current!;
        const ctx = c.getContext?.("2d");
        const d = ctx?.getImageData(Math.floor(uv[0] * c.width), Math.floor(uv[1] * c.height), 1, 1).data ?? new Uint8ClampedArray([128, 128, 128, 255]);
        const [r, g, b] = [d[0]! / 255, d[1]! / 255, d[2]! / 255];
        const mx = Math.max(r, g, b);
        const mn = Math.min(r, g, b);
        const dd = mx - mn;
        let hue = 0;
        if (dd > 1e-6) hue = mx === r ? 60 * (((g - b) / dd + 6) % 6) : mx === g ? 60 * ((b - r) / dd + 2) : 60 * ((r - g) / dd + 4);
        const sat = mx > 0 ? dd / mx : 0;
        set("masks.color", (s) => void s.masks[mask]?.components.push({ op, invert: false, shape: { type: "color", hue, range: 20, minSat: Math.max(0.08, sat * 0.5), smooth: 0.3 } }), true);
      } else {
        setLocalBusy(true);
        const r = await api.createMaskRaster(photoKey, "object", p!);
        set("masks.object", (s) => void s.masks[mask]?.components.push({ op, invert: false, shape: { type: "raster", kind: "object", raster: r.raster, feather: 0 } }), true);
      }
      setTool(null);
    } catch (err) {
      fail(err);
    } finally {
      setLocalBusy(false);
    }
  };

  const detectPortrait = async () => {
    setLocalBusy(true);
    try {
      const faces = await api.detectPortrait(photoKey);
      if (faces.length === 0) toast("info", t("portrait.noFaces"));
      set("portrait.detect", (s) => void (s.portrait.faces = faces), true);
    } catch (e) {
      fail(e);
    } finally {
      setLocalBusy(false);
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
      if (e.key === "Escape" && guiding) return e.preventDefault(), e.stopPropagation(), setGuiding(false);
      if (e.key === "Escape" && tool) return e.preventDefault(), e.stopPropagation(), setTool(null);
      if (!m && !e.altKey && k === "o" && selMask !== null) return e.preventDefault(), setMaskOverlay((v) => !v);
      if (m && !e.shiftKey && k === "u") return e.preventDefault(), void runAuto(autoOptions);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const paste = async () => {
    if (!clipboard) return toast("info", t("develop.clipboardEmpty"));
    // Como no Lightroom: com várias fotografias selecionadas (na tira), cola em todas.
    const keys = selection.ids.has(photoKey) && selection.ids.size > 1 ? [...selection.ids] : [photoKey];
    try {
      const n = await api.pasteSettings(keys, clipboard.settings, clipboard.blocks);
      replace(await api.editView(photoKey));
      if (keys.length > 1) void reloadPhotos();
      toast("info", t("develop.pasted", { count: keys.length > 1 ? n : 1 }));
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
      </div>
      <h4 className="small muted">{t("develop.upright")}</h4>
      <div className="dev-row" role="group" aria-label={t("develop.upright")} title={t("develop.uprightHint")}>
        <button
          type="button"
          className={`btn small${settings.geometry.upright === "" ? " active" : ""}`}
          onClick={() =>
            set("geometry.upright", (s) => {
              s.geometry.upright = "";
              s.geometry.guides = [];
              s.geometry.vertical = 0;
              s.geometry.horizontal = 0;
              s.geometry.straighten = 0;
            }, true)
          }
        >
          {t("develop.uprightOff")}
        </button>
        {UPRIGHT_MODES.map((m) => (
          <button
            key={m}
            type="button"
            className={`btn small${settings.geometry.upright === m ? " active" : ""}`}
            disabled={uprightBusy}
            onClick={async () => {
              setUprightBusy(true);
              try {
                const r = await api.upright(photoKey, settings, m);
                if (r === null) return toast("info", t("develop.uprightNone"));
                set("geometry.upright", (s) => {
                  s.geometry.upright = m;
                  s.geometry.vertical = Math.round(r.vertical * 10) / 10;
                  s.geometry.horizontal = Math.round(r.horizontal * 10) / 10;
                  s.geometry.straighten = Math.round(r.straighten * 100) / 100;
                  s.geometry.focal35 = r.focal35;
                }, true);
                toast(
                  "info",
                  t("develop.uprightDone", {
                    mode: t(`develop.uprightMode.${r.mode}`),
                    lines: r.verticalLines + r.horizontalLines,
                    before: r.deviationBefore.toFixed(1),
                    after: r.deviationAfter.toFixed(1),
                  }),
                );
              } catch (e) {
                fail(e);
              } finally {
                setUprightBusy(false);
              }
            }}
          >
            {t(`develop.uprightMode.${m}`)}
          </button>
        ))}
        <button
          type="button"
          className={`btn small${guiding || settings.geometry.upright === "guided" ? " active" : ""}`}
          aria-pressed={guiding}
          onClick={() => {
            setCropMode(false);
            setTool(null);
            setBefore(false);
            setGuiding((g) => !g);
          }}
        >
          {t("develop.uprightMode.guided")}
        </button>
      </div>
      {guiding && (
        <div className="dev-row">
          <p className="small muted">{t("develop.guidesHint", { count: (settings.geometry.guides ?? []).length })}</p>
          <button type="button" className="btn small" disabled={(settings.geometry.guides ?? []).length === 0} onClick={() => set("geometry.upright", (s) => void (s.geometry.guides = []), true)}>
            {t("develop.guidesClear")}
          </button>
          <button type="button" className="btn small primary" onClick={() => setGuiding(false)}>
            {t("develop.guidesDone")}
          </button>
        </div>
      )}
      <Slider label={t("develop.field.geometry.straighten")} value={settings.geometry.straighten} min={-45} max={45} step={0.1} digits={1} onChange={(v, done) => set("geometry.straighten", (s) => void (s.geometry.straighten = v), done)} />
      <Slider label={t("develop.field.geometry.vertical")} value={settings.geometry.vertical} min={-100} max={100} onChange={(v, done) => set("geometry.vertical", (s) => void (s.geometry.vertical = v), done)} />
      <Slider label={t("develop.field.geometry.horizontal")} value={settings.geometry.horizontal} min={-100} max={100} onChange={(v, done) => set("geometry.horizontal", (s) => void (s.geometry.horizontal = v), done)} />
      <Slider label={t("develop.field.geometry.aspectAdjust")} value={settings.geometry.aspectAdjust ?? 0} min={-100} max={100} onChange={(v, done) => set("geometry.aspectAdjust", (s) => void (s.geometry.aspectAdjust = v), done)} />
      <Slider label={t("develop.field.geometry.scale")} value={settings.geometry.scale ?? 100} min={100} max={200} onChange={(v, done) => set("geometry.scale", (s) => void (s.geometry.scale = v), done)} />
      <Slider label={t("develop.field.geometry.offsetX")} value={settings.geometry.offsetX ?? 0} min={-100} max={100} onChange={(v, done) => set("geometry.offsetX", (s) => void (s.geometry.offsetX = v), done)} />
      <Slider label={t("develop.field.geometry.offsetY")} value={settings.geometry.offsetY ?? 0} min={-100} max={100} onChange={(v, done) => set("geometry.offsetY", (s) => void (s.geometry.offsetY = v), done)} />
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
                if (v.key === photoKeyRef.current) setView(v);
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
          <button type="button" className="btn small icon-only" disabled={idx <= 0} onClick={() => onNavigate(order[idx - 1]!)} title={`${t("viewer.prev")} (←)`} aria-label={t("viewer.prev")}>
            <Icon name="chevronLeft" />
          </button>
          <button type="button" className="btn small icon-only" disabled={idx >= order.length - 1} onClick={() => onNavigate(order[idx + 1]!)} title={`${t("viewer.next")} (→)`} aria-label={t("viewer.next")}>
            <Icon name="chevronRight" />
          </button>
          <span className="sep" />
          <button type="button" className="btn small act" aria-label={t("develop.undo")} disabled={!view?.canUndo} onClick={() => void undo()} title={`${t("develop.undo")} (${mod}+Z)`}>
            <Icon name="undo" />
            <span className="act-label">{t("develop.undo")}</span>
          </button>
          <button type="button" className="btn small act" aria-label={t("develop.redo")} disabled={!view?.canRedo} onClick={() => void redo()} title={`${t("develop.redo")} (${mod}+Shift+Z)`}>
            <Icon name="redo" />
            <span className="act-label">{t("develop.redo")}</span>
          </button>
          <button type="button" className="btn small act" onClick={() => void reset()} aria-label={t("develop.reset")} title={t("develop.reset")}>
            <Icon name="reset" />
            <span className="act-label">{t("develop.reset")}</span>
          </button>
          <button type="button" className={`btn small act${before ? " active" : ""}`} aria-label={before ? t("develop.before") : t("develop.after")} aria-pressed={before} onClick={() => setBefore(!before)} title={`${t("develop.beforeAfter")} (\\)`}>
            <Icon name="beforeAfter" />
            <span className="act-label">{before ? t("develop.before") : t("develop.after")}</span>
          </button>
          <button type="button" className={`btn small act${split ? " active" : ""}`} aria-label={t("develop.split")} aria-pressed={split} onClick={() => setSplit(!split)} title={`${t("develop.splitHint")} (Y)`}>
            <Icon name="compare" />
            <span className="act-label">{t("develop.split")}</span>
          </button>
          <span className="sep" />
          <button type="button" className="btn small act" aria-label={t("develop.copy")} onClick={() => setDialog("copy")} title={`${mod}+Shift+C`}>
            <Icon name="copy" />
            <span className="act-label">{t("develop.copy")}</span>
          </button>
          <button type="button" className="btn small act" aria-label={t("develop.paste")} disabled={!clipboard} onClick={() => void paste()} title={`${mod}+Shift+V`}>
            <Icon name="paste" />
            <span className="act-label">{t("develop.paste")}</span>
          </button>
          <button type="button" className="btn small act" aria-label={t("develop.sync")} disabled={selection.ids.size < 2} onClick={() => setDialog("sync")} title={t("develop.syncHint")}>
            <Icon name="sync" />
            <span className="act-label">{t("develop.sync")}</span>
          </button>
          <button
            type="button"
            className={`btn small act auto-sync${autoSync && syncTargets.length > 0 ? " on" : ""}`}
            aria-pressed={autoSync}
            disabled={syncTargets.length === 0}
            onClick={() => setAutoSync(!autoSync)}
            title={syncTargets.length === 0 ? t("develop.autoSyncNeedSelection") : t("develop.autoSyncHint", { count: syncTargets.length })}
          >
            <Icon name="sync" />
            <span className="act-label">{autoSync && syncTargets.length > 0 ? t("develop.autoSyncOn", { count: syncTargets.length + 1 }) : t("develop.autoSync")}</span>
          </button>
          <button type="button" className="btn small act" aria-label={t("develop.makeVirtualCopy")} onClick={() => void virtualCopy()} title={`${mod}+'`}>
            <Icon name="duplicate" />
            <span className="act-label">{t("develop.makeVirtualCopy")}</span>
          </button>
        </div>
        <div ref={stage} className={`develop-stage${wbPicking ? " picking" : ""}`}>
          <div className="develop-image" style={shown ? { width: shown.w, height: shown.h } : undefined} onClick={pickWb}>
            <canvas ref={canvas} aria-label={t("develop.image")} role="img" style={shown ? { width: shown.w, height: shown.h } : undefined} />
            {tool && !before && !cropMode && shown && (
              <svg
                className={`tool-layer tool-${tool.kind}`}
                role="application"
                aria-label={t(`masks.toolLabel.${tool.kind}`)}
                width={shown.w}
                height={shown.h}
                onClick={(e) => void clickTool(e)}
                onPointerDown={(e) => {
                  if (tool.kind === "point" || tool.kind === "spot") return;
                  (e.target as Element).setPointerCapture?.(e.pointerId);
                  setDrag({ pts: [viewUv(e)] });
                }}
                onPointerMove={(e) => {
                  if (!drag) return;
                  const p = viewUv(e);
                  setDrag((d) => (d ? { pts: tool.kind === "brush" || tool.kind === "patch" ? [...d.pts, p] : [d.pts[0]!, p] } : d));
                }}
                onPointerUp={() => drag && void finishDrag(drag.pts)}
              >
                {drag && (tool.kind === "brush" || tool.kind === "patch") && (
                  <polyline
                    points={drag.pts.map((p) => `${p[0] * shown.w},${p[1] * shown.h}`).join(" ")}
                    fill="none"
                    stroke={tool.kind === "brush" && tool.erase ? "rgba(80,160,255,0.6)" : "rgba(255,90,90,0.6)"}
                    strokeWidth={Math.max(2, (brush.size / 100) * Math.max(shown.w, shown.h) * 2)}
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                )}
                {drag && tool.kind === "linear" && drag.pts.length > 1 && (
                  <line x1={drag.pts[0]![0] * shown.w} y1={drag.pts[0]![1] * shown.h} x2={drag.pts[1]![0] * shown.w} y2={drag.pts[1]![1] * shown.h} stroke="#fff" strokeWidth={2} />
                )}
                {drag && tool.kind === "radial" && drag.pts.length > 1 && (
                  <ellipse
                    cx={drag.pts[0]![0] * shown.w}
                    cy={drag.pts[0]![1] * shown.h}
                    rx={Math.abs(drag.pts[1]![0] - drag.pts[0]![0]) * shown.w}
                    ry={Math.abs(drag.pts[1]![1] - drag.pts[0]![1]) * shown.h}
                    fill="none"
                    stroke="#fff"
                    strokeWidth={2}
                  />
                )}
              </svg>
            )}
            {guiding && !before && shown && (
              <svg
                className="tool-layer tool-guides"
                role="application"
                aria-label={t("develop.guidesLabel")}
                width={shown.w}
                height={shown.h}
                onPointerDown={(e) => {
                  (e.target as Element).setPointerCapture?.(e.pointerId);
                  const p = viewUv(e);
                  setGuideDrag([p, p]);
                }}
                onPointerMove={(e) => guideDrag && setGuideDrag([guideDrag[0]!, viewUv(e)])}
                onPointerUp={() => {
                  if (!guideDrag) return;
                  const [a, b] = guideDrag;
                  setGuideDrag(null);
                  void addGuide(a!, b!);
                }}
              >
                {(settings.geometry.guides ?? []).map((g, i) => (
                  <g key={i} className="guide">
                    <line x1={g[0] * shown.w} y1={g[1] * shown.h} x2={g[2] * shown.w} y2={g[3] * shown.h} />
                    <circle cx={g[0] * shown.w} cy={g[1] * shown.h} r={4} />
                    <circle cx={g[2] * shown.w} cy={g[3] * shown.h} r={4} />
                  </g>
                ))}
                {guideDrag && <line className="guide-new" x1={guideDrag[0]![0] * shown.w} y1={guideDrag[0]![1] * shown.h} x2={guideDrag[1]![0] * shown.w} y2={guideDrag[1]![1] * shown.h} />}
              </svg>
            )}
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
          {!frame && <div className="viewer-loading state loading" role="status">{t("develop.loading")}</div>}
          {before && <div className="before-badge">{t("develop.before")}</div>}
          {preview && <div className="preview-badge">{t("presets.previewing")}</div>}
          {aiProgress !== null && settings.detail.aiDenoise > 0 && (
            <div className="ai-badge" role="status">
              {t("develop.aiRunning", { pct: Math.round(aiProgress * 100) })}
            </div>
          )}
        </div>
        {filmstrip}
      </section>
      <DevelopPanel
        settings={settings}
        asShotKelvin={asShot && asShot.key === photoKey ? asShot.k : null}
        set={set}
        histogram={frame?.histogram ?? null}
        wbPicking={wbPicking}
        onPickWb={() => setWbPicking(!wbPicking)}
        geometry={geometry}
        masks={
          <MasksPanel
            settings={settings}
            set={set}
            selected={selMask}
            setSelected={(i) => {
              setSelMask(i);
              if (tool?.kind !== "spot" && tool?.kind !== "patch") setTool(null);
            }}
            tool={tool}
            setTool={setTool}
            brush={brush}
            setBrush={setBrush}
            onAdd={(k, op, m) => void addKind(k, op, m)}
            overlay={maskOverlay}
            setOverlay={setMaskOverlay}
            busy={localBusy}
          />
        }
        portrait={<PortraitPanel settings={settings} set={set} onDetect={() => void detectPortrait()} busy={localBusy} />}
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
          <RetouchTools settings={settings} set={set} tool={tool} setTool={setTool} brush={brush} setBrush={setBrush} busy={autoBusy || localBusy} onDetect={(k) => void retouchOnly(k)} />
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
