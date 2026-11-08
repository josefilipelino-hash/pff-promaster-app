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
  error: number;
}

export type ItemState = "pending" | "imported" | "duplicate" | "alreadyImported" | "unsupported" | "error";

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
