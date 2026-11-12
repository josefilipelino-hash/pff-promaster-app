import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import "../i18n";
import axe from "axe-core";
import { examplePath, fitToFormat, formatDuration, newRecipe, sanitize } from "../logic/exportRecipe";
import type { DevelopSettings, PhotoRow, ProjectInfo } from "../types";

// --- Simulação do núcleo e das APIs Tauri ---------------------------------------------
const capsOf = (format: string, extra: Record<string, unknown> = {}) => ({
  format,
  available: true,
  reason: null,
  extension: format,
  depths: ["8"],
  compressions: [],
  quality: false,
  spaces: "icc",
  metadata: true,
  resolution: true,
  linear: false,
  ...extra,
});
const EXPORT_CAPS = [
  capsOf("jpeg", { quality: true, extension: "jpg" }),
  capsOf("png", { depths: ["8", "16"], compressions: ["fast", "best"] }),
  capsOf("tiff", { depths: ["8", "16", "32f"], compressions: ["none", "lzw", "zip"], extension: "tif" }),
  capsOf("avif", { depths: ["8", "10", "12"], quality: true, spaces: "cicp" }),
  capsOf("heic", { available: false, reason: "export.unavailable.hevc", depths: [] }),
];
const EXPORT_BASE = {
  id: "builtin:galeria",
  name: "Galeria",
  format: "jpeg",
  depth: "8",
  quality: 85,
  compression: "lossy",
  color: "srgb",
  iccPath: "",
  intent: "perceptual",
  size: { mode: "longEdge", unit: "px", a: 2560, b: 2560, noEnlarge: true, ppi: 300, matchOrientation: false },
  sharpen: { target: "screen", amount: "standard" },
  metadata: { mode: "all", removeGps: true, copyright: "", creator: "", contact: "" },
  watermark: { enabled: false, kind: "text", text: "© ", imagePath: "", fontPath: "", color: "#ffffff", size: 4, opacity: 60, anchor: "bottomRight", margin: 3 },
  naming: { template: "{nome}", prefix: "", suffix: "", seqStart: 1, seqDigits: 4, subfolder: "Galeria", conflict: "rename" },
};
const EXPORT_JPEG = EXPORT_BASE;
const GALLERY_SETTINGS = {
  couple: "Ana & Pedro", date: "2026-06-15", location: "Sintra", intro: "", cover: null, hero: null, logoPath: "",
  theme: "light", layout: "masonry", language: "pt", selection: "picks", minStars: 3, chapters: "grouped", hidden: [] as number[],
  favorites: true, downloads: "all", downloadSize: "high", chapterZips: false, protection: "none", secret: "",
  photographer: "", contactEmail: "", slideshowSeconds: 4,
};
const EXPORT_TIFF = { ...EXPORT_BASE, id: "builtin:arquivo-tiff", name: "Arquivo TIFF", format: "tiff", depth: "16", compression: "zip", color: "adobeRgb", size: { ...EXPORT_BASE.size, mode: "original" }, naming: { ...EXPORT_BASE.naming, subfolder: "Arquivo" } };

const apiMock = {
  available: vi.fn(() => true),
  appInfo: vi.fn(async () => ({ appVersion: "0.1.0", coreVersion: "0.1.0", catalogSchemaVersion: 2, os: "macos", arch: "aarch64" })),
  eventTypes: vi.fn(async () => ["wedding", "baptism"]),
  recentProjects: vi.fn(async () => [] as unknown[]),
  forgetRecent: vi.fn(async () => {}),
  createProject: vi.fn(),
  openProject: vi.fn(),
  closeProject: vi.fn(async () => {}),
  currentProject: vi.fn(async () => null),
  listPhotos: vi.fn(async () => [] as PhotoRow[]),
  photoDetails: vi.fn(async () => null),
  setStars: vi.fn(async () => 1),
  setFlag: vi.fn(async () => 1),
  setColor: vi.fn(async () => 1),
  startImport: vi.fn(async () => 1),
  safetyStatus: vi.fn(async () => safetyFixture()),
  createBackup: vi.fn(async () => ({ file: "/p/backups/catalog-manual-2.db", created: 1_794_300_000, bytes: 4096, kind: "manual" })),
  setBackupMinutes: vi.fn(async () => {}),
  validateCatalog: vi.fn(async () => ({ integrity: [], foreignKeys: 0 })),
  exportCatalog: vi.fn(async () => {}),
  restoreBackup: vi.fn(async () => ({ ...project, imageToken: 99 })),
  importCatalog: vi.fn(async () => ({ ...project, imageToken: 98 })),
  checkAvailability: vi.fn(async () => ({ ok: 4, offline: 1, missing: 0, changed: 0, reconnected: 0 })),
  removeFromCatalog: vi.fn(async (keys: number[]) => ({ removed: keys.length, trashed: 0, failed: [] })),
  moveToTrash: vi.fn(async (keys: number[]) => ({ removed: keys.length, trashed: keys.length, failed: [] })),
  revealOriginal: vi.fn(async () => {}),
  relinkFile: vi.fn(async () => ({ relinked: 3, stillUnavailable: 0 })),
  relinkFolder: vi.fn(async () => ({ relinked: 2, stillUnavailable: 0 })),
  resumeExport: vi.fn(async () => 31),
  discardExport: vi.fn(async () => {}),
  cancelImport: vi.fn(async () => {}),
  prioritizeThumbnails: vi.fn(async () => {}),
  saveUiState: vi.fn(async () => {}),
  startAnalysis: vi.fn(async () => 10),
  cancelAnalysis: vi.fn(async () => {}),
  analysisStatus: vi.fn(async () => analysisStatus(0, 10)),
  setSelectionSettings: vi.fn(async () => summary),
  recomputeSelection: vi.fn(async () => summary),
  listRecommendations: vi.fn(async () => [] as unknown[]),
  listGroups: vi.fn(async () => [] as unknown[]),
  analysisDetails: vi.fn(async () => ({ analysis: null, faces: [], recommendation: null, groupId: null, error: null }) as unknown),
  acceptRecommendations: vi.fn(async () => 1),
    acceptGroup: vi.fn(async () => 2),
  rebuildOrganization: vi.fn(async () => ({})),
  listPersons: vi.fn(async () => [] as unknown[]),
  personFaces: vi.fn(async () => [] as unknown[]),
  photoPersons: vi.fn(async () => [] as [number, number][]),
  updatePerson: vi.fn(async () => {}),
  mergePersons: vi.fn(async () => {}),
  excludeFaces: vi.fn(async () => 1),
  reclusterPeople: vi.fn(async () => 2),
  clearPeople: vi.fn(async () => {}),
  listMoments: vi.fn(async () => [] as unknown[]),
  assetMoments: vi.fn(async () => [] as unknown[]),
  createMoment: vi.fn(async () => 99),
  renameMoment: vi.fn(async () => {}),
  deleteMoment: vi.fn(async () => {}),
  assignMoment: vi.fn(async () => 2),
  resetMoments: vi.fn(async () => 1),
  reclassifyMoments: vi.fn(async () => [0, 0]),
  listCameras: vi.fn(async () => [] as unknown[]),
  suggestOffsets: vi.fn(async () => [] as unknown[]),
    setCameraOffsets: vi.fn(async () => ({})),
  editView: vi.fn(async (key: number) => editViewOf(key)),
  renderEdit: vi.fn(async (..._args: unknown[]) => frameBuffer(40, 30)),
  commitEdit: vi.fn(async (key: number, _label: string, settings: unknown) => ({ ...editViewOf(key), settings, position: 1, canUndo: true, history: [{ seq: 0, label: "original", createdAt: 0 }, { seq: 1, label: "light.exposure", createdAt: 1 }] })),
  undoEdit: vi.fn(async (key: number) => editViewOf(key)),
  redoEdit: vi.fn(async (key: number) => editViewOf(key)),
  gotoHistory: vi.fn(async (key: number) => editViewOf(key)),
  resetEdit: vi.fn(async (key: number) => editViewOf(key)),
  createSnapshot: vi.fn(async (key: number) => ({ ...editViewOf(key), snapshots: [{ id: 5, name: "Quente", createdAt: 1 }] })),
  applySnapshot: vi.fn(async (key: number) => editViewOf(key)),
  deleteSnapshot: vi.fn(async (key: number) => editViewOf(key)),
  createVirtualCopy: vi.fn(async () => 2 ** 40 + 77),
  deleteVirtualCopy: vi.fn(async () => {}),
  pasteSettings: vi.fn(async (keys: number[]) => keys.length),
  upright: vi.fn(async () => ({ vertical: 31.23, horizontal: 0, straighten: -1.249, focal35: 24, mode: "vertical", verticalLines: 12, horizontalLines: 0, deviationBefore: 4.2, deviationAfter: 0.2 })),
  pickWhiteBalance: vi.fn(async () => [23.4, -7.6] as [number, number]),
  autoStraighten: vi.fn(async () => 2.34),
  asShotKelvin: vi.fn(async () => null as number | null),
  exportLightroomRatings: vi.fn(async () => ({ created: 2, updated: 0, byStars: [0, 1, 1, 0], notRaw: ["IMG_9.jpg"], unavailable: [], failed: [], starsInApp: 2 })),
  aiDenoiseAvailable: vi.fn(async () => true),
  exportCapabilities: vi.fn(async () => EXPORT_CAPS),
  exportRecipes: vi.fn(async () => [EXPORT_JPEG, EXPORT_TIFF]),
  exportPlans: vi.fn(async () => [{ id: "builtin:casamento", name: "Exportar casamento", recipes: [EXPORT_JPEG, EXPORT_TIFF] }]),
  saveExportRecipe: vi.fn(async (r: unknown) => ({ ...(r as object), id: "x1" })),
  saveExportPlan: vi.fn(async (p: unknown) => ({ ...(p as object), id: "x2" })),
  deleteExportPreset: vi.fn(async () => true),
  inspectIccProfile: vi.fn(async () => "Lab Glossy"),
  estimateExport: vi.fn(async () => ({ photos: 3, files: 6, bytes: 120e6, freeBytes: 10e9, enoughSpace: true, perRecipe: [], notes: [], errors: [] })),
  startExport: vi.fn(async () => 1),
  exportJobs: vi.fn(async () => [] as unknown[]),
  pauseExport: vi.fn(async () => true),
  cancelExport: vi.fn(async () => true),
  clearFinishedExports: vi.fn(async () => {}),
  revealFolder: vi.fn(async () => {}),
  galleryConfig: vi.fn(async () => ({ settings: { ...GALLERY_SETTINGS }, watermark: { ...EXPORT_BASE.watermark }, destination: "/tmp/pub" })),
  saveGalleryConfig: vi.fn(async () => {}),
  galleryPreview: vi.fn(async () => ({ photos: 4, hidden: 0, chapters: [{ title: "Preparação", count: 2 }, { title: "Cerimónia", count: 2 }], folder: "/tmp/pub/ana-e-pedro-galeria", exists: true, blocked: false, problems: [] as string[], estimateBytes: 50e6 })),
  startGallery: vi.fn(async () => 9),
  setGalleryHidden: vi.fn(async (keys: number[], hidden: boolean) => ({ settings: { ...GALLERY_SETTINGS, hidden: hidden ? keys : [] } })),
  importClientFavorites: vi.fn(async () => ({ matched: 2, unmatched: ["x"] })),
  openGallery: vi.fn(async () => {}),
  autoEditPhoto: vi.fn(),
  startAutoEdit: vi.fn(async (keys: number[]) => keys.length),
  autoEditStatus: vi.fn(),
  pauseAutoEdit: vi.fn(async () => {}),
  resumeAutoEdit: vi.fn(async () => {}),
  cancelAutoEdit: vi.fn(async () => {}),
  listPresets: vi.fn(async () => presetList()),
  createPreset: vi.fn(async (name: string, ..._rest: unknown[]) => ({ ...presetList()[0]!, id: "u1", name, builtin: false, group: "" })),
  renamePreset: vi.fn(async () => ({})),
  movePreset: vi.fn(async () => ({})),
  reorderPresets: vi.fn(async () => {}),
  duplicatePreset: vi.fn(async () => ({})),
  deletePreset: vi.fn(async () => {}),
  importPresetFile: vi.fn(),
  exportPresets: vi.fn(async (ids: string[]) => ids.length),
  presetPreviewSettings: vi.fn(async (settings: unknown, _id: string, amount: number) => ({ ...(settings as object), light: { ...(settings as { light: object }).light, contrast: amount / 10 } })),
  applyPreset: vi.fn(async (keys: number[]) => keys.length),
  measureConsistency: vi.fn(async () => 4),
  consistencyGroups: vi.fn(async () => ({ groups: lookGroups(), missing: 0 })),
  startSync: vi.fn(async (_r: number, keys: number[]) => keys.length),
  createMaskRaster: vi.fn(async (_k: number, kind: string) => ({ raster: `r-${kind}`, kind, coverage: 0.4 })),
  detectPortrait: vi.fn(async () => [faceRegions()]),
  mapPoints: vi.fn(async (_k: number, _s: unknown, pts: [number, number][]) => pts),
  suggestHealSource: vi.fn(async (_k: number, x: number, y: number) => [x + 0.1, y] as [number, number]),
  albumFormats: vi.fn(async () => [{ name: "20x20", widthCm: 20, heightCm: 20 }, { name: "30x30", widthCm: 30, heightCm: 30 }, { name: "30x40", widthCm: 40, heightCm: 30 }]),
  albumTemplates: vi.fn(async (n: number) => (n <= 1 ? ["P", "-|B", "B|-"] : ["B|B", "F|F", "-|G2"])),
  listAlbums: vi.fn(async () => [{ id: "a1", name: "Álbum Ana & Pedro", format: "30x30", pages: 4, updatedAt: "" }]),
  getAlbum: vi.fn(async () => albumFixture()),
  createAlbum: vi.fn(async (name: string, spec: unknown) => ({ ...albumFixture(), id: "a2", name, spec })),
  saveAlbum: vi.fn(async () => {}),
  deleteAlbum: vi.fn(async () => true),
  autoAlbum: vi.fn(async () => ({ album: { ...albumFixture(), notes: ["abertura:1", "cerimonia:1"] }, used: 3, candidates: 4, skippedLowRes: 1, skippedDuplicates: 0 })),
  applyAlbumLayout: vi.fn(async (album: { spreads: { template: string }[] }, idx: number, tpl: string) => ({ ...album, spreads: album.spreads.map((s, i) => (i === idx ? { ...s, template: tpl } : s)) })),
  albumPreflight: vi.fn(async () => [
    { spread: null, slot: null, severity: "warning", code: "album.issue.unconfirmed", value: null },
    { spread: 0, slot: "s1", severity: "error", code: "album.issue.lowDpi", value: 120 },
  ]),
  albumPhotoInfos: vi.fn(async () => [1, 2, 3, 4].map((key) => ({ key, width: 6000, height: 4000, time: null, moment: null, flag: 1, stars: 4, score: 0.8, faces: key === 2 ? [[0.4, 0.2, 0.1, 0.15]] : [], persons: [], group: null }))),
  startAlbumExport: vi.fn(async () => 21),
  labPresets: vi.fn(async () => [{ id: "l1", name: "Lab Porto", notes: "", dpi: 300, bleedMm: 5, safeMm: 15, gutterMm: 20, color: null, iccPath: "", intent: "perceptual", exportUnit: "pages", minPages: 20, maxPages: 80 }]),
  saveLabPreset: vi.fn(async (p: object) => ({ ...p, id: "l2" })),
  deleteLabPreset: vi.fn(async () => true),
};
function safetyFixture() {
  return {
    report: { previousUnclean: false, validatedFull: false, validationMs: 5, restoredFrom: null, damagedKept: null, tempFilesRemoved: 0 },
    availability: { ok: 4, offline: 1, missing: 0, changed: 0, reconnected: 0 },
    backups: [
      { file: "/p/backups/catalog-1794290000.db", created: 1_794_290_000, bytes: 8192, kind: "auto" },
      { file: "/p/backups/catalog-danificado-1794280000.db", created: 1_794_280_000, bytes: 8192, kind: "danificado" },
    ],
    backupMinutes: 60,
    events: [[1_794_290_000, "sessao_interrompida", "validação completa em 40 ms"]] as [number, string, string][],
    pendingExports: [] as { id: string; name: string; destination: string; done: number; total: number }[],
  };
}
function albumFixture() {
  const spec = { format: "30x30", widthMm: 300, heightMm: 300, pages: 4, dpi: 300, bleedMm: 3, safeMm: 10, gutterMm: 16, color: "srgb", iccPath: "", intent: "perceptual", lab: "", confirmed: false, exportUnit: "spreads", minPages: 0, maxPages: 0, background: "#ffffff" };
  const crop = { zoom: 1, cx: 0.5, cy: 0.5 };
  return {
    id: "a1",
    name: "Álbum Ana & Pedro",
    spec,
    notes: [] as string[],
    spreads: [
      { id: "sp1", template: "B|F", marginMm: 12, spacingMm: 4, slots: [{ id: "s1", x: -3, y: -3, w: 303, h: 306, photo: 1, crop }, { id: "s2", x: 330, y: 60, w: 240, h: 160, photo: null, crop }] },
      { id: "sp2", template: "", marginMm: 12, spacingMm: 4, slots: [] as unknown[] },
    ],
  };
}
function faceRegions() {
  const sq = (x: number, y: number, d: number): [number, number][] => [[x, y], [x + d, y], [x + d, y + d], [x, y + d]];
  return { oval: sq(0.4, 0.2, 0.2), leftEye: sq(0.45, 0.25, 0.02), rightEye: sq(0.52, 0.25, 0.02), leftBrow: [], rightBrow: [], lipsOuter: sq(0.47, 0.33, 0.06), lipsInner: sq(0.48, 0.34, 0.04), underLeft: sq(0.45, 0.28, 0.02), underRight: sq(0.52, 0.28, 0.02), leftIris: [0.46, 0.26, 0.005], rightIris: [0.53, 0.26, 0.005] };
}
function presetList() {
  return [
    { id: "builtin:natural-wedding", name: "Natural Wedding", group: "Cor", components: ["tone", "presence"], settings: defaultSettings(), builtin: true, description: "Natural" },
    { id: "builtin:bw-classic", name: "B&W Classic", group: "Preto e branco", components: ["tone", "blackWhite"], settings: defaultSettings(), builtin: true, description: "P&B" },
    { id: "u7", name: "Meu look", group: "Clientes", components: ["tone"], settings: defaultSettings(), builtin: false, description: "" },
    { id: "u8", name: "Outro", group: "Clientes", components: ["tone"], settings: defaultSettings(), builtin: false, description: "" },
  ];
}
function lookGroups() {
  return [
    { id: "combined:0", code: "lighting.bright", tags: ["flash.off", "wb.neutral"], detail: "", moment: null, start: "2026-06-15T15:00:00", end: "2026-06-15T15:20:00", ids: [1, 2] },
    { id: "combined:1", code: "lighting.dark", tags: ["flash.on", "wb.warm"], detail: "", moment: null, start: "2026-06-15T22:00:00", end: "2026-06-15T23:10:00", ids: [3, 4] },
  ];
}
/** Frame binário como o do núcleo (dimensões, histograma, RGBA cinzento). */
function frameBuffer(w: number, h: number): ArrayBuffer {
  const buf = new ArrayBuffer(8 + 3 * 256 * 4 + w * h * 4);
  const dv = new DataView(buf);
  dv.setUint32(0, w, true);
  dv.setUint32(4, h, true);
  new Uint32Array(buf, 8, 768)[128] = w * h;
  new Uint8Array(buf, 8 + 3072).fill(128);
  return buf;
}
const summary = { best: 1, alternative: 1, review: 0, reject: 1, duplicate: 0, groups: 1, sequences: 1, durationMs: 1 };
function analysisStatus(analyzed: number, total: number) {
  return {
    running: false,
    counts: { total, analyzed, failed: 0, pending: total - analyzed },
    modelsAvailable: true,
    settings: { mode: "balanced", target: null },
    summary,
  };
}
vi.mock("../api", async (orig) => {
  const real = await orig<typeof import("../api")>();
  const noop = async () => () => {};
  return { ...real, api: apiMock, events: { importProgress: noop, importFinished: noop, thumbnails: noop, analysis: noop, editsChanged: noop, autoEdit: noop, aiDenoise: noop, exportProgress: noop, availability: noop } };
});
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: async () => () => {} }) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ isFullscreen: async () => false, setFullscreen: vi.fn(async () => {}) }) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));

beforeAll(() => {
  globalThis.ResizeObserver = class {
    observe() {}
    disconnect() {}
    unobserve() {}
  } as unknown as typeof ResizeObserver;
});

const { App } = await import("../App");
const { useStore } = await import("../store");
const { folderPreview } = await import("./NewProjectDialog");
const { computeLayout, visibleRows } = await import("./PhotoGrid");
const { fitScale, stepZoom } = await import("./Viewer");
const { clampCenter, fitCss, zoomAt, FIT_VIEW } = await import("./CompareView");
const { explain, joinList } = await import("../logic/explain");
const { parseOffset, formatOffset } = await import("../logic/clock");
const { faceCrop } = await import("./FaceThumb");
const { defaultSettings, parseFrame, fitCropToAspect } = await import("../logic/develop");
const { beforeForSplit } = await import("./develop/DevelopView");
const { historyLabel } = await import("./develop/DevelopView");
function editViewOf(key: number) {
  return { key, asset: key, variant: key, position: 0, history: [], canUndo: false, canRedo: false, snapshots: [], settings: defaultSettings() };
}
const i18n = (await import("../i18n")).default;

const project: ProjectInfo = {
  path: "/trabalho/2026-06-15_Ana_e_Pedro.momento",
  name: "Casamento Ana & Pedro",
  clientName: "Ana & Pedro",
  eventType: "wedding",
  eventDate: "2026-06-15",
  location: "Sintra",
  originalsDir: null,
  status: "imported",
  notes: "",
  createdAt: 0,
  updatedAt: 0,
  photoCount: 3000,
  uiState: null,
  importRunning: false,
  imageToken: 1,
};

const rows = (n: number): PhotoRow[] =>
    Array.from({ length: n }, (_, i) => ({
    id: i + 1,
    asset: i + 1,
    variant: i + 1,
    virtualCopy: null,
    edited: false,
    fileName: `IMG_${String(i + 1).padStart(5, "0")}.jpg`,
    captureTime: null,
    width: 6000,
    height: 4000,
    stars: 0,
    flag: i === 1 ? 1 : 0,
    color: null,
    thumb: "ready" as const,
    partial: i === 2,
    format: i === 2 ? "cr3" : "jpeg",
  }));

beforeEach(() => {
  vi.clearAllMocks();
  useStore.getState().setProject(null);
});
afterEach(cleanup);

describe("ecrã inicial", () => {
  it("mostra ações e projetos recentes em pt-PT", async () => {
    apiMock.recentProjects.mockResolvedValueOnce([
      { path: "/p.momento", name: "Batizado Maria", clientName: "Família Silva", eventType: "baptism", eventDate: "2026-03-01", location: "Porto", photoCount: 812, lastOpened: Date.now(), available: true },
      { path: "/q.momento", name: "Concerto", clientName: "", eventType: "concert", eventDate: null, location: "", photoCount: 0, lastOpened: Date.now(), available: false },
    ]);
    render(<App />);
    expect(await screen.findByText("Batizado Maria")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Novo projeto" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Abrir projeto…" })).toBeTruthy();
    expect(screen.getByText(/812 fotografias/)).toBeTruthy();
    expect(screen.getByText(/Pasta não encontrada/)).toBeTruthy();
  });

  it("formulário de novo projeto valida campos obrigatórios", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Novo projeto" }));
    const dialog = screen.getByRole("dialog", { name: "Novo projeto" });
    const create = within(dialog).getByRole("button", { name: "Criar projeto" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    fireEvent.change(within(dialog).getByPlaceholderText("Ex.: Casamento Ana & Pedro"), { target: { value: "Casamento" } });
    fireEvent.change(within(dialog).getByPlaceholderText("Pasta onde o projeto será criado"), { target: { value: "/trabalho" } });
    expect(create.disabled).toBe(false);
    expect(within(dialog).getByText(/\/trabalho\/Casamento\.momento/)).toBeTruthy();
  });

  it("fora do Tauri mostra aviso", () => {
    apiMock.available.mockReturnValueOnce(false);
    render(<App />);
    expect(screen.getByText(/fora da aplicação desktop/)).toBeTruthy();
  });
});

describe("ecrã de projeto", () => {
  async function openProject(n: number) {
    apiMock.listPhotos.mockResolvedValue(rows(n));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("grelha virtualizada: 3000 fotografias não criam 3000 elementos", async () => {
    await openProject(3000);
    expect(screen.getByRole("button", { name: /Todas\s*3/ })).toBeTruthy();
    const cells = document.querySelectorAll(".cell");
    expect(cells.length).toBeGreaterThan(0);
    expect(cells.length).toBeLessThan(200);
  });

  it("já não há secções futuras desativadas (os álbuns são uma vista)", async () => {
    await openProject(5);
    expect(screen.queryByText("Brevemente")).toBeNull();
    expect(screen.getByRole("button", { name: "Álbum" })).toBeTruthy();
  });

  it("atalhos classificam a fotografia com foco e gravam no núcleo", async () => {
    await openProject(10);
    fireEvent.mouseDown(document.querySelector('[data-id="3"]')!);
    fireEvent.keyDown(window, { key: "4" });
    fireEvent.keyDown(window, { key: "p" });
    fireEvent.keyDown(window, { key: "7" });
    await act(async () => {});
    expect(apiMock.setStars).toHaveBeenCalledWith([3], 4);
    expect(apiMock.setFlag).toHaveBeenCalledWith([3], 1);
    expect(apiMock.setColor).toHaveBeenCalledWith([3], "yellow");
    const p = useStore.getState().photos.find((r) => r.id === 3)!;
    expect([p.stars, p.flag, p.color]).toEqual([4, 1, "yellow"]);
  });

  it("seleção múltipla aplica a classificação a todas", async () => {
    await openProject(10);
    fireEvent.mouseDown(document.querySelector('[data-id="2"]')!);
    fireEvent.mouseDown(document.querySelector('[data-id="4"]')!, { shiftKey: true });
    fireEvent.keyDown(window, { key: "x" });
    await act(async () => {});
    expect(apiMock.setFlag).toHaveBeenCalledWith(expect.arrayContaining([2, 3, 4]), -1);
    expect((apiMock.setFlag.mock.calls[0] as unknown[])[0]).toHaveLength(3);
  });

  it("filtro Favoritas e abertura do visualizador", async () => {
    await openProject(10);
    fireEvent.click(screen.getByRole("button", { name: /Favoritas\s*1/ }));
    expect(document.querySelectorAll(".cell").length).toBe(1);
    fireEvent.doubleClick(document.querySelector('[data-id="2"]')!);
    expect(await screen.findByText(/1 de 1/)).toBeTruthy();
    expect(screen.getByRole("button", { name: /Ajustar/ })).toBeTruthy();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByText(/1 de 1/)).toBeNull();
  });

  it("revertida se o núcleo recusar a classificação", async () => {
    await openProject(3);
    apiMock.setStars.mockRejectedValueOnce({ code: "error.database", detail: "x" });
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    fireEvent.keyDown(window, { key: "5" });
    await act(async () => {});
    expect(useStore.getState().photos[0]!.stars).toBe(0);
    expect(await screen.findByRole("alert")).toBeTruthy();
  });
});

describe("formatos e estados (Fase 2)", () => {
  it("mostra o tipo de ficheiro e o selo de suporte parcial na grelha", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(5).map((r, i) => (i === 2 ? { ...r, fileName: "IMG_0003.CR3" } : r)));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    const cell = document.querySelector('[data-id="3"]')!;
    expect(cell.textContent).toContain("CR3");
    expect(cell.textContent).toContain("Parcial");
    expect(document.querySelectorAll(".cell-partial").length).toBe(1);
  });

  it("painel de informação explica suporte parcial e ficheiros ilegíveis", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(3).map((r, i) => (i === 0 ? { ...r, thumb: "failed" as const } : r)));
    apiMock.photoDetails.mockResolvedValue({
      id: 1, path: "/f/IMG.NEF", sourcePath: null, fileName: "IMG.NEF", fileSize: 1000, format: "raw-tiff", width: 6000, height: 4000,
      orientation: 1, captureTime: null, captureOffset: null, cameraMake: "Nikon", cameraModel: "Z 9", cameraSerial: null, lens: null,
      iso: null, exposureTime: null, fNumber: null, focalLength: null, focalLength35mm: null, importedAt: 0, thumbError: "dados truncados",
      bitDepth: 14, colorSpace: "Câmara (RGBG, 3 cores)", codec: "libraw", codecDetail: "LibRaw 0.22.3", embeddedPreview: "8256×5504",
      partialNote: "Só a preview embutida pode ser mostrada.",
    } as never);
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    expect(await screen.findByText("Não foi possível ler esta fotografia")).toBeTruthy();
    expect(screen.getByText("Suporte parcial")).toBeTruthy();
    expect(screen.getByText("NEF (RAW)")).toBeTruthy();
    expect(screen.getByText("14 bits")).toBeTruthy();
    expect(screen.getByText("8256×5504")).toBeTruthy();
  });
});

describe("seleção assistida (Fase 3)", () => {
  const recs = [
    { asset: 1, label: "best", groupId: 7, rank: 0, groupSize: 3 },
    { asset: 2, label: "alternative", groupId: 7, rank: 1, groupSize: 3 },
    { asset: 3, label: "reject", groupId: 7, rank: 2, groupSize: 3 },
    { asset: 4, label: "review", groupId: null, rank: 0, groupSize: 1 },
  ];
  async function openAnalyzed() {
    apiMock.listPhotos.mockResolvedValue(rows(6).map((r) => ({ ...r, flag: 0 })));
    apiMock.analysisStatus.mockResolvedValue(analysisStatus(6, 6));
    apiMock.listRecommendations.mockResolvedValue(recs);
    apiMock.listGroups.mockResolvedValue([{ id: 7, kind: "sequence", size: 3, members: [1, 2, 3] }]);
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("antes da análise: explica e oferece analisar (nada corre sozinho)", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(10));
    apiMock.analysisStatus.mockResolvedValue(analysisStatus(0, 10));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    expect(screen.getByText(/nada é enviado para a internet/)).toBeTruthy();
    expect(apiMock.startAnalysis).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Analisar 10 fotografias" }));
    await act(async () => {});
    expect(apiMock.startAnalysis).toHaveBeenCalledWith(false);
  });

  it("etiquetas na grelha, contagens e filtro por etiqueta", async () => {
    await openAnalyzed();
    expect(document.querySelector('[data-id="1"]')!.textContent).toContain("Melhor");
    expect(document.querySelector('[data-id="1"]')!.textContent).toContain("1/3");
    expect(document.querySelector('[data-id="3"]')!.textContent).toContain("Rejeitar?");
    fireEvent.click(screen.getByRole("button", { name: /Rejeitar sugerido\s*1/ }));
    expect(document.querySelectorAll(".cell").length).toBe(1);
    expect(document.querySelector('[data-id="3"]')).toBeTruthy();
  });

  it("modo e objetivo de entrega são enviados ao núcleo", async () => {
    await openAnalyzed();
    fireEvent.change(screen.getByRole("combobox", { name: /Modo/ }), { target: { value: "strict" } });
    await act(async () => {});
    expect(apiMock.setSelectionSettings).toHaveBeenCalledWith({ mode: "strict", target: null });
    const input = screen.getByPlaceholderText("Sem objetivo");
    fireEvent.change(input, { target: { value: "350" } });
    fireEvent.blur(input);
    await act(async () => {});
    expect(apiMock.setSelectionSettings).toHaveBeenCalledWith({ mode: "balanced", target: 350 });
  });

  it("vista de sequências: aceitar sugestões do grupo e comparar", async () => {
    await openAnalyzed();
    fireEvent.click(screen.getByRole("button", { name: /^Sequências\s*1$/ }));
    const row = screen.getByRole("region", { name: "Sequência" });
    expect(within(row).getByText(/3 fotografias/)).toBeTruthy();
    fireEvent.click(within(row).getByRole("button", { name: "Aceitar sugestões" }));
    await act(async () => {});
    expect(apiMock.acceptGroup).toHaveBeenCalledWith(7);
    fireEvent.click(within(row).getByRole("button", { name: "Comparar" }));
    const cmp = screen.getByRole("region", { name: "Comparação" });
    // Melhor + alternativa (a rejeitada fica de fora da comparação inicial).
    expect(cmp.querySelectorAll(".compare-pane").length).toBe(2);
  });

  it("comparação: teclas no painel ativo, trocar por outra e fechar", async () => {
    await openAnalyzed();
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    fireEvent.keyDown(window, { key: "c" });
    const cmp = screen.getByRole("region", { name: "Comparação" });
    expect(cmp.querySelectorAll(".compare-pane").length).toBe(3);
    fireEvent.keyDown(window, { key: "ArrowRight" });
    fireEvent.keyDown(window, { key: "x" });
    fireEvent.keyDown(window, { key: "3" });
    await act(async () => {});
    expect(apiMock.setFlag).toHaveBeenCalledWith([2], -1);
    expect(apiMock.setStars).toHaveBeenCalledWith([2], 3);
    const swap = within(cmp).getAllByRole("combobox", { name: "Selecionar outra…" })[0]!;
    fireEvent.change(swap, { target: { value: "3" } });
    expect(useStore.getState().compare).toEqual([3, 2, 1]);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("region", { name: "Comparação" })).toBeNull();
  });

  it("painel de informação: explicação e pontuações individuais", async () => {
    apiMock.analysisDetails.mockResolvedValue({
      analysis: { models: "x", width: 2048, height: 1365, sharpness: 2, focusPeak: 2.5, focusCenter: 2, motionBlur: 0.1, brightness: 0.5, highlightsClipped: 0.01, shadowsCrushed: 0, contrast: 0.6, noise: 2, tiltDegrees: 0.4, phash: 1, dhash: 2, durationMs: 100 },
      faces: [{ x: 0.1, y: 0.1, w: 0.1, h: 0.1, px: 160, detScore: 0.9, presence: 0.99, blinkLeft: 0.1, blinkRight: 0.12, smile: 0.8, jawOpen: 0.1, sharpness: 2.4, eyeSharpness: 2.5, cut: false }],
      recommendation: { asset: 1, label: "best", rank: 0, groupSize: 3, summary: "bestInSequence", reasons: '[{"code":"sharpestFace"},{"code":"allFacesVisible","value":2},{"code":"bestExposure"}]', focus: 0.95, motion: 0.9, exposure: 0.88, noise: 0.8, eyes: 1, expression: 0.8, composition: 1, technical: 0.9, people: 0.93, overall: 0.92 },
      groupId: 7,
      error: null,
    });
    await openAnalyzed();
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    expect(await screen.findByText("Melhor fotografia desta sequência: maior nitidez facial, ambos os rostos visíveis e melhor exposição.")).toBeTruthy();
    expect(screen.getByText("Olhos abertos")).toBeTruthy();
    expect(screen.getByText("Sequência de 3 · 1.ª")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Aceitar: marcar favorita" }));
    await act(async () => {});
    expect(apiMock.acceptRecommendations).toHaveBeenCalledWith([1]);
  });
});

describe("organização do casamento (Fase 4)", () => {
  const moments = [
    { id: 1, kind: "bride_prep", name: null, sort: 10, custom: false, count: 1 },
    { id: 13, kind: "kiss", name: null, sort: 130, custom: false, count: 1 },
    { id: 21, kind: "cake", name: null, sort: 210, custom: false, count: 0 },
    { id: 50, kind: null, name: "Fogo de artifício", sort: 999, custom: true, count: 0 },
  ];
  const persons = [
    { id: 7, name: "Ana", role: "bride", hidden: false, faceCount: 3, photoCount: 3, cover: { faceId: 1, asset: 1, x: 0.4, y: 0.2, w: 0.1, h: 0.1, source: 1 } },
    { id: 8, name: "Pedro", role: "groom", hidden: false, faceCount: 2, photoCount: 2, cover: null },
    { id: 9, name: null, role: "bride_mother", hidden: false, faceCount: 1, photoCount: 1, cover: null },
    { id: 10, name: null, role: null, hidden: false, faceCount: 4, photoCount: 4, cover: null },
  ];
  async function openOrganized() {
    apiMock.listPhotos.mockResolvedValue(rows(6).map((r) => ({ ...r, flag: 0 })));
    apiMock.analysisStatus.mockResolvedValue(analysisStatus(6, 6));
    apiMock.listMoments.mockResolvedValue(moments);
    apiMock.assetMoments.mockResolvedValue([
      { asset: 1, momentId: 1, confidence: 0.72, source: "auto", reasons: "[]" },
      { asset: 2, momentId: 13, confidence: null, source: "user", reasons: null },
    ]);
    apiMock.listPersons.mockResolvedValue(persons);
    // Fotografia 1: noiva; 2: noiva + noivo; 3: noiva + mãe da noiva + noivo; 4: noivo.
    apiMock.photoPersons.mockResolvedValue([[1, 7], [2, 7], [2, 8], [3, 7], [3, 9], [3, 8], [4, 8]]);
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }
  const cells = () => [...document.querySelectorAll(".cell")].map((c) => Number(c.getAttribute("data-id")));

  it("pesquisas: noiva, noivos juntos, noiva + mãe, uma pessoa", async () => {
    await openOrganized();
    fireEvent.click(screen.getByRole("button", { name: /Todas as fotografias da noiva/ }));
    expect(cells()).toEqual([1, 2, 3]);
    fireEvent.click(screen.getByRole("button", { name: /Noivos juntos/ }));
    expect(cells()).toEqual([2, 3]);
    fireEvent.click(screen.getByRole("button", { name: /Noiva \+ mãe/ }));
    expect(cells()).toEqual([3]);
    expect((screen.getByRole("button", { name: /Noivo \+ pai/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: /^Pedro\s*2$/ }));
    expect(cells()).toEqual([2, 3, 4]);
  });

  it("momentos: filtro, por classificar e arrastar fotografias para um momento", async () => {
    await openOrganized();
    const panel = screen.getByRole("region", { name: "Momentos" });
    expect(within(panel).getByText("Preparação da noiva")).toBeTruthy();
    expect(within(panel).queryByText("Bolo")).toBeNull(); // vazio: escondido
    expect(within(panel).getByText("Fogo de artifício")).toBeTruthy(); // personalizado: sempre
    fireEvent.click(within(panel).getByRole("button", { name: /Por classificar\s*4/ }));
    expect(cells()).toEqual([3, 4, 5, 6]);
        fireEvent.click(within(panel).getByRole("button", { name: /Por classificar/ }));
    
        // Arrastar a fotografia 3 (com a 4 selecionada também) para “Fogo de artifício”.
    fireEvent.mouseDown(document.querySelector('[data-id="3"]')!);
    fireEvent.mouseDown(document.querySelector('[data-id="4"]')!, { shiftKey: true });
    fireEvent.mouseMove(window, { clientX: 300, clientY: 300, buttons: 1 });
    expect(await screen.findByText(/Largue 2 fotografias num momento/)).toBeTruthy();
        expect(within(panel).queryByText("Bolo")).toBeNull(); // a lista não muda durante o arrasto
    fireEvent.mouseUp(within(panel).getByRole("button", { name: /Fogo de artifício/ }));
    await act(async () => {});
        expect(apiMock.assignMoment).toHaveBeenCalledWith([3, 4], 50);
    expect(screen.queryByText(/Largue 2 fotografias/)).toBeNull(); // regressão: o aviso desaparece ao largar
  });

  it("criar momento personalizado e escolher momento no painel", async () => {
    await openOrganized();
    fireEvent.click(screen.getByRole("button", { name: /\+ Novo momento/ }));
    const input = screen.getByPlaceholderText("Nome do momento");
    fireEvent.change(input, { target: { value: "Fogo de artifício 2" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.blur(input);
    await act(async () => {});
    expect(apiMock.createMoment).toHaveBeenCalledWith("Fogo de artifício 2", null);
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    expect(await screen.findByText("Automático · confiança 72 %")).toBeTruthy();
    fireEvent.change(screen.getByRole("combobox", { name: /Momento/ }), { target: { value: "21" } });
    await act(async () => {});
    expect(apiMock.assignMoment).toHaveBeenCalledWith([1], 21);
  });

  it("gerir pessoas: nome, papel, não é esta pessoa, juntar", async () => {
    apiMock.personFaces.mockResolvedValue([{ faceId: 31, asset: 1, x: 0.4, y: 0.2, w: 0.1, h: 0.1, source: 0 }]);
    await openOrganized();
    fireEvent.click(screen.getByRole("button", { name: /^Pessoas$/ }));
    const card = await screen.findByRole("article", { name: "Pessoa 10" });
    fireEvent.change(within(card).getByRole("textbox", { name: "Nome" }), { target: { value: "Rui" } });
    fireEvent.blur(within(card).getByRole("textbox", { name: "Nome" }));
    await act(async () => {});
    expect(apiMock.updatePerson).toHaveBeenCalledWith(10, "Rui", null, false);
    fireEvent.change(within(card).getByRole("combobox", { name: "Papel" }), { target: { value: "best_man" } });
    await act(async () => {});
    // Regressão: mudar o papel logo a seguir mantém o nome acabado de escrever.
    expect(apiMock.updatePerson).toHaveBeenLastCalledWith(10, "Rui", "best_man", false);
    fireEvent.click(await within(card).findByRole("button", { name: "Não é esta pessoa" }));
    await act(async () => {});
    expect(apiMock.excludeFaces).toHaveBeenCalledWith([31]);
    fireEvent.click(within(card).getByRole("checkbox", { name: "Selecionar" }));
    fireEvent.click(within(screen.getByRole("article", { name: "Ana" })).getByRole("checkbox", { name: "Selecionar" }));
    fireEvent.click(screen.getByRole("button", { name: "Juntar 2 selecionadas" }));
    await act(async () => {});
    expect(apiMock.mergePersons).toHaveBeenCalledWith(10, 7);
  });

  it("sincronizar câmaras: sugestão (Sony +3min42s, DJI −1min15s) e reconstruir", async () => {
    apiMock.listCameras.mockResolvedValue([
      { key: "Canon EOS R5", make: "Canon", model: "EOS R5", serial: null, photos: 900, firstTime: "2026-06-15T10:00:00", lastTime: "2026-06-15T23:50:00", offsetMs: 0, reference: false },
      { key: "SONY ILCE-7M4", make: "SONY", model: "ILCE-7M4", serial: null, photos: 400, firstTime: null, lastTime: null, offsetMs: 0, reference: false },
      { key: "DJI FC3582", make: "DJI", model: "FC3582", serial: null, photos: 40, firstTime: null, lastTime: null, offsetMs: 0, reference: false },
    ]);
    apiMock.suggestOffsets.mockResolvedValue([
      { camera: "SONY ILCE-7M4", offsetMs: -222000, votes: 31, agreement: 0.94 },
      { camera: "DJI FC3582", offsetMs: 75000, votes: 6, agreement: 0.8 },
    ]);
    await openOrganized();
    fireEvent.click(screen.getByRole("button", { name: /Sincronizar câmaras/ }));
    const dialog = await screen.findByRole("dialog", { name: "Sincronizar câmaras" });
    await act(async () => {});
    fireEvent.click(within(dialog).getByRole("button", { name: "Sugerir automaticamente" }));
    await act(async () => {});
    expect(apiMock.suggestOffsets).toHaveBeenCalledWith("Canon EOS R5");
    expect((within(dialog).getByRole("textbox", { name: /Desvio de SONY/ }) as HTMLInputElement).value).toBe("+3min42s");
    expect((within(dialog).getByRole("textbox", { name: /Desvio de DJI/ }) as HTMLInputElement).value).toBe("-1min15s");
    fireEvent.change(within(dialog).getByRole("textbox", { name: /Desvio de DJI/ }), { target: { value: "abc" } });
    expect((within(dialog).getByRole("button", { name: /Aplicar/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole("textbox", { name: /Desvio de DJI/ }), { target: { value: "-1:15" } });
    fireEvent.click(within(dialog).getByRole("button", { name: /Aplicar/ }));
    await act(async () => {});
    expect(apiMock.setCameraOffsets).toHaveBeenCalledWith([
      { key: "Canon EOS R5", offsetMs: 0, reference: true },
      { key: "SONY ILCE-7M4", offsetMs: -222000, reference: false },
      { key: "DJI FC3582", offsetMs: 75000, reference: false },
    ]);
  });
});

describe("edição (Fase 5)", () => {
  async function openDevelop(n = 4) {
    vi.useRealTimers();
    apiMock.listPhotos.mockResolvedValue(rows(n).map((r) => ({ ...r, flag: 0 })));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    fireEvent.mouseDown(document.querySelector('[data-id="2"]')!);
    fireEvent.keyDown(window, { key: "d" });
    await act(async () => {});
    return screen.getByRole("complementary", { name: "Ajustes" });
  }
  const slider = (name: string) => screen.getByRole("slider", { name }) as HTMLInputElement;

  it("Fase 15: um Auto que termina depois de mudar de fotografia não passa para a seguinte", async () => {
    let finish: (v: unknown) => void = () => {};
    apiMock.autoEditPhoto.mockImplementation(
      (key: number) =>
        new Promise((res) => {
          finish = () => {
            const v = editViewOf(key);
            v.settings.light.exposure = 1.5;
            res({ view: { ...v, position: 1, canUndo: true }, report: null });
          };
        }),
    );
    const panel = await openDevelop();
    fireEvent.click(within(panel).getByRole("button", { name: /✦ Auto/ }));
    // Seguinte (→) antes de o Auto da fotografia 2 terminar.
    act(() => useStore.getState().setSelection({ ids: new Set([3]), focus: 3, anchor: 3 }));
    await act(async () => {});
    await waitFor(() => expect(apiMock.editView).toHaveBeenCalledWith(3));
    await act(async () => finish(null));
    await act(async () => {});
    apiMock.commitEdit.mockClear();
    // Um ajuste na fotografia 3 grava a 3 com os SEUS ajustes (sem o +1,5 EV da 2).
    fireEvent.change(slider("Contraste"), { target: { value: "10" } });
    fireEvent.pointerUp(slider("Contraste"));
    await act(async () => {});
    const calls = apiMock.commitEdit.mock.calls;
    expect(calls.length).toBe(1);
    expect(calls[0]![0]).toBe(3);
    expect((calls[0]![2] as { light: { exposure: number } }).light.exposure).toBe(0);
  });

  it("Fase 15: o último ajuste por gravar é gravado na sua fotografia ao mudar de fotografia", async () => {
    await openDevelop();
    await waitFor(() => expect(apiMock.editView).toHaveBeenCalledWith(2));
    await act(async () => {});
    apiMock.commitEdit.mockClear();
    // Arrastar sem "largar" no próprio controlo (sem pointerup).
    fireEvent.change(slider("Exposição"), { target: { value: "0.7" } });
    await act(async () => {});
    expect(apiMock.commitEdit).not.toHaveBeenCalled();
    act(() => useStore.getState().setSelection({ ids: new Set([3]), focus: 3, anchor: 3 }));
    await act(async () => {});
    const c = apiMock.commitEdit.mock.calls.find((x) => x[0] === 2);
    expect(c, "ajuste da fotografia 2 gravado").toBeTruthy();
    expect((c![2] as { light: { exposure: number } }).light.exposure).toBe(0.7);
    expect(apiMock.commitEdit.mock.calls.some((x) => x[0] === 3)).toBe(false);
  });

  it("abrir o editor (D), arrastar a exposição: renders rápidos e, ao largar, final + gravação", async () => {
    await openDevelop();
    expect(apiMock.editView).toHaveBeenCalledWith(2);
    await waitFor(() => expect(apiMock.renderEdit).toHaveBeenCalled());
    expect(apiMock.renderEdit.mock.calls.at(-1)![4]).toBe("final");
    apiMock.renderEdit.mockClear();
    fireEvent.change(slider("Exposição"), { target: { value: "0.5" } });
    fireEvent.change(slider("Exposição"), { target: { value: "0.8" } });
    await act(async () => {});
    const firstFast = apiMock.renderEdit.mock.calls[0]!;
    expect(firstFast[4]).toBe("fast");
    expect((firstFast[1] as { light: { exposure: number } }).light.exposure).toBeGreaterThan(0);
    expect(apiMock.commitEdit).not.toHaveBeenCalled(); // durante o arrasto não grava
        fireEvent.pointerUp(slider("Exposição"), { target: { value: "0.8" } });
    await act(async () => {});
    // Regressão: com o foco no deslizador, os atalhos do editor continuam a funcionar.
    fireEvent.keyDown(slider("Exposição"), { key: "\\" });
    expect(screen.getByRole("button", { name: /^Antes$/ })).toBeTruthy();
    fireEvent.keyDown(slider("Exposição"), { key: "\\" });
    expect(apiMock.commitEdit).toHaveBeenCalledWith(2, "light.exposure", expect.objectContaining({ light: expect.objectContaining({ exposure: 0.8 }) }));
    // Depois de parar, o render final chega.
    await waitFor(() => expect(apiMock.renderEdit.mock.calls.at(-1)![4]).toBe("final"), { timeout: 2000 });
    expect(screen.getByText("Exposição", { selector: ".history-list *" })).toBeTruthy();
  });

  it("desfazer, antes/depois, conta-gotas, endireitar automático", async () => {
    await openDevelop();
    await act(async () => {});
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await act(async () => {});
    expect(apiMock.undoEdit).toHaveBeenCalledWith(2);
    apiMock.renderEdit.mockClear();
    fireEvent.click(screen.getByRole("button", { name: /^Depois$/ }));
    await act(async () => {});
    expect(screen.getByRole("button", { name: /^Antes$/ })).toBeTruthy();
    const call = apiMock.renderEdit.mock.calls.at(-1)!;
    expect(call[1]).toEqual(defaultSettings()); // antes = original
    fireEvent.click(screen.getByRole("button", { name: /Conta-gotas/ }));
    fireEvent.click(document.querySelector(".develop-image")!, { clientX: 5, clientY: 5 });
    await act(async () => {});
    expect(apiMock.pickWhiteBalance).toHaveBeenCalled();
    expect(apiMock.commitEdit).toHaveBeenCalledWith(2, "wb.picker", expect.objectContaining({ wb: { temperature: 23, tint: -8 } }));
    fireEvent.click(screen.getByRole("button", { name: /Geometria/ }));
    // Upright: modo Vertical aplica a correção calculada (perspetiva + endireitar + focal).
    const upright = screen.getByRole("group", { name: "Upright" });
    fireEvent.click(within(upright).getByRole("button", { name: "Vertical" }));
    await act(async () => {});
    expect(apiMock.upright).toHaveBeenCalledWith(2, expect.anything(), "vertical");
    expect(apiMock.commitEdit).toHaveBeenLastCalledWith(
      2,
      "geometry.upright",
      expect.objectContaining({ geometry: expect.objectContaining({ upright: "vertical", vertical: 31.2, horizontal: 0, straighten: -1.25, focal35: 24 }) }),
    );
    // Desligado repõe a perspetiva.
    fireEvent.click(within(upright).getByRole("button", { name: "Desligado" }));
    await act(async () => {});
    expect(apiMock.commitEdit).toHaveBeenLastCalledWith(2, "geometry.upright", expect.objectContaining({ geometry: expect.objectContaining({ upright: "", vertical: 0, straighten: 0 }) }));
  });

  it("copiar (sem geometria por omissão), colar, sincronizar com a seleção, cópia virtual, instantâneos", async () => {
    await openDevelop();
    fireEvent.click(screen.getByRole("button", { name: "Copiar" }));
    const dlg = screen.getByRole("dialog", { name: "Copiar definições" });
    expect((within(dlg).getByRole("checkbox", { name: "Geometria" }) as HTMLInputElement).checked).toBe(false);
    fireEvent.click(within(dlg).getByRole("button", { name: "Copiar" }));
    fireEvent.click(screen.getByRole("button", { name: "Colar" }));
    await act(async () => {});
    expect(apiMock.pasteSettings).toHaveBeenCalledWith([2], expect.anything(), expect.not.arrayContaining(["geometry"]));
    act(() => useStore.getState().setSelection({ ids: new Set([2, 3, 4]), focus: 2, anchor: 2 }));
    // Com várias selecionadas, Colar aplica a todas (como no Lightroom).
    fireEvent.click(screen.getByRole("button", { name: "Colar" }));
    await act(async () => {});
    expect(apiMock.pasteSettings).toHaveBeenLastCalledWith([2, 3, 4], expect.anything(), expect.any(Array));
    fireEvent.click(screen.getByRole("button", { name: "Sincronizar" }));
    const syncDlg = screen.getByRole("dialog", { name: "Sincronizar com 2 fotografias" });
    fireEvent.click(within(syncDlg).getByRole("radio", { name: "Copiar os mesmos valores" }));
    fireEvent.click(within(syncDlg).getByRole("button", { name: "Sincronizar" }));
    await act(async () => {});
    expect(apiMock.pasteSettings).toHaveBeenLastCalledWith([3, 4], expect.anything(), expect.any(Array));
    fireEvent.change(screen.getByRole("textbox", { name: "Nome do instantâneo" }), { target: { value: "Quente" } });
    fireEvent.submit(screen.getByRole("textbox", { name: "Nome do instantâneo" }));
    await act(async () => {});
    expect(apiMock.createSnapshot).toHaveBeenCalledWith(2, "Quente");
    fireEvent.click(await screen.findByRole("button", { name: "Quente" }));
    await act(async () => {});
    expect(apiMock.applySnapshot).toHaveBeenCalledWith(2, 5);
    fireEvent.click(screen.getByRole("button", { name: "Cópia virtual" }));
    await act(async () => {});
    expect(apiMock.createVirtualCopy).toHaveBeenCalledWith(2);
    expect(useStore.getState().selection.focus).toBe(2 ** 40 + 77);
  });

  it("recorte: renderiza sem recorte, proporção e sair com Esc volta à grelha", async () => {
    await openDevelop();
    await act(async () => {});
    apiMock.renderEdit.mockClear();
    fireEvent.keyDown(window, { key: "r" });
    await act(async () => {});
    expect(apiMock.renderEdit.mock.calls.at(-1)![5]).toBe(true);
    expect(screen.getByRole("group", { name: /Área de recorte/ })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Geometria/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Proporção" }), { target: { value: "1:1" } });
    await act(async () => {});
    const last = apiMock.commitEdit.mock.calls.at(-1)!;
    expect(last[1]).toBe("geometry.aspect");
    const crop = (last[2] as { geometry: { crop: { w: number; h: number } } }).geometry.crop;
    expect(crop.w * 40).toBeCloseTo(crop.h * 30, 3); // 1:1 em píxeis do frame 40×30
    fireEvent.keyDown(window, { key: "Escape" });
    expect(useStore.getState().view).toBe("grid");
  });

  it("frames, recorte com proporção e etiquetas do histórico", () => {
    const f = parseFrame(frameBuffer(4, 3));
    expect([f.width, f.height, f.rgba.length, f.histogram[1][128]]).toEqual([4, 3, 48, 0]);
    expect(f.histogram[0][128]).toBe(12);
    const c = fitCropToAspect({ x: 0, y: 0, w: 1, h: 1 }, 1, 300, 200);
    expect(c.w * 300).toBeCloseTo(c.h * 200);
    expect(c.x).toBeCloseTo((1 - c.w) / 2);
    const t = i18n.t.bind(i18n) as (k: string, o?: Record<string, unknown>) => string;
    expect(historyLabel("light.exposure", t)).toBe("Exposição");
    expect(historyLabel("reset.light", t)).toBe("Repor Luz");
    expect(historyLabel("paste", t)).toBe("Colar definições");
    expect(historyLabel("auto", t)).toBe("Edição automática");
  });

  it("edição automática: intensidade, analisar esta fotografia, relatório e AI Denoise recomendado", async () => {
    const report = {
      exposure: 0.6,
      temperature: 8,
      tint: -3,
      noise: { luminance: 6, chroma: 3, iso: 6400, sensorFactor: 1, level: "high", aiRecommended: true },
      tilt: -1.2,
      cropped: false,
      redEyes: 2,
      spots: 0,
      faces: 3,
    };
    apiMock.autoEditPhoto.mockImplementation(async (key: number) => {
      const v = editViewOf(key);
      v.settings.light.exposure = 0.6;
      return { view: { ...v, history: [{ seq: 1, label: "auto", createdAt: 1 }], position: 1, canUndo: true }, report };
    });
    await openDevelop();
    const panel = screen.getByRole("complementary", { name: "Ajustes" });
    expect(within(panel).getByRole("region", { name: "Automático" })).toBeTruthy();
    fireEvent.click(within(panel).getByRole("button", { name: "Intensa" }));
    expect(useStore.getState().autoOptions.intensity).toBe("strong");
    // Personalizada: força e grupos.
    fireEvent.click(within(panel).getByRole("button", { name: "Personalizada" }));
    expect(within(panel).getByRole("slider", { name: "Força (%)" })).toBeTruthy();
    fireEvent.click(within(panel).getByRole("checkbox", { name: "Balanço de brancos" }));
    expect(useStore.getState().autoOptions.whiteBalance).toBe(false);
    // Voltar a uma predefinida volta a ligar todos os grupos.
    fireEvent.click(within(panel).getByRole("button", { name: "Intensa" }));
    expect(useStore.getState().autoOptions.whiteBalance).toBe(true);
    fireEvent.click(within(panel).getByRole("button", { name: /✦ Auto/ }));
    await act(async () => {});
    expect(apiMock.autoEditPhoto).toHaveBeenCalledWith(2, expect.objectContaining({ intensity: "strong", redEye: true, crop: false }));
    expect(screen.getByText("Exposição +0,60 EV")).toBeTruthy();
    expect(screen.getByText("Ruído: alto (ISO 6400)")).toBeTruthy();
    expect(screen.getByText("Horizonte endireitado 1,2°")).toBeTruthy();
    expect(screen.getByText("2 olhos vermelhos corrigidos")).toBeTruthy();
    expect(screen.getByText("Edição automática", { selector: ".history-list *" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar AI Denoise" }));
    await act(async () => {});
    expect(apiMock.commitEdit).toHaveBeenCalledWith(2, "detail.aiDenoise", expect.objectContaining({ detail: expect.objectContaining({ aiDenoise: 50 }) }));
    // Atalho Ctrl+U.
    apiMock.autoEditPhoto.mockClear();
    fireEvent.keyDown(window, { key: "u", ctrlKey: true });
    await act(async () => {});
    expect(apiMock.autoEditPhoto).toHaveBeenCalledTimes(1);
  });

  it("retoque (olhos vermelhos/manchas só) e vista dividida antes/depois", async () => {
    apiMock.autoEditPhoto.mockImplementation(async (key: number) => {
      const v = editViewOf(key);
      v.settings.retouch.redEyes = [{ x: 0.4, y: 0.3, r: 0.01 }];
      return { view: v, report: { exposure: 0, temperature: 0, tint: 0, noise: { luminance: 0, chroma: 0, iso: null, sensorFactor: 1.2, level: "none", aiRecommended: false }, tilt: null, cropped: false, redEyes: 1, spots: 0, faces: 1 } };
    });
    await openDevelop();
    const panel = screen.getByRole("complementary", { name: "Ajustes" });
    fireEvent.click(within(panel).getByRole("button", { name: /Retoque/ }));
    fireEvent.click(within(panel).getAllByRole("button", { name: "Detetar" })[0]!);
    await act(async () => {});
    const opts = apiMock.autoEditPhoto.mock.calls.at(-1)![1] as Record<string, unknown>;
    expect(opts).toMatchObject({ redEye: true, dust: false, tone: false, whiteBalance: false, color: false, denoise: false, lens: false, straighten: false, crop: false });
    expect(screen.getByText("Olhos vermelhos: 1")).toBeTruthy();
    fireEvent.click(within(panel).getAllByRole("button", { name: "Remover" })[0]!);
    await act(async () => {});
    expect(apiMock.commitEdit).toHaveBeenCalledWith(2, "retouch.redEye", expect.objectContaining({ retouch: { redEyes: [], spots: [], patches: [] } }));
    // Vista dividida (Y): o "antes" tem o mesmo enquadramento, sem os ajustes.
    apiMock.renderEdit.mockClear();
    fireEvent.keyDown(window, { key: "y" });
    await act(async () => {});
    await waitFor(() => expect(screen.getByRole("slider", { name: /Antes\/depois lado a lado/ })).toBeTruthy());
    const before = apiMock.renderEdit.mock.calls.find((c) => JSON.stringify(c[1]) === JSON.stringify(beforeForSplit(defaultSettings())));
    expect(before).toBeTruthy();
    const handle = screen.getByRole("slider", { name: /Antes\/depois lado a lado/ });
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(handle.getAttribute("aria-valuenow")).toBe("45");
  });
});

describe("presets e consistência (Fase 7)", () => {
  async function openApp(n = 4) {
    vi.useRealTimers();
    apiMock.listPhotos.mockResolvedValue(rows(n).map((r) => ({ ...r, flag: 0 })));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("editor: pré-visualizar ao passar o rato, aplicar com Amount, criar preset", async () => {
    await openApp();
    fireEvent.mouseDown(document.querySelector('[data-id="2"]')!);
    fireEvent.keyDown(window, { key: "d" });
    await act(async () => {});
    const panel = screen.getByRole("complementary", { name: "Ajustes" });
    const sec = within(panel).getByRole("region", { name: "Presets" });
    expect(within(sec).getByText(/Preto e branco/)).toBeTruthy();
    fireEvent.change(within(sec).getByRole("slider", { name: "Intensidade (%)" }), { target: { value: "60" } });
    apiMock.renderEdit.mockClear();
    fireEvent.mouseEnter(within(sec).getByRole("button", { name: "Natural Wedding" }));
    await act(async () => {});
    expect(apiMock.presetPreviewSettings).toHaveBeenLastCalledWith(expect.anything(), "builtin:natural-wedding", 60);
    await waitFor(() => expect(apiMock.renderEdit.mock.calls.some((c) => (c[1] as { light: { contrast: number } }).light.contrast === 6)).toBe(true));
    expect(apiMock.commitEdit).not.toHaveBeenCalled();
    fireEvent.click(within(sec).getByRole("button", { name: "Natural Wedding" }));
    await act(async () => {});
    expect(apiMock.commitEdit).toHaveBeenCalledWith(2, "preset:Natural Wedding", expect.objectContaining({ light: expect.objectContaining({ contrast: 6 }) }));
    const t = i18n.t.bind(i18n) as (k: string, o?: Record<string, unknown>) => string;
    expect(historyLabel("preset:Film Warm", t)).toBe("Preset: Film Warm");
    expect(historyLabel("sync", t)).toBe("Sincronizado com referência");
    // Criar preset: só os componentes escolhidos.
    fireEvent.click(within(sec).getByRole("button", { name: "+ Criar preset" }));
    const dlg = screen.getByRole("dialog", { name: "Criar preset a partir desta fotografia" });
    fireEvent.change(within(dlg).getByRole("textbox", { name: "Nome" }), { target: { value: "Igreja quente" } });
    fireEvent.click(within(dlg).getByRole("checkbox", { name: /^Exposição$/ }));
    fireEvent.click(within(dlg).getByRole("button", { name: "Criar preset" }));
    await act(async () => {});
    expect(apiMock.createPreset).toHaveBeenCalledWith("Igreja quente", "", expect.arrayContaining(["tone", "exposure"]), expect.anything());
    expect(apiMock.createPreset.mock.calls[0]![2]).not.toContain("lut");
  });

  it("gerir: renomear, duplicar, ordenar, eliminar com confirmação, exportar", async () => {
    await openApp();
    fireEvent.mouseDown(document.querySelector('[data-id="2"]')!);
    fireEvent.keyDown(window, { key: "d" });
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Gerir…" }));
    const dlg = screen.getByRole("dialog", { name: "Gerir presets" });
    const rowOf = (name: string) => within(dlg).getByText(name).closest("li")!;
    expect(within(rowOf("Natural Wedding")).queryByRole("button", { name: "Eliminar" })).toBeNull();
    fireEvent.click(within(rowOf("Meu look")).getByRole("button", { name: "Renomear" }));
    const input = within(dlg).getByDisplayValue("Meu look");
    fireEvent.change(input, { target: { value: "Look Ana" } });
    fireEvent.blur(input);
    await act(async () => {});
    expect(apiMock.renamePreset).toHaveBeenCalledWith("u7", "Look Ana");
    fireEvent.click(within(rowOf("Natural Wedding")).getByRole("button", { name: "Duplicar" }));
    await act(async () => {});
    expect(apiMock.duplicatePreset).toHaveBeenCalledWith("builtin:natural-wedding", "(cópia)");
    fireEvent.click(within(dlg).getByRole("button", { name: "Descer Meu look" }));
    await act(async () => {});
    expect(apiMock.reorderPresets).toHaveBeenCalledWith(["u8", "u7"]);
    fireEvent.click(within(rowOf("Outro")).getByRole("button", { name: "Eliminar" }));
    fireEvent.click(within(screen.getByRole("dialog", { name: 'Eliminar o preset "Outro"?' })).getByRole("button", { name: "Eliminar" }));
    await act(async () => {});
    expect(apiMock.deletePreset).toHaveBeenCalledWith("u8");
    expect((within(dlg).getByRole("button", { name: "Exportar (0)" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("aplicar preset: seleção, momento e casamento completo", async () => {
    await openApp(5);
    act(() => useStore.getState().setSelection({ ids: new Set([1, 2]), focus: 1, anchor: 1 }));
    fireEvent.click(screen.getByRole("button", { name: "Presets…" }));
    const dlg = await screen.findByRole("dialog", { name: "Aplicar preset" });
    fireEvent.click(within(dlg).getByRole("button", { name: "B&W Classic" }));
    fireEvent.change(within(dlg).getByRole("slider", { name: "Intensidade (%)" }), { target: { value: "80" } });
    fireEvent.click(within(dlg).getByRole("button", { name: "Aplicar a 2 fotografias" }));
    await act(async () => {});
    expect(apiMock.applyPreset).toHaveBeenCalledWith([1, 2], "builtin:bw-classic", 80);
    fireEvent.click(screen.getByRole("button", { name: "Presets…" }));
    const dlg2 = await screen.findByRole("dialog", { name: "Aplicar preset" });
    fireEvent.click(within(dlg2).getByRole("radio", { name: /Casamento completo/ }));
    fireEvent.click(within(dlg2).getByRole("button", { name: "Natural Wedding" }));
    fireEvent.click(within(dlg2).getByRole("button", { name: "Aplicar a 5 fotografias" }));
    await act(async () => {});
    expect(apiMock.applyPreset).toHaveBeenLastCalledWith([1, 2, 3, 4, 5], "builtin:natural-wedding", 100);
  });

  it("consistência: grupos, referência, sincronizar e sincronização inteligente no editor", async () => {
    await openApp();
    fireEvent.click(screen.getByRole("button", { name: "Consistência" }));
    await act(async () => {});
    expect(apiMock.consistencyGroups).toHaveBeenCalledWith("combined");
    const groups = screen.getByRole("complementary", { name: "Grupos" });
    expect(within(groups).getByText("Luz forte (exterior)")).toBeTruthy();
    expect(within(groups).getByText(/22:00–23:10 · Com flash · Luz quente/)).toBeTruthy();
    fireEvent.change(within(groups).getByRole("combobox", { name: "Agrupar por" }), { target: { value: "camera" } });
    await act(async () => {});
    expect(apiMock.consistencyGroups).toHaveBeenLastCalledWith("camera");
    fireEvent.click(within(groups).getByRole("button", { name: /Interior escuro/ }));
    fireEvent.click(screen.getByRole("button", { name: "IMG_00003.jpg" }));
    expect(screen.getByRole("button", { name: "IMG_00003.jpg" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByRole("checkbox", { name: "Tons de pele" }));
    fireEvent.click(screen.getByRole("button", { name: "Sincronizar 1 fotografia com a referência" }));
    await act(async () => {});
    expect(apiMock.startSync).toHaveBeenCalledWith(3, [3, 4], expect.objectContaining({ skin: false, exposure: true, look: true }));
    expect(screen.getByRole("region", { name: "Sincronizar com referência" })).toBeTruthy();
    // Medir o que falta.
    apiMock.consistencyGroups.mockResolvedValueOnce({ groups: lookGroups(), missing: 3 });
    fireEvent.change(within(groups).getByRole("combobox", { name: "Agrupar por" }), { target: { value: "flash" } });
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Medir agora" }));
    await act(async () => {});
    expect(apiMock.measureConsistency).toHaveBeenCalled();
  });
});

describe("edição localizada (Fase 8)", () => {
  async function openDevelop() {
    vi.useRealTimers();
    apiMock.listPhotos.mockResolvedValue(rows(3).map((r) => ({ ...r, flag: 0 })));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    fireEvent.mouseDown(document.querySelector('[data-id="2"]')!);
    fireEvent.keyDown(window, { key: "d" });
    await act(async () => {});
    await waitFor(() => expect(apiMock.renderEdit).toHaveBeenCalled());
    const panel = screen.getByRole("complementary", { name: "Ajustes" });
    fireEvent.click(within(panel).getByRole("button", { name: /Máscaras/ }));
    return panel;
  }
  const lastCommit = () => apiMock.commitEdit.mock.calls.at(-1)! as [number, string, DevelopSettings];

  it("máscara de IA (pessoa), sobreposição, ajuste local, gradiente a subtrair, eliminar", async () => {
    const panel = await openDevelop();
    fireEvent.click(within(panel).getByRole("button", { name: "+ Nova máscara…" }));
    fireEvent.click(within(panel).getByRole("menuitem", { name: "Pessoa" }));
    await act(async () => {});
    expect(apiMock.createMaskRaster).toHaveBeenCalledWith(2, "person", null);
    let [, label, st] = lastCommit();
    expect(label).toBe("masks.add.person");
    expect(st.masks[0]!.components[0]).toEqual({ op: "add", invert: false, shape: { type: "raster", kind: "person", raster: "r-person", feather: 0 } });
    // A máscara selecionada aparece a vermelho no render (sobreposição).
    await waitFor(() => expect(apiMock.renderEdit.mock.calls.some((c) => c[6] === 0)).toBe(true));
    const detail = within(panel).getByRole("group", { name: "Máscara Pessoa" });
    fireEvent.change(within(detail).getByRole("slider", { name: "Exposição" }), { target: { value: "0.7" } });
    fireEvent.pointerUp(within(detail).getByRole("slider", { name: "Exposição" }), { target: { value: "0.7" } });
    await act(async () => {});
    [, label, st] = lastCommit();
    expect(label).toBe("masks.exposure");
    expect(st.masks[0]!.adjust.exposure).toBe(0.7);
    // Subtrair um gradiente linear desenhado na imagem.
    fireEvent.click(within(detail).getByRole("button", { name: "− Subtrair…" }));
    fireEvent.click(within(detail).getByRole("menuitem", { name: "Gradiente linear" }));
    await act(async () => {});
    const layer = screen.getByRole("application", { name: "Gradiente linear: arraste" });
    fireEvent.pointerDown(layer, { clientX: 0, clientY: 0, pointerId: 1 });
    fireEvent.pointerMove(layer, { clientX: 0, clientY: 0, pointerId: 1 });
    fireEvent.pointerUp(layer, { pointerId: 1 });
    await act(async () => {});
    expect(apiMock.mapPoints).toHaveBeenCalledWith(2, expect.anything(), expect.any(Array), true, false);
    [, label, st] = lastCommit();
    expect(label).toBe("masks.linear");
    expect(st.masks[0]!.components[1]).toMatchObject({ op: "subtract", shape: { type: "linear" } });
    expect(screen.queryByRole("application")).toBeNull();
    // Inverter e densidade, e eliminar.
    fireEvent.click(within(detail).getByRole("checkbox", { name: "Inverter a máscara" }));
    await act(async () => {});
    expect(lastCommit()[2].masks[0]!.invert).toBe(true);
    fireEvent.click(within(panel).getByRole("button", { name: "Eliminar a máscara Pessoa" }));
    await act(async () => {});
    expect(lastCommit()[2].masks).toEqual([]);
  });

  it("pincel: pintar e apagar com tamanho, suavidade e fluxo", async () => {
    const panel = await openDevelop();
    fireEvent.click(within(panel).getByRole("button", { name: "+ Nova máscara…" }));
    fireEvent.click(within(panel).getByRole("menuitem", { name: "Pincel" }));
    await act(async () => {});
    fireEvent.change(within(panel).getByRole("slider", { name: "Fluxo" }), { target: { value: "40" } });
    const layer = screen.getByRole("application", { name: "Pincel: arraste para pintar" });
    fireEvent.pointerDown(layer, { clientX: 1, clientY: 1, pointerId: 1 });
    fireEvent.pointerMove(layer, { clientX: 2, clientY: 1, pointerId: 1 });
    fireEvent.pointerMove(layer, { clientX: 3, clientY: 1, pointerId: 1 });
    fireEvent.pointerUp(layer, { pointerId: 1 });
    await act(async () => {});
    const [, label, st] = lastCommit();
    expect(label).toBe("masks.stroke");
    const brush = st.masks[0]!.components[0]!.shape as { strokes: { points: unknown[]; flow: number; erase: boolean }[] };
    expect(brush.strokes).toHaveLength(1);
    expect(brush.strokes[0]!.points).toHaveLength(3);
    expect(brush.strokes[0]!).toMatchObject({ flow: 40, erase: false });
    fireEvent.click(within(panel).getByRole("button", { name: "Apagar" }));
    const layer2 = screen.getByRole("application", { name: "Pincel: arraste para pintar" });
    fireEvent.pointerDown(layer2, { clientX: 1, clientY: 1, pointerId: 1 });
    fireEvent.pointerUp(layer2, { pointerId: 1 });
    await act(async () => {});
    const strokes = (lastCommit()[2].masks[0]!.components[0]!.shape as { strokes: { erase: boolean }[] }).strokes;
    expect(strokes.map((x) => x.erase)).toEqual([false, true]);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("application")).toBeNull();
  });

  it("retoque (corrigir/clonar/remover) e retrato (detetar, ajustar, remover)", async () => {
    const panel = await openDevelop();
    fireEvent.click(within(panel).getByRole("button", { name: /Retoque/ }));
    fireEvent.click(within(panel).getByRole("button", { name: "Corrigir" }));
    fireEvent.click(screen.getByRole("application", { name: "Clique na mancha a remover" }), { clientX: 0, clientY: 0 });
    await act(async () => {});
    expect(apiMock.suggestHealSource).toHaveBeenCalled();
    let [, label, st] = lastCommit();
    expect(label).toBe("retouch.heal");
    expect(st.retouch.spots[0]).toMatchObject({ heal: true, sx: 0.1, r: 0.03 });
    fireEvent.change(within(panel).getByRole("combobox", { name: "Modo da mancha 1" }), { target: { value: "clone" } });
    await act(async () => {});
    expect(lastCommit()[2].retouch.spots[0]!.heal).toBe(false);
    fireEvent.click(within(panel).getByRole("button", { name: "Remover a mancha 1" }));
    await act(async () => {});
    expect(lastCommit()[2].retouch.spots).toEqual([]);
    // Remover objeto: traço → origem sugerida → patch.
    fireEvent.click(within(panel).getByRole("button", { name: "Remover objeto" }));
    const layer = screen.getByRole("application", { name: "Pinte sobre o objeto a remover" });
    fireEvent.pointerDown(layer, { clientX: 0, clientY: 0, pointerId: 1 });
    fireEvent.pointerUp(layer, { pointerId: 1 });
    await act(async () => {});
    [, label, st] = lastCommit();
    expect(label).toBe("retouch.object");
    expect(st.retouch.patches[0]).toMatchObject({ heal: true, dx: 0.1, dy: 0 });
    // Retrato.
    fireEvent.click(within(panel).getByRole("button", { name: /Retrato/ }));
    fireEvent.click(within(panel).getByRole("button", { name: "Detetar rostos" }));
    await act(async () => {});
    expect(apiMock.detectPortrait).toHaveBeenCalledWith(2);
    expect(lastCommit()[2].portrait.faces).toHaveLength(1);
    expect(within(panel).getByText("1 rosto")).toBeTruthy();
    const skin = within(panel).getByRole("slider", { name: "Suavizar pele" });
    fireEvent.change(skin, { target: { value: "30" } });
    fireEvent.pointerUp(skin, { target: { value: "30" } });
    await act(async () => {});
    expect(lastCommit()[2].portrait.skin).toBe(30);
    fireEvent.click(within(panel).getByRole("button", { name: "Remover retrato" }));
    await act(async () => {});
    expect(lastCommit()[2].portrait).toEqual({ skin: 0, darkCircles: 0, shine: 0, teeth: 0, eyes: 0, faces: [] });
  });
});

describe("edição automática em lote (Fase 6)", () => {
  it("Editar automaticamente seleção: diálogo, início da fila, pausa, retoma e cancelar", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(5).map((r) => ({ ...r, flag: 0 })));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    expect((screen.getByRole("button", { name: /Auto \(0\)/ }) as HTMLButtonElement).disabled).toBe(true);
    act(() => useStore.getState().setSelection({ ids: new Set([1, 3, 4]), focus: 1, anchor: 1 }));
    fireEvent.click(screen.getByRole("button", { name: /Auto \(3\)/ }));
    const dlg = await screen.findByRole("dialog", { name: "Editar automaticamente 3 fotografias" });
    fireEvent.click(within(dlg).getByRole("button", { name: "Equilibrada" }));
    fireEvent.change(within(dlg).getByRole("combobox", { name: "AI Denoise" }), { target: { value: "whenNeeded" } });
    expect(within(dlg).getByText(/O AI Denoise é lento/)).toBeTruthy();
    fireEvent.click(within(dlg).getByRole("button", { name: "Editar 3 fotografias" }));
    await act(async () => {});
    expect(apiMock.startAutoEdit).toHaveBeenCalledWith([1, 3, 4], expect.objectContaining({ intensity: "balanced", aiDenoise: "whenNeeded" }));
    const queue = screen.getByRole("region", { name: "Edição automática" });
    expect(within(queue).getByText("0 de 3")).toBeTruthy();
    fireEvent.click(within(queue).getByRole("button", { name: /Pausar/ }));
    expect(apiMock.pauseAutoEdit).toHaveBeenCalled();
    // O núcleo envia o progresso (aqui simulado no estado).
    act(() => useStore.getState().setAutoProgress({ kind: "auto", total: 3, done: 1, failed: 0, paused: true, cancelled: false, finished: false, current: "IMG_00003.jpg" }));
    expect(within(queue).getByText("em pausa")).toBeTruthy();
    expect(within(queue).getByText("IMG_00003.jpg")).toBeTruthy();
    fireEvent.click(within(queue).getByRole("button", { name: /Retomar/ }));
    expect(apiMock.resumeAutoEdit).toHaveBeenCalled();
    fireEvent.click(within(queue).getByRole("button", { name: "Cancelar" }));
    expect(apiMock.cancelAutoEdit).toHaveBeenCalled();
    act(() => useStore.getState().setAutoProgress({ kind: "auto", total: 3, done: 1, failed: 0, paused: false, cancelled: true, finished: true, current: null }));
    expect(within(queue).getByText("cancelada")).toBeTruthy();
    fireEvent.click(within(queue).getByRole("button", { name: "Fechar" }));
    expect(screen.queryByRole("region", { name: "Edição automática" })).toBeNull();
  });
});

describe("exportação (Fase 9)", () => {
  async function openWith(n: number) {
    apiMock.listPhotos.mockResolvedValue(rows(n));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("diálogo: plano com várias receitas, formatos indisponíveis explicados, editor e início", async () => {
    const dialog = await import("@tauri-apps/plugin-dialog");
    (dialog.open as unknown as ReturnType<typeof vi.fn>).mockResolvedValueOnce("/tmp/entrega");
    await openWith(4);
    act(() => useStore.getState().setSelection({ ids: new Set([1, 2]), focus: 1, anchor: 1 }));
    fireEvent.click(screen.getByRole("button", { name: /Exportar…/ }));
    const dlg = await screen.findByRole("dialog", { name: "Exportar" });
    await act(async () => {});
    // Plano "Exportar casamento" com as duas receitas e as suas pastas.
    const list = within(dlg).getByRole("list", { name: "Receitas desta exportação" });
    expect(within(list).getByText("Galeria")).toBeTruthy();
    expect(within(list).getByText("/Arquivo")).toBeTruthy();
    // HEIC visível mas desativado (sem botões falsos).
    const fmt = within(dlg).getByRole("combobox", { name: "Formato" }) as HTMLSelectElement;
    const heic = Array.from(fmt.options).find((o) => o.value === "heic")!;
    expect(heic.disabled).toBe(true);
    // Mudar para AVIF: profundidades 8/10/12 e só espaços com CICP.
    fireEvent.change(fmt, { target: { value: "avif" } });
    const depth = within(dlg).getByRole("combobox", { name: "Profundidade" }) as HTMLSelectElement;
    expect(Array.from(depth.options).map((o) => o.value)).toEqual(["8", "10", "12"]);
    const space = within(dlg).getByRole("combobox", { name: "Espaço de cor" }) as HTMLSelectElement;
    expect(Array.from(space.options).map((o) => o.value)).toEqual(["srgb", "displayP3", "rec709", "rec2020"]);
    // Nome de exemplo segue o modelo.
    fireEvent.change(within(dlg).getByRole("textbox", { name: "Prefixo" }), { target: { value: "AP_" } });
    expect(within(dlg).getByTestId("export-example").textContent).toBe("Galeria/AP_IMG_0001.avif");
    // Marca de água: grelha de posições.
    fireEvent.click(within(dlg).getByRole("checkbox", { name: /Aplicar marca de água/ }));
    fireEvent.click(within(dlg).getByRole("radio", { name: "Canto superior esquerdo" }));
    // Destino e estimativa; exportar a seleção.
    fireEvent.click(within(dlg).getAllByRole("button", { name: "Escolher…" }).at(-1)!);
    await act(async () => {});
    await waitFor(() => expect(within(dlg).getByRole("status").textContent).toMatch(/6 ficheiros/));
    fireEvent.click(within(dlg).getByRole("button", { name: /Exportar 6 ficheiros/ }));
    await act(async () => {});
    const req = (apiMock.startExport.mock.calls as unknown as unknown[][])[0]![0] as { keys: number[]; recipes: { format: string; watermark: { anchor: string; enabled: boolean }; naming: { prefix: string } }[]; destination: string };
    expect(req.keys).toEqual([1, 2]);
    expect(req.destination).toBe("/tmp/entrega");
    expect(req.recipes).toHaveLength(2);
    expect(req.recipes[0]).toMatchObject({ format: "avif", naming: { prefix: "AP_" }, watermark: { enabled: true, anchor: "topLeft" } });
    expect(req.recipes[1]!.format).toBe("tiff");
  });

  it("fila: progresso, restantes, tamanho, espaço, tempo, erros, pausa e pasta", async () => {
    await openWith(2);
    const job = {
      id: 7,
      name: "Exportar casamento",
      destination: "/tmp/entrega",
      state: "running",
      pausedReason: null,
      photosTotal: 10,
      photosDone: 4,
      filesTotal: 50,
      filesDone: 19,
      filesFailed: 1,
      filesSkipped: 0,
      bytesWritten: 200 * 1024 * 1024,
      bytesEstimated: 500 * 1024 * 1024,
      freeBytes: 20 * 1024 * 1024 * 1024,
      elapsedSeconds: 40,
      etaSeconds: 60,
      current: "IMG_0005.CR3",
      errors: [{ file: "IMG_0003.jpg", message: "original não encontrado" }],
      notes: [],
      folders: [],
    };
    act(() => useStore.getState().upsertExportJob(job as never));
    const q = screen.getByRole("region", { name: "Exportação" });
    expect(within(q).getByText("20 de 50 ficheiros · faltam 6 fotografias")).toBeTruthy();
    expect(within(q).getByText(/200 MB de ≈ 500 MB · livre 20 GB · faltam 1 min 00 s/)).toBeTruthy();
    fireEvent.click(within(q).getByRole("button", { name: "1 erro" }));
    expect(within(q).getByText(/original não encontrado/)).toBeTruthy();
    fireEvent.click(within(q).getByRole("button", { name: /Pausa/ }));
    expect(apiMock.pauseExport).toHaveBeenCalledWith(7, true);
    act(() => useStore.getState().upsertExportJob({ ...job, state: "paused", pausedReason: "noSpace" } as never));
    expect(within(q).getByText(/espaço insuficiente/)).toBeTruthy();
    fireEvent.click(within(q).getByRole("button", { name: /Retomar/ }));
    expect(apiMock.pauseExport).toHaveBeenCalledWith(7, false);
    act(() => useStore.getState().upsertExportJob({ ...job, state: "done", photosDone: 10, filesDone: 49 } as never));
    fireEvent.click(within(q).getByRole("button", { name: "Mostrar pasta" }));
    expect(apiMock.revealFolder).toHaveBeenCalledWith("/tmp/entrega");
  });

  it("lógica: nomes como o núcleo, ajuste ao formato e duração", () => {
    const r = newRecipe("x");
    expect(sanitize("CON")).toBe("_CON");
    expect(sanitize("  ponto. ")).toBe("ponto");
    const v = { stem: "IMG_1", seq: 7, date: "2026-06-15", time: "1742", project: "P", client: "Ana: Pedro", recipe: "R", moment: "Cerimónia", width: 1, height: 1 };
    expect(examplePath({ ...r, naming: { ...r.naming, template: "{data}_{seq}_{nome}", prefix: "AP_", suffix: "_web" } }, "jpg", v)).toBe("AP_2026-06-15_0007_IMG_1_web.jpg");
    expect(examplePath({ ...r, naming: { ...r.naming, subfolder: "Entrega/../{momento}//Alta" } }, "tif", v)).toBe("Entrega/Cerimónia/Alta/IMG_1.tif");
    const avif = EXPORT_CAPS[3] as never;
    expect(fitToFormat({ ...r, color: "adobeRgb", depth: "16" }, avif, ["srgb", "displayP3", "adobeRgb", "proPhoto", "rec709", "rec2020", "icc"])).toMatchObject({ format: "avif", color: "srgb", depth: "8" });
    expect(formatDuration(3725)).toBe("1 h 02 min");
    expect(formatDuration(65)).toBe("1 min 05 s");
  });
});

describe("galeria (Fase 10)", () => {
  it("diálogo: conteúdo, ocultar, tema, PIN, publicação indisponível e gerar com confirmação", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(4));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    act(() => useStore.getState().setSelection({ ids: new Set([2, 3]), focus: 2, anchor: 2 }));
    fireEvent.click(screen.getByRole("button", { name: "Galeria…" }));
    const dlg = await screen.findByRole("dialog", { name: "Galeria do cliente" });
    await act(async () => {});
    expect((within(dlg).getByRole("textbox", { name: "Noivos (título)" }) as HTMLInputElement).value).toBe("Ana & Pedro");
    await waitFor(() => expect(within(dlg).getByRole("list", { name: "Capítulos da galeria" }).textContent).toContain("Cerimónia"));
    fireEvent.click(within(dlg).getByRole("button", { name: "Ocultar seleção (2)" }));
    await act(async () => {});
    expect(apiMock.setGalleryHidden).toHaveBeenCalledWith([2, 3], true);
    expect(within(dlg).getByText("2 fotografias ocultas")).toBeTruthy();
    // Aparência: tema editorial e hero da fotografia em foco.
    fireEvent.click(within(dlg).getByRole("tab", { name: "Aparência" }));
    fireEvent.click(within(dlg).getByRole("radio", { name: /Editorial/ }));
    fireEvent.click(within(dlg).getAllByRole("button", { name: "Usar a fotografia em foco" })[0]!);
    // Funcionalidades: PIN só com dígitos.
    fireEvent.click(within(dlg).getByRole("tab", { name: "Funcionalidades" }));
    fireEvent.change(within(dlg).getByRole("combobox", { name: "Proteção" }), { target: { value: "pin" } });
    fireEvent.change(within(dlg).getByRole("textbox", { name: "PIN (4–8 dígitos)" }), { target: { value: "24a68" } });
    expect((within(dlg).getByRole("textbox", { name: "PIN (4–8 dígitos)" }) as HTMLInputElement).value).toBe("2468");
    expect(within(dlg).getByText(/barreira ligeira/)).toBeTruthy();
    // Publicar: a publicação direta não existe e a interface di-lo em texto (sem "Fase futura").
    fireEvent.click(within(dlg).getByRole("tab", { name: "Publicar" }));
    expect(within(dlg).getByText(/não está disponível nesta versão/)).toBeTruthy();
    expect(within(dlg).queryByText("Fase futura")).toBeNull();
    expect(within(dlg).queryByRole("button", { name: /R2|S3|SFTP|B2/ })).toBeNull();
    fireEvent.change(within(dlg).getByRole("textbox", { name: "Favoritas do cliente" }), { target: { value: "IMG_0001\nx" } });
    fireEvent.click(within(dlg).getByRole("button", { name: "Marcar favoritas" }));
    await act(async () => {});
    expect(within(dlg).getByText(/2 fotografias marcadas/)).toBeTruthy();
    // Gerar: a pasta já tem uma galeria → confirmar substituição.
    await waitFor(() => expect(within(dlg).getByRole("button", { name: "Gerar galeria (4)" })).toBeTruthy());
    fireEvent.click(within(dlg).getByRole("button", { name: "Gerar galeria (4)" }));
    expect(within(dlg).getByText(/Substituir\?/)).toBeTruthy();
    fireEvent.click(within(dlg).getByRole("button", { name: "Substituir" }));
    await act(async () => {});
    const call = apiMock.startGallery.mock.calls.at(-1) as unknown as [{ settings: typeof GALLERY_SETTINGS }, unknown, boolean];
    expect(call[2]).toBe(true);
    expect(call[0].settings).toMatchObject({ theme: "editorial", hero: 2, protection: "pin", secret: "2468" });
  });

  it("fila: galeria concluída oferece abri-la", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(2));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    act(() =>
      useStore.getState().upsertExportJob({
        id: 9, name: "Galeria", destination: "/tmp/pub/g", state: "done", pausedReason: null, photosTotal: 2, photosDone: 2, filesTotal: 6, filesDone: 6,
        filesFailed: 0, filesSkipped: 0, bytesWritten: 1e6, bytesEstimated: 1e6, freeBytes: 1e9, elapsedSeconds: 3, etaSeconds: 0, current: null,
        errors: [], notes: [], folders: [], index: "/tmp/pub/g/index.html",
      } as never),
    );
    fireEvent.click(screen.getByRole("button", { name: "Abrir galeria" }));
    expect(apiMock.openGallery).toHaveBeenCalledWith("/tmp/pub/g/index.html");
  });
});

describe("álbuns (Fase 11)", () => {
  async function openAlbum() {
    apiMock.listPhotos.mockResolvedValue(rows(4));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Álbum" }));
    await waitFor(() => expect((screen.getByRole("textbox", { name: "Nome" }) as HTMLInputElement).value).toBe("Álbum Ana & Pedro"));
    await act(async () => {});
  }

  it("editor: alertas, layout, arrastar e largar, automático e preset de laboratório", async () => {
    await openAlbum();
    // Alertas de DPI e especificação por confirmar; exportar bloqueado com erros.
    expect(screen.getByText("Spread 1: Resolução efetiva baixa: 120 DPI.")).toBeTruthy();
    expect(screen.getByText(/Não existem valores universais/)).toBeTruthy();
    expect(screen.getByText("1 erro por corrigir")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Exportar álbum" }) as HTMLButtonElement).disabled).toBe(true);
    expect(document.querySelector('[data-slot="s1"]')!.className).toContain("has-error");
    // Layouts para 1 fotografia; aplicar a panorâmica.
    fireEvent.click(await screen.findByRole("button", { name: "Aplicar layout P" }));
    await act(async () => {});
    expect(apiMock.applyAlbumLayout).toHaveBeenCalledWith(expect.objectContaining({ id: "a1" }), 0, "P", 12, 4, []);
    // Arrastar a fotografia 2 da tira para a moldura vazia.
    const slot2 = document.querySelector<HTMLElement>('[data-slot="s2"]')!;
    const efp = vi.fn(() => slot2);
    Object.defineProperty(document, "elementFromPoint", { value: efp, configurable: true });
    const item = screen.getAllByRole("button", { name: /IMG_0000?2/ })[0]!;
    fireEvent.pointerDown(item, { clientX: 10, clientY: 10 });
    fireEvent.pointerMove(document.querySelector(".album-view")!, { clientX: 300, clientY: 200 });
    expect(document.querySelector(".album-ghost")).toBeTruthy();
    fireEvent.pointerUp(document.querySelector(".album-view")!, { clientX: 300, clientY: 200 });
    expect(efp).toHaveBeenCalled();
    await waitFor(() => expect(apiMock.saveAlbum).toHaveBeenCalledWith(expect.objectContaining({ spreads: expect.arrayContaining([expect.objectContaining({ slots: expect.arrayContaining([expect.objectContaining({ id: "s2", photo: 2 })]) })]) })));
    // A fotografia 2 tem um rosto: o recorte centra-se nele.
    const saved = apiMock.saveAlbum.mock.calls.at(-1) as unknown as [{ spreads: { slots: { id: string; crop: { cx: number } }[] }[] }];
    expect(saved[0].spreads[0]!.slots.find((x) => x.id === "s2")!.crop.cx).toBeCloseTo(0.45);
    // Seleção + teclado: mover 10 mm e apagar.
    fireEvent.pointerDown(slot2);
    fireEvent.pointerUp(document.querySelector(".album-view")!);
    fireEvent.keyDown(screen.getByLabelText("Spread", { selector: ".album-stage" }), { key: "ArrowRight", shiftKey: true });
    await waitFor(() => {
      const c = apiMock.saveAlbum.mock.calls.at(-1) as unknown as [{ spreads: { slots: { id: string; x: number }[] }[] }];
      expect(c[0].spreads[0]!.slots.find((x) => x.id === "s2")!.x).toBe(340);
    });
    // As teclas do editor não classificam fotografias da grelha (invisível).
    fireEvent.keyDown(window, { key: "5" });
    fireEvent.keyDown(window, { key: "x" });
    expect(apiMock.setStars).not.toHaveBeenCalled();
    expect(apiMock.setFlag).not.toHaveBeenCalled();
    // Automático com confirmação (já há fotografias no álbum).
    fireEvent.click(screen.getByRole("button", { name: /Criar álbum automaticamente/ }));
    expect(screen.getByText("Isto substitui as fotografias de todos os spreads.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Substituir" }));
    await act(async () => {});
    expect(apiMock.autoAlbum).toHaveBeenCalledWith("a1", { maxPerPage: 3, marginMm: 12, spacingMm: 4, keepPages: true });
    expect(screen.getByText(/Usadas 3 de 4 fotografias/)).toBeTruthy();
    expect(screen.getByText("Abertura (1) → Cerimónia (1)")).toBeTruthy();
    // Especificação: aplicar o preset do laboratório (nada de valores universais).
    fireEvent.click(screen.getByRole("tab", { name: "Especificação" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Preset de laboratório" }), { target: { value: "l1" } });
    await waitFor(() =>
      expect(apiMock.saveAlbum).toHaveBeenCalledWith(expect.objectContaining({ spec: expect.objectContaining({ lab: "Lab Porto", bleedMm: 5, safeMm: 15, gutterMm: 20, exportUnit: "pages", confirmed: true }) })),
    );
    expect((screen.getByRole("checkbox", { name: /Medidas confirmadas/ }) as HTMLInputElement).checked).toBe(true);
    // Formato 30x40 (horizontal) e personalizado.
    fireEvent.change(screen.getByRole("combobox", { name: "Formato" }), { target: { value: "30x40" } });
    expect((screen.getByRole("spinbutton", { name: "Largura da página (cm)" }) as HTMLInputElement).value).toBe("40");
    fireEvent.change(screen.getByRole("spinbutton", { name: "Largura da página (cm)" }), { target: { value: "27.5" } });
    expect((screen.getByRole("combobox", { name: "Formato" }) as HTMLSelectElement).value).toBe("custom");
    fireEvent.change(screen.getByRole("spinbutton", { name: "Páginas" }), { target: { value: "9" } });
    expect((screen.getByRole("spinbutton", { name: "Páginas" }) as HTMLInputElement).value).toBe("10");
  });

  it("fila: álbum concluído oferece abrir o PDF", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(2));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    act(() =>
      useStore.getState().upsertExportJob({
        id: 21, name: "Álbum", destination: "/tmp/lab", state: "done", pausedReason: null, photosTotal: 10, photosDone: 10, filesTotal: 10, filesDone: 10,
        filesFailed: 0, filesSkipped: 0, bytesWritten: 5e6, bytesEstimated: 5e6, freeBytes: 1e9, elapsedSeconds: 3, etaSeconds: 0, current: null,
        errors: [], notes: [], folders: [], index: "/tmp/lab/album-impressao.pdf",
      } as never),
    );
    expect(screen.queryByRole("button", { name: "Abrir galeria" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Abrir PDF" }));
    expect(apiMock.openGallery).toHaveBeenCalledWith("/tmp/lab/album-impressao.pdf");
  });

  it("exportar o PDF de prova para a pasta escolhida", async () => {
    apiMock.albumPreflight.mockResolvedValue([{ spread: null, slot: null, severity: "warning", code: "album.issue.unconfirmed", value: null }]);
    const dialog = await import("@tauri-apps/plugin-dialog");
    vi.mocked(dialog.open).mockResolvedValueOnce("/tmp/lab");
    await openAlbum();
    expect(screen.getByText("Sem erros. Reveja os avisos antes de enviar.")).toBeTruthy();
    fireEvent.click(screen.getAllByRole("button", { name: "Escolher…" }).at(-1)!);
    await waitFor(() => expect((screen.getByRole("textbox", { name: "Pasta de destino" }) as HTMLInputElement).value).toBe("/tmp/lab"));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportação" }), { target: { value: "proofPdf" } });
    fireEvent.click(screen.getByRole("button", { name: "Exportar álbum" }));
    await act(async () => {});
    expect(apiMock.startAlbumExport).toHaveBeenCalledWith("a1", "proofPdf", "/tmp/lab");
  });
});

describe("segurança (Fase 13)", () => {
  async function openProj(n = 4) {
    apiMock.listPhotos.mockResolvedValue(rows(n).map((r, i) => (i === 1 ? { ...r, availability: "offline" as const } : r)));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("Delete pede uma escolha explícita; remover só do catálogo nunca apaga ficheiros", async () => {
    await openProj();
    fireEvent.mouseDown(document.querySelector('[data-id="3"]')!);
    fireEvent.keyDown(window, { key: "Delete" });
    const dlg = await screen.findByRole("dialog", { name: "Remover 1 fotografia" });
    const go = () => within(dlg).getAllByRole("button").at(-1) as HTMLButtonElement;
    expect(go().disabled).toBe(true);
    // Enter não faz nada sem escolha (o foco está numa opção, não num botão de ação).
    fireEvent.keyDown(document.activeElement!, { key: "Enter" });
    expect(apiMock.removeFromCatalog).not.toHaveBeenCalled();
    // Lixo: exige a confirmação adicional.
    fireEvent.click(within(dlg).getByRole("radio", { name: /Mover os originais para o lixo/ }));
    expect(go().disabled).toBe(true);
    fireEvent.click(within(dlg).getByRole("checkbox", { name: /Compreendo/ }));
    expect(go().disabled).toBe(false);
    // Muda de ideias: só do catálogo.
    fireEvent.click(within(dlg).getByRole("radio", { name: /Remover só do catálogo/ }));
    fireEvent.click(go());
    await act(async () => {});
    expect(apiMock.removeFromCatalog).toHaveBeenCalledWith([3], true);
    expect(apiMock.moveToTrash).not.toHaveBeenCalled();
  });

  it("coleção Indisponíveis, distintivo offline e localizar o original", async () => {
    await openProj();
    expect(screen.getByRole("button", { name: /Indisponíveis\s*1/ })).toBeTruthy();
    expect(screen.getByLabelText("Original offline (disco desligado)")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Indisponíveis/ }));
    await act(async () => {});
    expect(document.querySelectorAll(".cell").length).toBe(1);
  });

  it("diálogo de segurança: versões, restauro com confirmação, agendamento e retomar exportação", async () => {
    apiMock.safetyStatus.mockResolvedValue({
      ...safetyFixture(),
      report: { ...safetyFixture().report, previousUnclean: true, validatedFull: true, validationMs: 40 },
      pendingExports: [{ id: "17-3", name: "Exportar casamento", destination: "/tmp/e", done: 40, total: 120 }],
    });
    await openProj();
    // Exportação interrompida → o diálogo abre sozinho; sessão interrompida explicada.
    const dlg = await screen.findByRole("dialog", { name: "Segurança e cópias" });
    expect(screen.getByText(/A sessão anterior não terminou corretamente/)).toBeTruthy();
    expect(within(dlg).getByText(/verificado por completo em 40 ms/)).toBeTruthy();
    expect(within(dlg).getByText("Exportar casamento: 40 de 120 fotografias exportadas")).toBeTruthy();
    fireEvent.click(within(dlg).getByRole("button", { name: "Retomar" }));
    await act(async () => {});
    expect(apiMock.resumeExport).toHaveBeenCalledWith("17-3");
    // Versões.
    fireEvent.click(within(dlg).getByRole("tab", { name: "Versões" }));
    fireEvent.change(within(dlg).getByRole("combobox", { name: "Cópia automática" }), { target: { value: "15" } });
    await act(async () => {});
    expect(apiMock.setBackupMinutes).toHaveBeenCalledWith(15);
    fireEvent.click(within(dlg).getByRole("button", { name: "Criar versão agora" }));
    await act(async () => {});
    expect(apiMock.createBackup).toHaveBeenCalled();
    // O catálogo danificado guardado não pode ser "restaurado"; a versão automática sim, com confirmação.
    expect(within(dlg).getAllByRole("button", { name: "Restaurar" })).toHaveLength(1);
    fireEvent.click(within(dlg).getByRole("button", { name: "Restaurar" }));
    expect(apiMock.restoreBackup).not.toHaveBeenCalled();
    const confirm = within(dlg).getByRole("alertdialog", { name: "Confirmar" });
    fireEvent.click(within(confirm).getByRole("button", { name: "Restaurar" }));
    await act(async () => {});
    expect(apiMock.restoreBackup).toHaveBeenCalledWith("/p/backups/catalog-1794290000.db");
    apiMock.safetyStatus.mockResolvedValue(safetyFixture());
  });

  it("importação com verificação e cópia para o disco de segurança", async () => {
    const dialog = await import("@tauri-apps/plugin-dialog");
    await openProj();
    act(() => useStore.getState().setPendingImport({ paths: ["/cartao/DCIM"], hasFolders: true }));
    const dlg = await screen.findByRole("dialog", { name: /Importar/ });
    vi.mocked(dialog.open).mockResolvedValueOnce("/Volumes/Backup");
    fireEvent.click(within(dlg).getByRole("checkbox", { name: /Copiar também para um disco de segurança/ }));
    await act(async () => {});
    expect(within(dlg).getByText("Para: /Volumes/Backup")).toBeTruthy();
    fireEvent.click(within(dlg).getByRole("button", { name: /Importar$|Iniciar|Começar/ }));
    await act(async () => {});
    expect(apiMock.startImport).toHaveBeenCalledWith(["/cartao/DCIM"], "reference", true, { verify: true, backupDir: "/Volumes/Backup" });
  });
});

describe("UX profissional (Fase 14)", () => {
  async function openProj(n = 6) {
    apiMock.listPhotos.mockResolvedValue(rows(n));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("menu contextual: clique direito e teclado (Shift+F10), ações reais com atalhos visíveis", async () => {
    await openProj();
    fireEvent.contextMenu(document.querySelector('[data-id="2"]')!, { clientX: 100, clientY: 100 });
    const menu = screen.getByRole("menu", { name: "Ações da fotografia" });
    // Atalhos visíveis, discretos, à direita de cada ação.
    expect(within(menu).getByRole("menuitem", { name: /Marcar como favorita\s*P/ })).toBeTruthy();
    expect(document.activeElement).toBe(within(menu).getAllByRole("menuitem")[0]);
    // Teclado: ↓ ↓ ↓ ↓ chega a "Marcar como favorita"; Enter executa a ação real.
    for (let i = 0; i < 3; i++) fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(document.activeElement?.textContent).toContain("Marcar como favorita");
    fireEvent.click(document.activeElement!);
    await act(async () => {});
    expect(apiMock.setFlag).toHaveBeenCalledWith([2], 1);
    expect(screen.queryByRole("menu")).toBeNull();
    // Shift+F10 abre para a fotografia com foco; Esc fecha.
    fireEvent.keyDown(window, { key: "F10", shiftKey: true });
    expect(screen.getByRole("menu", { name: "Ações da fotografia" })).toBeTruthy();
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    // Letra salta para o item seguinte com essa inicial: "r" → Revelar, "r" → Rejeitar.
    fireEvent.contextMenu(document.querySelector('[data-id="3"]')!, { clientX: 10, clientY: 10 });
    fireEvent.keyDown(document.activeElement!, { key: "r" });
    expect(document.activeElement?.textContent).toContain("Revelar");
    fireEvent.keyDown(document.activeElement!, { key: "r" });
    expect(document.activeElement?.textContent).toContain("Rejeitar");
  });

  it("copiar a edição de uma fotografia e colar em várias, na grelha (menu e Ctrl+Shift+C/V)", async () => {
    await openProj();
    act(() => useStore.getState().setClipboard(null));
    act(() => useStore.getState().setSelection({ ids: new Set([2]), focus: 2, anchor: 2 }));
    fireEvent.contextMenu(document.querySelector('[data-id="2"]')!, { clientX: 100, clientY: 100 });
    const menu = screen.getByRole("menu", { name: "Ações da fotografia" });
    expect((within(menu).getByRole("menuitem", { name: /Colar edição/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(within(menu).getByRole("menuitem", { name: /Copiar edição/ }));
    await act(async () => {});
    expect(apiMock.editView).toHaveBeenCalledWith(2);
    const clip = useStore.getState().clipboard!;
    expect(clip.blocks).not.toContain("geometry");
    // Seleciona três fotografias e cola com o atalho: aplica a todas.
    act(() => useStore.getState().setSelection({ ids: new Set([3, 4, 5]), focus: 3, anchor: 3 }));
    fireEvent.keyDown(window, { key: "V", ctrlKey: true, shiftKey: true });
    await act(async () => {});
    expect(apiMock.pasteSettings).toHaveBeenLastCalledWith([3, 4, 5], clip.settings, clip.blocks);
    fireEvent.contextMenu(document.querySelector('[data-id="4"]')!, { clientX: 100, clientY: 100 });
    expect(within(screen.getByRole("menu", { name: "Ações da fotografia" })).getByRole("menuitem", { name: /Colar edição em 3 fotografias/ })).toBeTruthy();
  });

  it("barra numa linha: importar e mais num menu; tema escuro por omissão e claro opcional", async () => {
    await openProj();
    expect(document.documentElement.dataset.theme ?? "dark").toBe("dark");
    fireEvent.click(screen.getByRole("button", { name: "Mais" }));
    const more = screen.getByRole("menu", { name: "Mais" });
    expect(within(more).getByRole("menuitemcheckbox", { name: /Escuro/ }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(within(more).getByRole("menuitemcheckbox", { name: /Claro/ }));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.style.getPropertyValue("--bg")).toBe("#f5f4f2");
    fireEvent.click(screen.getByRole("button", { name: "Mais" }));
    fireEvent.click(within(screen.getByRole("menu", { name: "Mais" })).getByRole("menuitemcheckbox", { name: /Escuro/ }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    const dialog = await import("@tauri-apps/plugin-dialog");
    fireEvent.click(screen.getByRole("button", { name: "Importar" }));
    fireEvent.click(within(screen.getByRole("menu", { name: "Importar" })).getByRole("menuitem", { name: /Pasta…/ }));
    await act(async () => {});
    expect(vi.mocked(dialog.open)).toHaveBeenCalledWith(expect.objectContaining({ directory: true }));
  });

  it("tira de fotografias virtualizada no visualizador: 3000 fotografias, poucos elementos, atual marcada", async () => {
    await openProj(3000);
    fireEvent.doubleClick(document.querySelector('[data-id="2"]')!);
    const strip = await screen.findByRole("list", { name: "Tira de fotografias (3000)" });
    expect(strip.querySelectorAll(".strip-cell").length).toBeLessThan(60);
    expect(within(strip).getByRole("button", { name: "IMG_00002.jpg" }).getAttribute("aria-current")).toBe("true");
    fireEvent.click(within(strip).getByRole("button", { name: "IMG_00003.jpg" }));
    await act(async () => {});
    expect(within(strip).getByRole("button", { name: "IMG_00003.jpg" }).getAttribute("aria-current")).toBe("true");
  });
});

describe("estados claros (Fase 14)", () => {
  it("barra de estado: offline e em falta visíveis, com estado e anúncio acessível", async () => {
    const list = rows(5).map((r, i) => ({ ...r, availability: (i < 2 ? "offline" : i === 2 ? "missing" : "ok") as PhotoRow["availability"] }));
    apiMock.listPhotos.mockResolvedValue(list);
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    const status = document.querySelector(".status-states")!;
    expect(status.getAttribute("role")).toBe("status");
    expect(within(status as HTMLElement).getByText("2 offline (disco ausente)").className).toContain("state offline");
    expect(within(status as HTMLElement).getByText("1 em falta").className).toContain("state failed");
  });
});

describe("smoke test (Fase 16)", () => {
  it("miniatura concluída enquanto a lista é recarregada não fica 'pendente'", async () => {
    let resolve: (v: PhotoRow[]) => void = () => {};
    apiMock.listPhotos.mockImplementationOnce(() => new Promise<PhotoRow[]>((r) => (resolve = r)));
    const p = useStore.getState().reloadPhotos();
    // O núcleo gravou a miniatura e avisou ANTES de a lista (lida antes) chegar.
    act(() => useStore.getState().applyThumbBatch({ ready: [2], failed: [], remaining: 0, done: 1, total: 1 } as never));
    resolve(rows(3).map((r) => ({ ...r, thumb: "pending" as const })));
    await act(async () => {
      await p;
    });
    expect(useStore.getState().photos.find((x) => x.id === 2)!.thumb).toBe("ready");
    expect(useStore.getState().photos.find((x) => x.id === 1)!.thumb).toBe("pending");
  });
});

describe("culling seguro (Fase 15)", () => {
  async function openProj(n = 6) {
    apiMock.listPhotos.mockResolvedValue(rows(n).map((r) => ({ ...r, flag: 0, stars: 0 })));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("no editor, as teclas classificam a fotografia mostrada (não uma escondida pelo filtro)", async () => {
    await openProj();
    // Seleção numa fotografia que o filtro esconde (ex.: escolhida noutra vista).
    act(() => useStore.getState().setSelection({ ids: new Set([999]), focus: 999, anchor: 999 }));
    fireEvent.keyDown(window, { key: "d" });
    await act(async () => {});
    apiMock.setFlag.mockClear();
    fireEvent.keyDown(window, { key: "x" });
    await act(async () => {});
    expect(apiMock.setFlag).toHaveBeenCalledTimes(1);
    const keys = (apiMock.setFlag.mock.calls[0] as unknown[])[0] as number[];
    expect(keys).not.toContain(999);
  });

  it("clique direito numa seleção múltipla: o menu age sobre todas", async () => {
    await openProj();
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    fireEvent.mouseDown(document.querySelector('[data-id="3"]')!, { shiftKey: true });
    expect(useStore.getState().selection.ids.size).toBe(3);
    fireEvent.mouseDown(document.querySelector('[data-id="2"]')!, { button: 2 });
    fireEvent.contextMenu(document.querySelector('[data-id="2"]')!, { clientX: 50, clientY: 50 });
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: /Rejeitar/ }));
    await act(async () => {});
    expect(apiMock.setFlag).toHaveBeenCalledWith([1, 2, 3], -1);
  });

  it("em Pessoas, classificar/apagar não age sobre a seleção invisível da grelha", async () => {
    await openProj();
    fireEvent.mouseDown(document.querySelector('[data-id="2"]')!);
    fireEvent.click(screen.getByRole("button", { name: "Pessoas" }));
    await act(async () => {});
    apiMock.setFlag.mockClear();
    apiMock.setStars.mockClear();
    for (const key of ["x", "p", "5", "Delete"]) fireEvent.keyDown(window, { key });
    await act(async () => {});
    expect(apiMock.setFlag).not.toHaveBeenCalled();
    expect(apiMock.setStars).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("acessibilidade (Fase 14)", () => {
  // Auditoria axe-core (regras WCAG 2.x A/AA) aos ecrãs principais. O contraste é verificado
  // à parte, nos tokens (o jsdom não calcula cores nem layout).
  async function audit(label: string) {
    const r = await axe.run(document.body, {
      runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"] },
      rules: { "color-contrast": { enabled: false } },
    });
    const v = r.violations.map((x) => `${label}: ${x.id} (${x.impact}) — ${x.nodes.slice(0, 3).map((n) => n.target.join(" ")).join(" | ")}`);
    expect(v).toEqual([]);
  }
  async function openProj(n = 6) {
    apiMock.listPhotos.mockResolvedValue(rows(n));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("ecrã inicial sem violações", async () => {
    render(<App />);
    await screen.findByRole("button", { name: /Novo projeto/ });
    await act(async () => {});
    await audit("início");
  });

  it("grelha, menu contextual, visualizador com tira, comparação e revelação sem violações", async () => {
    await openProj();
    await audit("grelha");
    fireEvent.contextMenu(document.querySelector('[data-id="2"]')!, { clientX: 100, clientY: 100 });
    expect(screen.getByRole("menu")).toBeTruthy();
    await audit("menu contextual");
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    act(() => useStore.getState().openViewer(2));
    await act(async () => {});
    expect(document.querySelector(".filmstrip")).toBeTruthy();
    await audit("visualizador");
    act(() => useStore.getState().openViewer(null));
    fireEvent.click(screen.getByRole("button", { name: "Revelar" }));
    await act(async () => {});
    expect(document.querySelector(".dev-histogram")).toBeTruthy();
    await audit("revelação");
  });

  it("comparação, pessoas, álbum, ajuda e diálogos (exportar, galeria) sem violações", async () => {
    await openProj();
    act(() => useStore.getState().setSelection({ ids: new Set([1, 2, 3]), focus: 1, anchor: 1 }));
    fireEvent.keyDown(window, { key: "c" });
    await act(async () => {});
    expect(screen.getByRole("region", { name: "Comparação" })).toBeTruthy();
    await audit("comparação");
    fireEvent.keyDown(window, { key: "Escape" });
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Pessoas" }));
    await act(async () => {});
    await audit("pessoas");
    fireEvent.click(screen.getByRole("button", { name: "Álbum" }));
    await act(async () => {});
    expect(document.querySelector(".album-stage, .album-empty")).toBeTruthy();
    await audit("álbum");
    fireEvent.click(screen.getByRole("button", { name: "Grelha" }));
    await act(async () => {});
    fireEvent.keyDown(window, { key: "?" });
    await act(async () => {});
    expect(screen.getByRole("dialog", { name: /Atalhos/ })).toBeTruthy();
    await audit("ajuda");
    fireEvent.keyDown(window, { key: "Escape" });
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: /Exportar…/ }));
    await screen.findByRole("dialog", { name: "Exportar" });
    await act(async () => {});
    await audit("exportar");
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Galeria…" }));
    await screen.findByRole("dialog", { name: "Galeria do cliente" });
    await act(async () => {});
    await audit("galeria");
  });
});

describe("cálculos de layout", () => {
  it("desvios de relógio", () => {
    expect(parseOffset("+3min42s")).toBe(222000);
    expect(parseOffset("+3m42s")).toBe(222000);
    expect(parseOffset("-1min15s")).toBe(-75000);
    expect(parseOffset("+00:03:42")).toBe(222000);
    expect(parseOffset("-1:15")).toBe(-75000);
    expect(parseOffset("+1h2m")).toBe(3720000);
    expect(parseOffset("0")).toBe(0);
    expect(parseOffset("abc")).toBeNull();
    expect(formatOffset(222000)).toBe("+3min42s");
    expect(formatOffset(-75000)).toBe("-1min15s");
    expect(formatOffset(3600000)).toBe("+1h");
    expect(formatOffset(0)).toBe("0");
  });
  it("recorte do rosto a partir da miniatura", () => {
    // Rosto de 10 % no centro de uma fotografia 6000×4000, avatar de 60 px.
    const c = faceCrop({ faceId: 1, asset: 1, x: 0.45, y: 0.45, w: 0.1, h: 0.1, source: 0 }, 6000, 4000, 60);
        const nums = (v: string) => v.split(" ").map((x) => parseFloat(x));
    const [sw, sh] = nums(c.backgroundSize); // lado = 600 × 1,5 = 900 px → escala 60/900
    expect(sw).toBeCloseTo(400);
    expect(sh).toBeCloseTo(266.667, 2);
    const [px, py] = nums(c.backgroundPosition);
    expect(px).toBeCloseTo(-170);
    expect(py).toBeCloseTo(-103.333, 2);
  });
  it("explicações em linguagem natural", () => {
    const t = i18n.t.bind(i18n) as (k: string, o?: Record<string, unknown>) => string;
    expect(joinList(["a", "b", "c"], "e")).toBe("a, b e c");
    expect(explain("rejectSuggested", [{ code: "eyesClosed", value: 2 }, { code: "tilted", value: -3.44 }], t, "pt-PT")).toBe("Rejeição sugerida: olhos fechados em 2 pessoas e inclinada -3,4°.");
    expect(explain("goodSingle", [], t, "pt-PT")).toBe("Boa fotografia.");
  });
  it("comparação sincronizada", () => {
    expect(fitCss(6000, 4000, 600, 600)).toBeCloseTo(0.1);
    expect(clampCenter(0.9, 300, 600)).toBe(0.5); // imagem menor que o painel: centrada
    expect(clampCenter(0.99, 1200, 600)).toBeCloseTo(0.75);
    // O ponto sob o cursor mantém-se: zoom 1→2 a 100 px à direita do centro (disp 600).
    const v = zoomAt(FIT_VIEW, 2, 100, 0, 600, 400);
    const before = 0.5 + 100 / 600;
    expect(v.cx + 100 / 1200).toBeCloseTo(before);
    expect(zoomAt(v, 0.5, 0, 0, 1200, 800)).toEqual(FIT_VIEW);
  });
  it("pré-visualização do nome da pasta igual ao núcleo Rust", () => {
    // Mesmos casos que packages/project-management/src/paths.rs
    expect(folderPreview("2026-06-15", "Ana & Pedro", "x")).toBe("2026-06-15_Ana_e_Pedro.momento");
    expect(folderPreview("", "", "Batizado / Maria")).toBe("Batizado_Maria.momento");
    expect(folderPreview("", "  ", "  ")).toBe("Projeto.momento");
    expect(folderPreview("2026-01-01", "João Ção", "")).toBe("2026-01-01_João_Ção.momento");
  });
  it("grelha preenche a largura", () => {
    const l = computeLayout(1000, 180);
    expect(l.columns).toBe(5);
    expect(l.cell * l.columns + 8 * (l.columns - 1)).toBeLessThanOrEqual(1000 - 24);
    expect(visibleRows(0, 600, l.rowHeight, 1000)[0]).toBe(0);
    expect(visibleRows(100_000, 600, l.rowHeight, 1000)[1]).toBeLessThan(1000);
  });
  it("zoom do visualizador", () => {
    // 6000×4000 num palco de 1200×800 em ecrã Retina (dpr 2): ajustar = 40 %.
    expect(fitScale(6000, 4000, 1200, 800, 2)).toBeCloseTo(0.4);
    expect(fitScale(800, 600, 1200, 800, 1)).toBe(1); // nunca amplia além de 100 %
    expect(stepZoom(0.4, 1)).toBe(0.5);
    expect(stepZoom(1, 1)).toBe(2);
    expect(stepZoom(1, -1)).toBe(0.5);
  });
});

describe("auditoria final: sugestões e pessoas", () => {
  it("uma favorita ou com estrelas aparece como protegida, nunca como rejeição sugerida", async () => {
    const { LabelChip } = await import("./LabelChip");
    const { unmount } = render(<LabelChip label="reject" compact protectedByUser />);
    expect(screen.getByText("Protegida")).toBeTruthy();
    expect(screen.queryByText("Rejeitar?")).toBeNull();
    unmount();
    render(<LabelChip label="reject" compact />);
    expect(screen.getByText("Rejeitar?")).toBeTruthy();
  });

  it("papéis com mais do que uma pessoa não escondem fotografias", async () => {
    const { personsForRoles } = await import("./PeoplePanel");
    const p = (id: number, role: string | null) => ({ id, name: null, role, hidden: false, photoCount: 1, faceCount: 1, cover: null }) as never;
    expect(personsForRoles([p(1, "bride"), p(2, "groom")], ["bride", "groom"])).toEqual([1, 2]);
    expect(personsForRoles([p(1, "bride")], ["bride", "groom"])).toBeNull();
    expect(personsForRoles([p(1, "bride"), p(3, "bride"), p(2, "groom")], ["bride", "groom"])).toBe("ambiguous");
  });
});

describe("auditoria final: erros da interface", () => {
  it("uma falha num componente mostra a mensagem e não deixa a janela em branco", async () => {
    const { ErrorBoundary } = await import("./ErrorBoundary");
    const Boom = () => {
      throw new Error("falhou de propósito");
    };
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );
    spy.mockRestore();
    expect(screen.getByRole("alert").textContent).toContain("falhou de propósito");
    expect(screen.getByRole("button", { name: "Recarregar a interface" })).toBeTruthy();
  });
});

describe("pedidos do utilizador: Kelvin, sincronização, Lightroom", () => {
  it("temperatura em Kelvin: conversões nos dois sentidos e limites", async () => {
    const k = await import("../logic/kelvin");
    expect(k.formatKelvin(0, 5150)).toBe("5150");
    expect(k.formatKelvin(100, 3000)).toBe("6000");
    expect(Math.round(k.parseKelvin("6 000 K", 3000)!)).toBe(100);
    expect(k.parseKelvin("abc", 5000)).toBeNull();
    // Fora de 2000–50000 K: limitado.
    expect(k.kelvinFromRelative(k.parseKelvin("100", 5000)!, 5000)).toBeCloseTo(2000, 0);
    const [lo, hi] = k.relativeRange(5000);
    expect(k.kelvinFromRelative(lo, 5000)).toBeCloseTo(2000, 0);
    expect(k.kelvinFromRelative(hi, 5000)).toBeCloseTo(50000, 0);
  });

  it("sincronização automática só leva os blocos que mudaram e nunca recorte/máscaras/retoque", async () => {
    const { changedSyncBlocks } = await import("./develop/DevelopView");
    const { defaultSettings } = await import("../logic/develop");
    const a = defaultSettings();
    const b = defaultSettings();
    b.light.exposure = 0.5;
    b.wb.temperature = 12;
    b.geometry.straighten = 3;
    expect(changedSyncBlocks(a, b).sort()).toEqual(["light", "wb"]);
    expect(changedSyncBlocks(a, defaultSettings())).toEqual([]);
  });

  it("Lightroom: correspondência, confirmação e relatório", async () => {
    const { LightroomDialog } = await import("./LightroomDialog");
    const onClose = vi.fn();
    render(<LightroomDialog onClose={onClose} />);
    const dlg = screen.getByRole("dialog", { name: "Classificações para o Lightroom" });
    expect(dlg.textContent).toContain("★★★★★");
    expect(dlg.textContent).toContain("Os RAW nunca são alterados");
    fireEvent.click(within(dlg).getByRole("button", { name: "Escrever classificações" }));
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("2 ficheiros .xmp escritos"));
    expect(screen.getByRole("status").textContent).toContain("1 fotografia não é RAW");
  });
});
