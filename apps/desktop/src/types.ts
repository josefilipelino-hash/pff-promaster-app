/** Espelhos dos tipos Rust (packages/database, packages/project-management). */

export type ThumbState = "pending" | "ready" | "failed";
export type Color = "red" | "yellow" | "green" | "blue" | "purple";
export const COLORS: Color[] = ["red", "yellow", "green", "blue", "purple"];

export interface PhotoRow {
  id: number;
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
