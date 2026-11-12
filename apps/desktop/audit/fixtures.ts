// Gerado a partir de src/components/components.test.tsx (dados simulados dos testes) e
// adaptado para a auditoria visual num browser real (audit/run.mjs). Não é usado pela aplicação.
/* eslint-disable */
import type { DevelopSettings, PhotoRow, ProjectInfo } from "../src/types";
import { defaultSettings } from "../src/logic/develop";
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const fn = <T extends (...a: any[]) => any>(f: T): T => f;
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
  available: fn(() => true),
  appInfo: fn(async () => ({ appVersion: "0.1.0", coreVersion: "0.1.0", catalogSchemaVersion: 2, os: "macos", arch: "aarch64" })),
  eventTypes: fn(async () => ["wedding", "baptism"]),
  recentProjects: fn(async () => [] as unknown[]),
  forgetRecent: fn(async () => {}),
  createProject: fn(async () => undefined),
  openProject: fn(async () => undefined),
  closeProject: fn(async () => {}),
  currentProject: fn(async () => null),
  listPhotos: fn(async () => [] as PhotoRow[]),
  photoDetails: fn(async () => null),
  setStars: fn(async () => 1),
  setFlag: fn(async () => 1),
  setColor: fn(async () => 1),
  startImport: fn(async () => 1),
  safetyStatus: fn(async () => safetyFixture()),
  createBackup: fn(async () => ({ file: "/p/backups/catalog-manual-2.db", created: 1_794_300_000, bytes: 4096, kind: "manual" })),
  setBackupMinutes: fn(async () => {}),
  validateCatalog: fn(async () => ({ integrity: [], foreignKeys: 0 })),
  exportCatalog: fn(async () => {}),
  restoreBackup: fn(async () => ({ ...project, imageToken: 99 })),
  importCatalog: fn(async () => ({ ...project, imageToken: 98 })),
  checkAvailability: fn(async () => ({ ok: 4, offline: 1, missing: 0, changed: 0, reconnected: 0 })),
  removeFromCatalog: fn(async (keys: number[]) => ({ removed: keys.length, trashed: 0, failed: [] })),
  moveToTrash: fn(async (keys: number[]) => ({ removed: keys.length, trashed: keys.length, failed: [] })),
  revealOriginal: fn(async () => {}),
  relinkFile: fn(async () => ({ relinked: 3, stillUnavailable: 0 })),
  relinkFolder: fn(async () => ({ relinked: 2, stillUnavailable: 0 })),
  resumeExport: fn(async () => 31),
  discardExport: fn(async () => {}),
  cancelImport: fn(async () => {}),
  prioritizeThumbnails: fn(async () => {}),
  saveUiState: fn(async () => {}),
  startAnalysis: fn(async () => 10),
  cancelAnalysis: fn(async () => {}),
  analysisStatus: fn(async () => analysisStatus(0, 10)),
  setSelectionSettings: fn(async () => summary),
  recomputeSelection: fn(async () => summary),
  listRecommendations: fn(async () => [] as unknown[]),
  listGroups: fn(async () => [] as unknown[]),
  analysisDetails: fn(async () => ({ analysis: null, faces: [], recommendation: null, groupId: null, error: null }) as unknown),
  acceptRecommendations: fn(async () => 1),
    acceptGroup: fn(async () => 2),
  rebuildOrganization: fn(async () => ({})),
  listPersons: fn(async () => [] as unknown[]),
  personFaces: fn(async () => [] as unknown[]),
  photoPersons: fn(async () => [] as [number, number][]),
  updatePerson: fn(async () => {}),
  mergePersons: fn(async () => {}),
  excludeFaces: fn(async () => 1),
  reclusterPeople: fn(async () => 2),
  clearPeople: fn(async () => {}),
  listMoments: fn(async () => [] as unknown[]),
  assetMoments: fn(async () => [] as unknown[]),
  createMoment: fn(async () => 99),
  renameMoment: fn(async () => {}),
  deleteMoment: fn(async () => {}),
  assignMoment: fn(async () => 2),
  resetMoments: fn(async () => 1),
  reclassifyMoments: fn(async () => [0, 0]),
  listCameras: fn(async () => [] as unknown[]),
  suggestOffsets: fn(async () => [] as unknown[]),
    setCameraOffsets: fn(async () => ({})),
  editView: fn(async (key: number) => editViewOf(key)),
  renderEdit: fn(async (..._args: unknown[]) => frameBuffer(40, 30)),
  commitEdit: fn(async (key: number, _label: string, settings: unknown) => ({ ...editViewOf(key), settings, position: 1, canUndo: true, history: [{ seq: 0, label: "original", createdAt: 0 }, { seq: 1, label: "light.exposure", createdAt: 1 }] })),
  undoEdit: fn(async (key: number) => editViewOf(key)),
  redoEdit: fn(async (key: number) => editViewOf(key)),
  gotoHistory: fn(async (key: number) => editViewOf(key)),
  resetEdit: fn(async (key: number) => editViewOf(key)),
  createSnapshot: fn(async (key: number) => ({ ...editViewOf(key), snapshots: [{ id: 5, name: "Quente", createdAt: 1 }] })),
  applySnapshot: fn(async (key: number) => editViewOf(key)),
  deleteSnapshot: fn(async (key: number) => editViewOf(key)),
  createVirtualCopy: fn(async () => 2 ** 40 + 77),
  deleteVirtualCopy: fn(async () => {}),
  pasteSettings: fn(async (keys: number[]) => keys.length),
  lensProfile: fn(async () => ({ enabled: true, name: "Sony FE 24-70mm f/2.8 GM II", distortion: 100, vignetting: 100, chromatic: true, distModel: 3, dist: [0.0045, -0.0042, 0.0034], distScale: 1.8, tcaModel: 1, tca: [1, 1, 0, 0, 0, 0], vig: [-0.3, -0.1, 0.06], vigScale: 1 })),
  applyLensProfiles: fn(async (keys: number[]) => [keys.length, 0]),
  applyAspect: fn(async (keys: number[]) => keys.length),
  upright: fn(async () => ({ vertical: 31.23, horizontal: 0, straighten: -1.249, focal35: 24, mode: "vertical", verticalLines: 12, horizontalLines: 0, deviationBefore: 4.2, deviationAfter: 0.2 })),
  pickWhiteBalance: fn(async () => [23.4, -7.6] as [number, number]),
  autoStraighten: fn(async () => 2.34),
  asShotKelvin: fn(async () => null as number | null),
  exportLightroomRatings: fn(async () => ({ created: 2, updated: 0, byStars: [0, 1, 1, 0], notRaw: ["IMG_9.jpg"], unavailable: [], failed: [], starsInApp: 2 })),
  aiDenoiseAvailable: fn(async () => true),
  exportCapabilities: fn(async () => EXPORT_CAPS),
  exportRecipes: fn(async () => [EXPORT_JPEG, EXPORT_TIFF]),
  exportPlans: fn(async () => [{ id: "builtin:casamento", name: "Exportar casamento", recipes: [EXPORT_JPEG, EXPORT_TIFF] }]),
  saveExportRecipe: fn(async (r: unknown) => ({ ...(r as object), id: "x1" })),
  saveExportPlan: fn(async (p: unknown) => ({ ...(p as object), id: "x2" })),
  deleteExportPreset: fn(async () => true),
  inspectIccProfile: fn(async () => "Lab Glossy"),
  estimateExport: fn(async () => ({ photos: 3, files: 6, bytes: 120e6, freeBytes: 10e9, enoughSpace: true, perRecipe: [], notes: [], errors: [] })),
  startExport: fn(async () => 1),
  exportJobs: fn(async () => [] as unknown[]),
  pauseExport: fn(async () => true),
  cancelExport: fn(async () => true),
  clearFinishedExports: fn(async () => {}),
  revealFolder: fn(async () => {}),
  galleryConfig: fn(async () => ({ settings: { ...GALLERY_SETTINGS }, watermark: { ...EXPORT_BASE.watermark }, destination: "/tmp/pub" })),
  saveGalleryConfig: fn(async () => {}),
  galleryPreview: fn(async () => ({ photos: 4, hidden: 0, chapters: [{ title: "Preparação", count: 2 }, { title: "Cerimónia", count: 2 }], folder: "/tmp/pub/ana-e-pedro-galeria", exists: true, blocked: false, problems: [] as string[], estimateBytes: 50e6 })),
  startGallery: fn(async () => 9),
  setGalleryHidden: fn(async (keys: number[], hidden: boolean) => ({ settings: { ...GALLERY_SETTINGS, hidden: hidden ? keys : [] } })),
  importClientFavorites: fn(async () => ({ matched: 2, unmatched: ["x"] })),
  openGallery: fn(async () => {}),
  autoEditPhoto: fn(async () => undefined),
  startAutoEdit: fn(async (keys: number[]) => keys.length),
  autoEditStatus: fn(async () => undefined),
  pauseAutoEdit: fn(async () => {}),
  resumeAutoEdit: fn(async () => {}),
  cancelAutoEdit: fn(async () => {}),
  listPresets: fn(async () => presetList()),
  createPreset: fn(async (name: string, ..._rest: unknown[]) => ({ ...presetList()[0]!, id: "u1", name, builtin: false, group: "" })),
  renamePreset: fn(async () => ({})),
  movePreset: fn(async () => ({})),
  reorderPresets: fn(async () => {}),
  duplicatePreset: fn(async () => ({})),
  deletePreset: fn(async () => {}),
  importPresetFile: fn(async () => undefined),
  exportPresets: fn(async (ids: string[]) => ids.length),
  presetPreviewSettings: fn(async (settings: unknown, _id: string, amount: number) => ({ ...(settings as object), light: { ...(settings as { light: object }).light, contrast: amount / 10 } })),
  applyPreset: fn(async (keys: number[]) => keys.length),
  measureConsistency: fn(async () => 4),
  consistencyGroups: fn(async () => ({ groups: lookGroups(), missing: 0 })),
  startSync: fn(async (_r: number, keys: number[]) => keys.length),
  createMaskRaster: fn(async (_k: number, kind: string) => ({ raster: `r-${kind}`, kind, coverage: 0.4 })),
  detectPortrait: fn(async () => [faceRegions()]),
  mapPoints: fn(async (_k: number, _s: unknown, pts: [number, number][]) => pts),
  suggestHealSource: fn(async (_k: number, x: number, y: number) => [x + 0.1, y] as [number, number]),
  albumFormats: fn(async () => [{ name: "20x20", widthCm: 20, heightCm: 20 }, { name: "30x30", widthCm: 30, heightCm: 30 }, { name: "30x40", widthCm: 40, heightCm: 30 }]),
  albumTemplates: fn(async (n: number) => (n <= 1 ? ["P", "-|B", "B|-"] : ["B|B", "F|F", "-|G2"])),
  listAlbums: fn(async () => [{ id: "a1", name: "Álbum Ana & Pedro", format: "30x30", pages: 4, updatedAt: "" }]),
  getAlbum: fn(async () => albumFixture()),
  createAlbum: fn(async (name: string, spec: unknown) => ({ ...albumFixture(), id: "a2", name, spec })),
  saveAlbum: fn(async () => {}),
  deleteAlbum: fn(async () => true),
  autoAlbum: fn(async () => ({ album: { ...albumFixture(), notes: ["abertura:1", "cerimonia:1"] }, used: 3, candidates: 4, skippedLowRes: 1, skippedDuplicates: 0 })),
  applyAlbumLayout: fn(async (album: { spreads: { template: string }[] }, idx: number, tpl: string) => ({ ...album, spreads: album.spreads.map((s, i) => (i === idx ? { ...s, template: tpl } : s)) })),
  albumPreflight: fn(async () => [
    { spread: null, slot: null, severity: "warning", code: "album.issue.unconfirmed", value: null },
    { spread: 0, slot: "s1", severity: "error", code: "album.issue.lowDpi", value: 120 },
  ]),
  albumPhotoInfos: fn(async () => [1, 2, 3, 4].map((key) => ({ key, width: 6000, height: 4000, time: null, moment: null, flag: 1, stars: 4, score: 0.8, faces: key === 2 ? [[0.4, 0.2, 0.1, 0.15]] : [], persons: [], group: null }))),
  startAlbumExport: fn(async () => 21),
  labPresets: fn(async () => [{ id: "l1", name: "Lab Porto", notes: "", dpi: 300, bleedMm: 5, safeMm: 15, gutterMm: 20, color: null, iccPath: "", intent: "perceptual", exportUnit: "pages", minPages: 20, maxPages: 80 }]),
  saveLabPreset: fn(async (p: object) => ({ ...p, id: "l2" })),
  deleteLabPreset: fn(async () => true),
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
function editViewOf(key: number) {
  return { key, asset: key, variant: key, position: 0, history: [], canUndo: false, canRedo: false, snapshots: [], settings: defaultSettings() };
}
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

export { apiMock, project, rows, editViewOf };
