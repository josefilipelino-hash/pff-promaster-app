import { create } from "zustand";
import { perfMark, perfWatchGrid } from "./logic/perfProbe";
import { api } from "./api";
import { DEFAULT_FILTER, type Filter } from "./logic/filters";
import { EMPTY_SELECTION, type Selection } from "./logic/selection";
import type { RecMap } from "./logic/filters";
import { defaultAutoOptions } from "./logic/develop";
import type {
  Preset,
  AutoEditProgress,
  ExportProgress,
  AutoOptions,
  DevelopSettings,
  AssetMoment,
  MomentRow,
  PersonRow,
  AnalysisDetails,
  AnalysisProgress,
  AnalysisStatus,
  Color,
  ErrorPayload,
  GroupRow,
  ImportProgress,
  ImportSummary,
  PhotoDetails,
  PhotoRow,
  ProjectInfo,
  ThumbBatch,
} from "./types";

export const THUMB_MIN = 100;
export const THUMB_MAX = 400;
export const THUMB_DEFAULT = 180;

export interface Toast {
  id: number;
  kind: "error" | "info";
  text: string;
}

/** Estado persistido no catálogo para reabrir o projeto como foi deixado. */
export interface UiState {
  filter: Filter;
  thumbSize: number;
  focusId: number | null;
  infoOpen: boolean;
  view?: View;
}

/** Vista principal: grelha de fotografias ou lista de sequências/grupos. */
export type View = "grid" | "sequences" | "people" | "develop" | "consistency" | "album";

interface State {
  project: ProjectInfo | null;
  photos: PhotoRow[];
  index: Map<number, number>;
  filter: Filter;
  thumbSize: number;
  infoOpen: boolean;
  selection: Selection;
  viewerId: number | null;
  details: Map<number, PhotoDetails>;
  importProgress: ImportProgress | null;
  importRunning: boolean;
  importSummary: ImportSummary | null;
  thumbProgress: { done: number; total: number; remaining: number } | null;
  /** Incrementado quando uma miniatura muda, para forçar o recarregamento da imagem. */
  thumbVersion: Map<number, number>;
  pendingImport: { paths: string[]; hasFolders: boolean } | null;
  helpOpen: boolean;
  toasts: Toast[];
  view: View;
  // Seleção assistida
  analysis: AnalysisStatus | null;
  analysisProgress: AnalysisProgress | null;
  recs: RecMap;
  groups: GroupRow[];
  groupById: Map<number, GroupRow>;
  analysisDetails: Map<number, AnalysisDetails>;
    /** Fotografias na comparação lado a lado (2–4), ou null. */
  compare: number[] | null;
  // Organização (Fase 4)
  persons: PersonRow[];
  personById: Map<number, PersonRow>;
  photoPersons: Map<number, number[]>;
  moments: MomentRow[];
  assetMoments: Map<number, AssetMoment>;
  /** Momento efetivo de cada fotografia (null = por classificar). */
  photoMoment: Map<number, number | null>;
  /** Fotografias a ser arrastadas (para um momento), ou null. */
    dragIds: number[] | null;
  clockOpen: boolean;
  /** Definições copiadas (Copiar/Colar). */
  clipboard: { settings: DevelopSettings; blocks: string[] } | null;
  // Edição automática (Fase 6)
  /** Últimas opções usadas (editor e "Editar automaticamente seleção"). */
  autoOptions: AutoOptions;
  /** Estado da fila em segundo plano (null = sem fila visível). */
  autoProgress: AutoEditProgress | null;
  autoDialog: boolean;
  // Presets (Fase 7)
  presets: Preset[];
  /** Diálogo "Aplicar preset" (seleção, momento, casamento). */
  presetDialog: boolean;
  // Exportação (Fase 9)
  exportDialog: boolean;
  galleryDialog: boolean;
  /** Trabalhos da fila de exportação (atualizados pelos eventos). */
  exportJobs: ExportProgress[];
}

interface Actions {
  setProject(p: ProjectInfo | null): void;
  setPhotos(rows: PhotoRow[]): void;
  reloadPhotos(): Promise<void>;
  setFilter(f: Partial<Filter>): void;
  setThumbSize(n: number): void;
  toggleInfo(): void;
  setSelection(s: Selection): void;
  openViewer(id: number | null): void;
  rate(kind: "stars" | "flag" | "color", value: number | Color | null, ids: number[]): Promise<void>;
  applyThumbBatch(b: ThumbBatch): void;
  loadDetails(id: number): Promise<void>;
  setImportProgress(p: ImportProgress | null): void;
  setImportRunning(r: boolean): void;
  setImportSummary(s: ImportSummary | null): void;
  setPendingImport(p: State["pendingImport"]): void;
  setHelpOpen(o: boolean): void;
  toast(kind: Toast["kind"], text: string): void;
  dismissToast(id: number): void;
  uiState(): UiState;
  setView(v: View): void;
  reloadSelection(): Promise<void>;
  setAnalysisProgress(p: AnalysisProgress | null): void;
  loadAnalysisDetails(id: number): Promise<void>;
    openCompare(ids: number[] | null): void;
  reloadOrganization(): Promise<void>;
  setDrag(ids: number[] | null): void;
    setClockOpen(o: boolean): void;
  setClipboard(c: State["clipboard"]): void;
  setAutoOptions(o: AutoOptions): void;
  setAutoProgress(p: AutoEditProgress | null): void;
  setAutoDialog(o: boolean): void;
  reloadPresets(): Promise<void>;
  setPresetDialog(o: boolean): void;
  setExportDialog(o: boolean): void;
  setGalleryDialog(o: boolean): void;
  /** Atualiza (ou junta) um trabalho da fila de exportação. */
  upsertExportJob(p: ExportProgress): void;
  setExportJobs(j: ExportProgress[]): void;
  /** Miniaturas/previews editadas mudaram: novas URLs e recarregar a lista. */
  bumpEdited(keys: number[]): void;
}

let toastSeq = 1;

const AUTO_OPTIONS_KEY = "momento.autoOptions";

/** Opções da edição automática lembradas entre sessões (preferência deste computador). */
function loadAutoOptions(): AutoOptions {
  try {
    const raw = localStorage.getItem(AUTO_OPTIONS_KEY);
    if (raw) return { ...defaultAutoOptions(), ...(JSON.parse(raw) as Partial<AutoOptions>) };
  } catch {
    /* ignorado */
  }
  return defaultAutoOptions();
}

function buildIndex(rows: PhotoRow[]): Map<number, number> {
  const m = new Map<number, number>();
  rows.forEach((r, i) => m.set(r.id, i));
  return m;
}

export function parseUiState(json: string | null): Partial<UiState> {
  if (!json) return {};
  try {
    const v = JSON.parse(json) as Partial<UiState>;
    const out: Partial<UiState> = {};
    if (v.filter && typeof v.filter === "object") out.filter = { ...DEFAULT_FILTER, ...v.filter };
    if (typeof v.thumbSize === "number") out.thumbSize = Math.min(THUMB_MAX, Math.max(THUMB_MIN, v.thumbSize));
    if (typeof v.focusId === "number") out.focusId = v.focusId;
    if (typeof v.infoOpen === "boolean") out.infoOpen = v.infoOpen;
        if (v.view === "grid" || v.view === "sequences" || v.view === "people") out.view = v.view;
    return out;
  } catch {
    return {};
  }
}

export const useStore = create<State & Actions>((set, get) => ({
  project: null,
  photos: [],
  index: new Map(),
  filter: DEFAULT_FILTER,
  thumbSize: THUMB_DEFAULT,
  infoOpen: true,
  selection: EMPTY_SELECTION,
  viewerId: null,
  details: new Map(),
  importProgress: null,
  importRunning: false,
  importSummary: null,
  thumbProgress: null,
  thumbVersion: new Map(),
  pendingImport: null,
  helpOpen: false,
  toasts: [],
  view: "grid",
  analysis: null,
  analysisProgress: null,
  recs: new Map(),
  groups: [],
  groupById: new Map(),
    analysisDetails: new Map(),
  compare: null,
  persons: [],
  personById: new Map(),
  photoPersons: new Map(),
  moments: [],
  assetMoments: new Map(),
  photoMoment: new Map(),
    dragIds: null,
  clockOpen: false,
  clipboard: null,
  autoOptions: loadAutoOptions(),
  autoProgress: null,
  autoDialog: false,
  presets: [],
  presetDialog: false,
  exportDialog: false,
  galleryDialog: false,
  exportJobs: [],



  setProject(p) {
    if (!p) {
      set({
        project: null,
        photos: [],
        index: new Map(),
        selection: EMPTY_SELECTION,
        viewerId: null,
        details: new Map(),
        importProgress: null,
        importRunning: false,
        importSummary: null,
        thumbProgress: null,
        thumbVersion: new Map(),
        view: "grid",
        analysis: null,
        analysisProgress: null,
        recs: new Map(),
        groups: [],
        groupById: new Map(),
                analysisDetails: new Map(),
        compare: null,
        persons: [],
        personById: new Map(),
        photoPersons: new Map(),
        moments: [],
        assetMoments: new Map(),
        photoMoment: new Map(),
        dragIds: null,
        clockOpen: false,
        autoProgress: null,
        autoDialog: false,
        exportDialog: false,
        galleryDialog: false,
        exportJobs: [],
      });
      return;
    }
    const ui = parseUiState(p.uiState);
    set({
      project: p,
      filter: ui.filter ?? DEFAULT_FILTER,
      thumbSize: ui.thumbSize ?? THUMB_DEFAULT,
      infoOpen: ui.infoOpen ?? true,
      selection: ui.focusId != null ? { ids: new Set([ui.focusId]), focus: ui.focusId, anchor: ui.focusId } : EMPTY_SELECTION,
      importRunning: p.importRunning,
      viewerId: null,
      details: new Map(),
      view: ui.view ?? "grid",
      compare: null,
    });
  },

  setPhotos(rows) {
    set({ photos: rows, index: buildIndex(rows) });
  },

  async reloadPhotos() {
    const t0 = performance.now();
    const first = get().photos.length === 0;
    const rows = await api.listPhotos();
    const t1 = performance.now();
    get().setPhotos(rows);
    perfMark("lista_fotografias", { fotografias: rows.length, ipc_ms: Math.round(t1 - t0), estado_ms: Math.round(performance.now() - t1) });
    if (first) perfWatchGrid(t0, rows.length);
  },

  setFilter(f) {
    set({ filter: { ...get().filter, ...f } });
  },

  setThumbSize(n) {
    set({ thumbSize: Math.min(THUMB_MAX, Math.max(THUMB_MIN, Math.round(n))) });
  },

  toggleInfo() {
    set({ infoOpen: !get().infoOpen });
  },

  setSelection(s) {
    set({ selection: s });
  },

  openViewer(id) {
    set({ viewerId: id });
    if (id !== null) {
      const sel = get().selection;
      if (sel.focus !== id) set({ selection: { ids: new Set([id]), focus: id, anchor: id } });
    }
  },

  async rate(kind, value, ids) {
    if (ids.length === 0) return;
    const { photos, index } = get();
    const before = ids.map((id) => photos[index.get(id)!]).filter((p): p is PhotoRow => !!p);
    const patch = (p: PhotoRow): PhotoRow =>
      kind === "stars" ? { ...p, stars: value as number } : kind === "flag" ? { ...p, flag: value as number } : { ...p, color: value as Color | null };
    // Atualização otimista; reverte se o núcleo recusar.
    const next = photos.slice();
    for (const p of before) next[index.get(p.id)!] = patch(p);
    set({ photos: next });
    try {
      if (kind === "stars") await api.setStars(ids, value as number);
      else if (kind === "flag") await api.setFlag(ids, value as number);
      else await api.setColor(ids, value as Color | null);
    } catch (e) {
      const cur = get().photos.slice();
      for (const p of before) cur[get().index.get(p.id)!] = p;
      set({ photos: cur });
      throw e;
    }
  },

  applyThumbBatch(b) {
    const { photos, index, thumbVersion } = get();
    const next = photos.slice();
    const versions = new Map(thumbVersion);
    let changed = false;
    for (const [ids, state] of [
      [b.ready, "ready"],
      [b.failed, "failed"],
    ] as const) {
      for (const id of ids) {
        const i = index.get(id);
        if (i === undefined) continue;
        next[i] = { ...next[i]!, thumb: state };
        versions.set(id, (versions.get(id) ?? 0) + 1);
        changed = true;
      }
    }
    set({
      ...(changed ? { photos: next, thumbVersion: versions } : {}),
      thumbProgress: b.remaining > 0 ? { done: b.done, total: b.total, remaining: b.remaining } : null,
    });
  },

  async loadDetails(id) {
    if (get().details.has(id)) return;
    const d = await api.photoDetails(id);
    if (d) {
      const m = new Map(get().details);
      m.set(id, d);
      set({ details: m });
    }
  },

  setImportProgress(p) {
    set({ importProgress: p });
  },
  setImportRunning(r) {
    set({ importRunning: r });
  },
  setImportSummary(s) {
    set({ importSummary: s });
  },
  setPendingImport(p) {
    set({ pendingImport: p });
  },
  setHelpOpen(o) {
    set({ helpOpen: o });
  },

  toast(kind, text) {
    const id = toastSeq++;
    set({ toasts: [...get().toasts, { id, kind, text }] });
    setTimeout(() => get().dismissToast(id), kind === "error" ? 8000 : 4000);
  },
  dismissToast(id) {
    set({ toasts: get().toasts.filter((t) => t.id !== id) });
  },

  uiState() {
    const s = get();
    return { filter: s.filter, thumbSize: s.thumbSize, focusId: s.selection.focus, infoOpen: s.infoOpen, view: s.view };
  },

  setView(v) {
    set({ view: v });
  },

  async reloadSelection() {
    const [analysis, recs, groups] = await Promise.all([api.analysisStatus(), api.listRecommendations(), api.listGroups()]);
    set({
      analysis,
      recs: new Map(recs.map((r) => [r.asset, r])),
      groups,
      groupById: new Map(groups.map((g) => [g.id, g])),
      analysisDetails: new Map(),
    });
  },

  setAnalysisProgress(p) {
    set({ analysisProgress: p });
  },

  async loadAnalysisDetails(id) {
    if (get().analysisDetails.has(id)) return;
    const d = await api.analysisDetails(id);
    const m = new Map(get().analysisDetails);
    m.set(id, d);
    set({ analysisDetails: m });
  },

    openCompare(ids) {
    set({ compare: ids && ids.length >= 1 ? ids.slice(0, 4) : null });
  },

  async reloadOrganization() {
    const [persons, pairs, moments, am] = await Promise.all([api.listPersons(), api.photoPersons(), api.listMoments(), api.assetMoments()]);
    const photoPersons = new Map<number, number[]>();
    for (const [a, p] of pairs) {
      const l = photoPersons.get(a);
      if (l) l.push(p);
      else photoPersons.set(a, [p]);
    }
    const assetMoments = new Map(am.map((m) => [m.asset, m]));
    const photoMoment = new Map<number, number | null>(am.map((m) => [m.asset, m.momentId]));
    set({ persons, personById: new Map(persons.map((p) => [p.id, p])), photoPersons, moments, assetMoments, photoMoment });
  },

  setDrag(ids) {
    set({ dragIds: ids && ids.length > 0 ? ids : null });
  },

    setClockOpen(o) {
    set({ clockOpen: o });
  },

  setClipboard(c) {
    set({ clipboard: c });
  },

  setAutoOptions(o) {
    set({ autoOptions: o });
    try {
      localStorage.setItem(AUTO_OPTIONS_KEY, JSON.stringify(o));
    } catch {
      /* armazenamento indisponível */
    }
  },

  setAutoProgress(p) {
    set({ autoProgress: p });
  },

  setAutoDialog(o) {
    set({ autoDialog: o });
  },

  async reloadPresets() {
    set({ presets: await api.listPresets() });
  },

  setPresetDialog(o) {
    set({ presetDialog: o });
  },

  setExportDialog(o) {
    set({ exportDialog: o });
  },

  setGalleryDialog(o) {
    set({ galleryDialog: o });
  },

  upsertExportJob(p) {
    const jobs = get().exportJobs;
    const i = jobs.findIndex((j) => j.id === p.id);
    set({ exportJobs: i < 0 ? [...jobs, p] : jobs.map((j) => (j.id === p.id ? p : j)) });
  },

  setExportJobs(j) {
    set({ exportJobs: j });
  },

  bumpEdited(keys) {
    const versions = new Map(get().thumbVersion);
    for (const k of keys) versions.set(k, (versions.get(k) ?? 0) + 1);
    set({ thumbVersion: versions });
  },
}));

/** Converte um erro do núcleo (`ErrorPayload`) ou do JavaScript num texto. */
export function errorText(e: unknown, t: (k: string) => string): string {
  if (e && typeof e === "object" && "code" in e) {
    const p = e as ErrorPayload;
    const base = t(p.code);
    return p.detail ? `${base} ${p.detail}` : base;
  }
  return e instanceof Error ? e.message : String(e);
}
