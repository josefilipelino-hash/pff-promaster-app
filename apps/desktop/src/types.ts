/** Espelhos dos tipos Rust (packages/database, packages/project-management). */

export type ThumbState = "pending" | "ready" | "failed";
export type Color = "red" | "yellow" | "green" | "blue" | "purple";
export const COLORS: Color[] = ["red", "yellow", "green", "blue", "purple"];

export interface PhotoRow {
  /** Chave da fotografia (versão principal = id do ficheiro; cópias virtuais ≥ 2^40). */
  id: number;
  /** Ficheiro (asset) da fotografia. */
  asset: number;
  variant: number;
  /** Nome da cópia virtual (null = versão principal). */
  virtualCopy: string | null;
  edited: boolean;
  fileName: string;
  captureTime: string | null;
  width: number;
  height: number;
  stars: number;
  /** -1 rejeitada, 0 sem marca, 1 favorita */
  flag: number;
  color: Color | null;
  thumb: ThumbState;
  /** Suporte parcial (ex.: RAW só com preview embutida). */
  partial: boolean;
  format: string;
}

export interface PhotoDetails {
  id: number;
  path: string;
  sourcePath: string | null;
  fileName: string;
  fileSize: number;
  format: string;
  width: number;
  height: number;
  orientation: number;
  captureTime: string | null;
  captureOffset: string | null;
  cameraMake: string | null;
  cameraModel: string | null;
  cameraSerial: string | null;
  lens: string | null;
  iso: number | null;
  exposureTime: number | null;
  fNumber: number | null;
  focalLength: number | null;
  focalLength35mm: number | null;
  importedAt: number;
  thumbError: string | null;
  bitDepth: number | null;
  colorSpace: string | null;
  codec: string | null;
  codecDetail: string | null;
  embeddedPreview: string | null;
  partialNote: string | null;
}

export interface ProjectInfo {
  path: string;
  name: string;
  clientName: string;
  eventType: string;
  eventDate: string | null;
  location: string;
  originalsDir: string | null;
  status: string;
  notes: string;
  createdAt: number;
  updatedAt: number;
  photoCount: number;
  uiState: string | null;
  importRunning: boolean;
  imageToken: number;
}

export interface RecentProject {
  path: string;
  name: string;
  clientName: string;
  eventType: string;
  eventDate: string | null;
  location: string;
  photoCount: number;
  lastOpened: number;
  available: boolean;
}

export interface NewProject {
  name: string;
  clientName: string;
  eventType: string;
  eventDate: string | null;
  location: string;
  originalsDir: string | null;
  workDir: string;
}

export type ImportMode = "reference" | "copy";

export interface ImportProgress {
  sessionId: number;
  total: number;
  processed: number;
  imported: number;
  duplicates: number;
  skipped: number;
  errors: number;
}

export interface SessionCounts {
  total: number;
  pending: number;
  imported: number;
  duplicate: number;
  alreadyImported: number;
  unsupported: number;
  missingCodec: number;
  corrupt: number;
  error: number;
}

export type ItemState = "pending" | "imported" | "duplicate" | "alreadyImported" | "unsupported" | "missingCodec" | "corrupt" | "error";

export interface ImportSummary {
  sessionId: number;
  counts: SessionCounts;
  issues: { sourcePath: string; state: ItemState; message: string | null }[];
  cancelled: boolean;
  suspended: boolean;
  error: string | null;
}

export interface ThumbBatch {
  ready: number[];
  failed: number[];
  remaining: number;
  done: number;
  total: number;
}

export interface ErrorPayload {
  code: string;
  detail: string;
}

export interface AppInfo {
  appVersion: string;
  coreVersion: string;
  catalogSchemaVersion: number;
  os: string;
  arch: string;
}

// --- Seleção assistida (Fase 3) ---------------------------------------------------------

export type Label = "best" | "alternative" | "review" | "reject" | "duplicate";
export const LABELS: Label[] = ["best", "alternative", "review", "reject", "duplicate"];
export type SelectionMode = "conservative" | "balanced" | "strict";
export const SELECTION_MODES: SelectionMode[] = ["conservative", "balanced", "strict"];

export interface SelectionSettings {
  mode: SelectionMode;
  /** Número aproximado de fotografias a entregar (null = sem objetivo). */
  target: number | null;
}

export interface SelectionSummary {
  best: number;
  alternative: number;
  review: number;
  reject: number;
  duplicate: number;
  groups: number;
  sequences: number;
  durationMs: number;
}

export interface AnalysisCounts {
  total: number;
  analyzed: number;
  failed: number;
  pending: number;
}

export interface AnalysisStatus {
  running: boolean;
  counts: AnalysisCounts;
  modelsAvailable: boolean;
  settings: SelectionSettings;
  summary: SelectionSummary;
}

export interface AnalysisProgress {
  done: number;
  failed: number;
  total: number;
  remaining: number;
  finished: boolean;
}

export interface RecommendationBrief {
  asset: number;
  label: Label;
  groupId: number | null;
  rank: number;
  groupSize: number;
}

export interface GroupRow {
  id: number;
  kind: "sequence" | "duplicate" | "raw_jpeg";
  size: number;
  /** Do melhor para o pior. */
  members: number[];
}

export interface AnalysisRow {
  models: string | null;
  width: number;
  height: number;
  sharpness: number;
  focusPeak: number;
  focusCenter: number;
  motionBlur: number;
  brightness: number;
  highlightsClipped: number;
  shadowsCrushed: number;
  contrast: number;
  noise: number;
  tiltDegrees: number | null;
  phash: number;
  dhash: number;
  durationMs: number;
}

export interface FaceRow {
  x: number;
  y: number;
  w: number;
  h: number;
  px: number;
  detScore: number;
  presence: number;
  blinkLeft: number;
  blinkRight: number;
  smile: number;
  jawOpen: number;
  sharpness: number;
  eyeSharpness: number;
  cut: boolean;
}

export interface Reason {
  code: string;
  value?: number;
}

export interface RecommendationRow {
  asset: number;
  label: Label;
  rank: number;
  groupSize: number;
  summary: string;
  /** JSON `[{code, value?}]`. */
  reasons: string;
  focus: number;
  motion: number;
  exposure: number;
  noise: number;
  eyes: number | null;
  expression: number | null;
  composition: number;
  technical: number;
  people: number | null;
  overall: number;
}

export interface AnalysisDetails {
  analysis: AnalysisRow | null;
  faces: FaceRow[];
  recommendation: RecommendationRow | null;
  groupId: number | null;
  error: string | null;
}

// --- Organização do casamento (Fase 4) ---------------------------------------------------

export const ROLES = ["bride", "groom", "bride_mother", "bride_father", "groom_mother", "groom_father", "sibling", "best_man", "maid_of_honor", "important", "other"] as const;
export type Role = (typeof ROLES)[number];

export interface FaceRef {
  faceId: number;
  asset: number;
  x: number;
  y: number;
  w: number;
  h: number;
  /** 0 automático, 1 confirmado */
  source: number;
}

export interface PersonRow {
  id: number;
  name: string | null;
  role: Role | null;
  hidden: boolean;
  faceCount: number;
  photoCount: number;
  cover: FaceRef | null;
}

export interface MomentRow {
  id: number;
  /** Chave predefinida; null = personalizado */
  kind: string | null;
  name: string | null;
  sort: number;
  custom: boolean;
  count: number;
}

export interface AssetMoment {
  asset: number;
  /** null = “Por classificar” escolhido pelo utilizador */
  momentId: number | null;
  confidence: number | null;
  source: "auto" | "user";
  reasons: string | null;
}

export interface CameraRow {
  key: string;
  make: string | null;
  model: string | null;
  serial: string | null;
  photos: number;
  firstTime: string | null;
  lastTime: string | null;
  offsetMs: number;
  reference: boolean;
}

export interface CameraOffset {
  key: string;
  offsetMs: number;
  reference: boolean;
}

export interface OffsetSuggestion {
  camera: string;
  offsetMs: number;
  votes: number;
  agreement: number;
}

export interface OrganizeSummary {
  selection: SelectionSummary;
  persons: number;
  classified: number;
  unclassified: number;
  durationMs: number;
}

// --- Edição (Fase 5) -----------------------------------------------------------------------

export interface DevelopSettings {
  light: { exposure: number; contrast: number; highlights: number; shadows: number; whites: number; blacks: number };
  wb: { temperature: number; tint: number };
  color: { saturation: number; vibrance: number };
  hsl: { hue: number[]; saturation: number[]; luminance: number[] };
  curve: {
    highlights: number;
    lights: number;
    darks: number;
    shadows: number;
    splitShadows: number;
    splitMidtones: number;
    splitHighlights: number;
    rgb: [number, number][];
    red: [number, number][];
    green: [number, number][];
    blue: [number, number][];
  };
  detail: {
    sharpenAmount: number;
    sharpenRadius: number;
    sharpenDetail: number;
    sharpenMasking: number;
    noiseLuminance: number;
    noiseColor: number;
    /** Preservação do detalhe na redução de ruído (0–100). */
    noiseDetail: number;
    /** AI Denoise: 0 = desligado, 1–100 = intensidade. */
    aiDenoise: number;
  };
  effects: {
    texture: number;
    clarity: number;
    dehaze: number;
    vignette: number;
    vignetteMidpoint: number;
    vignetteFeather: number;
    vignetteRoundness: number;
    grain: number;
    grainSize: number;
    grainRoughness: number;
  };
  geometry: {
    crop: { x: number; y: number; w: number; h: number };
    quarterTurns: number;
    flipHorizontal: boolean;
    straighten: number;
    aspect: string;
    vertical: number;
    horizontal: number;
  };
  lens: { distortion: number; caRed: number; caBlue: number; vignetting: number; vignettingMidpoint: number };
  retouch: {
    redEyes: { x: number; y: number; r: number }[];
    spots: Spot[];
    patches: Patch[];
  };
  grading: { shadowsHue: number; shadowsSat: number; highlightsHue: number; highlightsSat: number; balance: number };
  bw: { enabled: boolean; mix: number[] };
  lut: { id: string; name: string; amount: number };
  masks: LocalMask[];
  portrait: Portrait;
}

// --- Edição localizada (Fase 8) — coordenadas normalizadas da fotografia orientada ----------

export interface Spot {
  x: number;
  y: number;
  /** Raio em unidades do lado maior. */
  r: number;
  sx: number;
  sy: number;
  heal: boolean;
  feather: number;
  opacity: number;
}

export interface BrushStroke {
  points: [number, number][];
  size: number;
  feather: number;
  flow: number;
  erase: boolean;
}

export interface Patch {
  stroke: BrushStroke;
  dx: number;
  dy: number;
  heal: boolean;
  opacity: number;
}

export type MaskOp = "add" | "subtract" | "intersect";

export type MaskShape =
  | { type: "brush"; strokes: BrushStroke[] }
  | { type: "linear"; x0: number; y0: number; x1: number; y1: number }
  | { type: "radial"; cx: number; cy: number; rx: number; ry: number; angle: number; feather: number }
  | { type: "luminance"; min: number; max: number; smooth: number }
  | { type: "color"; hue: number; range: number; minSat: number; smooth: number }
  | { type: "raster"; kind: string; raster: string; feather: number };

export interface MaskComponent {
  op: MaskOp;
  invert: boolean;
  shape: MaskShape;
}

export interface LocalAdjust {
  exposure: number;
  contrast: number;
  highlights: number;
  shadows: number;
  whites: number;
  blacks: number;
  temperature: number;
  tint: number;
  saturation: number;
  texture: number;
  clarity: number;
  sharpness: number;
  noise: number;
}

export interface LocalMask {
  id: string;
  name: string;
  enabled: boolean;
  invert: boolean;
  density: number;
  components: MaskComponent[];
  adjust: LocalAdjust;
}

export interface FaceRegions {
  oval: [number, number][];
  leftEye: [number, number][];
  rightEye: [number, number][];
  leftBrow: [number, number][];
  rightBrow: [number, number][];
  lipsOuter: [number, number][];
  lipsInner: [number, number][];
  underLeft: [number, number][];
  underRight: [number, number][];
  leftIris: [number, number, number];
  rightIris: [number, number, number];
}

export interface Portrait {
  skin: number;
  darkCircles: number;
  shine: number;
  teeth: number;
  eyes: number;
  faces: FaceRegions[];
}

/** Tipos de máscara que se podem criar. */
export const MASK_KINDS = ["brush", "linear", "radial", "luminance", "color", "subject", "person", "face", "skin", "hair", "clothes", "sky", "background", "object"] as const;
export type MaskKind = (typeof MASK_KINDS)[number];
/** Máscaras calculadas (IA local ou céu). */
export const RASTER_KINDS: MaskKind[] = ["subject", "person", "face", "skin", "hair", "clothes", "sky", "background", "object"];
export const LOCAL_FIELDS: (keyof LocalAdjust)[] = ["exposure", "contrast", "highlights", "shadows", "whites", "blacks", "temperature", "tint", "saturation", "texture", "clarity", "sharpness", "noise"];

export interface HistoryEntry {
  seq: number;
  label: string;
  createdAt: number;
}

export interface SnapshotRow {
  id: number;
  name: string;
  createdAt: number;
}

export interface EditView {
  key: number;
  asset: number;
  variant: number;
  position: number;
  history: HistoryEntry[];
  canUndo: boolean;
  canRedo: boolean;
  snapshots: SnapshotRow[];
  settings: DevelopSettings;
}

export const SETTINGS_BLOCKS = ["light", "wb", "color", "hsl", "curve", "detail", "effects", "geometry", "lens", "retouch", "grading", "bw", "lut", "masks", "portrait"] as const;
export type SettingsBlock = (typeof SETTINGS_BLOCKS)[number];

// --- Edição automática (Fase 6) -------------------------------------------------------------

export type AutoIntensity = "verySoft" | "soft" | "natural" | "balanced" | "strong" | "custom";
export const AUTO_INTENSITIES: AutoIntensity[] = ["verySoft", "soft", "natural", "balanced", "strong", "custom"];
export type AiDenoiseMode = "off" | "whenNeeded" | "always";

export interface AutoOptions {
  intensity: AutoIntensity;
  /** Força em % (só "custom"), 0–150. */
  strength: number;
  tone: boolean;
  whiteBalance: boolean;
  color: boolean;
  sharpen: boolean;
  denoise: boolean;
  aiDenoise: AiDenoiseMode;
  straighten: boolean;
  crop: boolean;
  lens: boolean;
  redEye: boolean;
  dust: boolean;
}

export type NoiseLevel = "none" | "low" | "moderate" | "high" | "veryHigh";

export interface AutoReport {
  exposure: number;
  temperature: number;
  tint: number;
  noise: { luminance: number; chroma: number; iso: number | null; sensorFactor: number; level: NoiseLevel; aiRecommended: boolean };
  tilt: number | null;
  cropped: boolean;
  redEyes: number;
  spots: number;
  faces: number;
}

export interface AutoEditResult {
  view: EditView;
  report: AutoReport;
}

export interface AutoEditProgress {
  /** "auto" (edição automática) ou "sync" (sincronizar com referência). */
  kind: string;
  total: number;
  done: number;
  failed: number;
  paused: boolean;
  cancelled: boolean;
  finished: boolean;
  current: string | null;
}

export interface AiDenoiseProgress {
  key: number;
  done: number;
  total: number;
  finished: boolean;
}

// --- Presets e consistência (Fase 7) --------------------------------------------------------

export type PresetComponent =
  | "exposure"
  | "tone"
  | "whiteBalance"
  | "presence"
  | "hsl"
  | "curve"
  | "grading"
  | "blackWhite"
  | "sharpening"
  | "noiseReduction"
  | "effects"
  | "lens"
  | "lut";
export const PRESET_COMPONENTS: PresetComponent[] = ["exposure", "tone", "whiteBalance", "presence", "hsl", "curve", "grading", "blackWhite", "sharpening", "noiseReduction", "effects", "lens", "lut"];
export const LOOK_COMPONENTS: PresetComponent[] = ["tone", "presence", "hsl", "curve", "grading", "blackWhite", "effects", "lut"];

export interface Preset {
  id: string;
  name: string;
  group: string;
  components: PresetComponent[];
  settings: DevelopSettings;
  builtin: boolean;
  description: string;
}

export interface CompatReport {
  interpreted: string[];
  ignored: [string, string][];
}

export interface PresetImportResult {
  presets: Preset[];
  report: CompatReport | null;
  kind: "momentopreset" | "xmp" | "cube";
}

export type Criterion = "combined" | "lighting" | "camera" | "scene" | "time" | "whiteBalance" | "flash";
export const CRITERIA: Criterion[] = ["combined", "lighting", "camera", "scene", "time", "whiteBalance", "flash"];

export interface LookGroup {
  id: string;
  code: string;
  tags: string[];
  detail: string;
  moment: number | null;
  start: string | null;
  end: string | null;
  ids: number[];
}

export interface MatchOptions {
  exposure: boolean;
  whiteBalance: boolean;
  skin: boolean;
  contrast: boolean;
  color: boolean;
  look: boolean;
}

// ---------------------------------------------------------------- exportação (Fase 9)

export type ExportFormat = "jpeg" | "jpegXl" | "jpeg2000" | "png" | "tiff" | "bigTiff" | "webp" | "avif" | "heif" | "heic" | "exr" | "dng" | "pdf" | "psd";
export type ExportDepth = "8" | "10" | "12" | "16" | "16f" | "32f";
export type ExportCompression = "none" | "fast" | "best" | "lzw" | "zip" | "piz" | "rle" | "lossless" | "lossy";
export type ExportColorSpace = "srgb" | "displayP3" | "adobeRgb" | "proPhoto" | "rec709" | "rec2020" | "icc";
export const EXPORT_SPACES: ExportColorSpace[] = ["srgb", "displayP3", "adobeRgb", "proPhoto", "rec709", "rec2020", "icc"];
export type SizeMode = "original" | "longEdge" | "shortEdge" | "width" | "height" | "fit" | "fill" | "megapixels";
export const SIZE_MODES: SizeMode[] = ["original", "longEdge", "shortEdge", "width", "height", "fit", "fill", "megapixels"];
export type Anchor = "topLeft" | "top" | "topRight" | "left" | "center" | "right" | "bottomLeft" | "bottom" | "bottomRight";
export const ANCHORS: Anchor[] = ["topLeft", "top", "topRight", "left", "center", "right", "bottomLeft", "bottom", "bottomRight"];

export interface ExportCaps {
  format: ExportFormat;
  available: boolean;
  /** Chave i18n da razão quando indisponível. */
  reason: string | null;
  extension: string;
  depths: ExportDepth[];
  compressions: ExportCompression[];
  quality: boolean;
  spaces: "icc" | "builtin" | "cicp";
  metadata: boolean;
  resolution: boolean;
  linear: boolean;
}

export interface ExportRecipe {
  id: string;
  name: string;
  format: ExportFormat;
  depth: ExportDepth;
  quality: number;
  compression: ExportCompression;
  color: ExportColorSpace;
  iccPath: string;
  intent: "perceptual" | "relativeColorimetric";
  size: { mode: SizeMode; unit: "px" | "cm" | "in"; a: number; b: number; noEnlarge: boolean; ppi: number; matchOrientation: boolean };
  sharpen: { target: "none" | "screen" | "matte" | "glossy"; amount: "low" | "standard" | "high" };
  metadata: { mode: "all" | "copyright" | "none"; removeGps: boolean; copyright: string; creator: string; contact: string };
  watermark: {
    enabled: boolean;
    kind: "text" | "image";
    text: string;
    imagePath: string;
    fontPath: string;
    color: string;
    size: number;
    opacity: number;
    anchor: Anchor;
    margin: number;
  };
  naming: { template: string; prefix: string; suffix: string; seqStart: number; seqDigits: number; subfolder: string; conflict: "rename" | "overwrite" | "skip" };
}

export interface ExportPlan {
  id: string;
  name: string;
  recipes: ExportRecipe[];
}

export interface ExportRequest {
  keys: number[];
  recipes: ExportRecipe[];
  destination: string;
  name: string;
  momentNames: Record<number, string>;
}

export interface ExportNote {
  recipe: string;
  code: string;
}

export interface ExportEstimate {
  photos: number;
  files: number;
  bytes: number;
  freeBytes: number | null;
  enoughSpace: boolean;
  perRecipe: { name: string; files: number; bytes: number }[];
  notes: ExportNote[];
  errors: ExportNote[];
}

export type ExportJobState = "queued" | "running" | "paused" | "done" | "cancelled" | "failed";

export interface ExportProgress {
  id: number;
  name: string;
  destination: string;
  state: ExportJobState;
  pausedReason: string | null;
  photosTotal: number;
  photosDone: number;
  filesTotal: number;
  filesDone: number;
  filesFailed: number;
  filesSkipped: number;
  bytesWritten: number;
  bytesEstimated: number;
  freeBytes: number | null;
  elapsedSeconds: number;
  etaSeconds: number | null;
  current: string | null;
  errors: { file: string; message: string }[];
  notes: ExportNote[];
  folders: string[];
  /** Galeria: index.html gerado. */
  index: string | null;
}

// ---------------------------------------------------------------- galeria (Fase 10)

export type GalleryTheme = "light" | "dark" | "minimal" | "editorial";
export const GALLERY_THEMES: GalleryTheme[] = ["light", "dark", "minimal", "editorial"];

export interface GallerySettings {
  couple: string;
  date: string;
  location: string;
  intro: string;
  cover: number | null;
  hero: number | null;
  logoPath: string;
  theme: GalleryTheme;
  layout: "masonry" | "grid";
  language: "pt" | "en";
  selection: "picks" | "notRejected" | "stars" | "all";
  minStars: number;
  chapters: "grouped" | "detailed" | "none";
  hidden: number[];
  favorites: boolean;
  downloads: "none" | "individual" | "all";
  downloadSize: "web" | "high";
  chapterZips: boolean;
  protection: "none" | "pin" | "password";
  secret: string;
  photographer: string;
  contactEmail: string;
  slideshowSeconds: number;
}

export interface GalleryConfig {
  settings: GallerySettings;
  watermark: ExportRecipe["watermark"];
  destination: string;
}

export interface GalleryPreview {
  photos: number;
  hidden: number;
  chapters: { title: string; count: number }[];
  folder: string;
  exists: boolean;
  blocked: boolean;
  problems: string[];
  estimateBytes: number;
}

export interface FavoritesImport {
  matched: number;
  unmatched: string[];
}
