import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnalysisDetails,
  AnalysisProgress,
  AnalysisStatus,
  AppInfo,
  GroupRow,
  RecommendationBrief,
  SelectionSettings,
  SelectionSummary,
  Color,
  ImportMode,
  ImportProgress,
  ImportSummary,
  NewProject,
  PhotoDetails,
  PhotoRow,
  ProjectInfo,
  RecentProject,
  ThumbBatch,
} from "./types";

/** Ponte única para o núcleo Rust. Todos os comandos estão em src-tauri/src/commands.rs. */
export const api = {
  available: () => isTauri(),
  appInfo: () => invoke<AppInfo>("app_info"),
  eventTypes: () => invoke<string[]>("event_types"),
  recentProjects: () => invoke<RecentProject[]>("list_recent_projects"),
  forgetRecent: (path: string) => invoke<void>("forget_recent_project", { path }),
  createProject: (input: NewProject) => invoke<ProjectInfo>("create_project", { input }),
  openProject: (path: string) => invoke<ProjectInfo>("open_project", { path }),
  closeProject: () => invoke<void>("close_project"),
  currentProject: () => invoke<ProjectInfo | null>("current_project"),
  listPhotos: () => invoke<PhotoRow[]>("list_photos"),
  photoDetails: (id: number) => invoke<PhotoDetails | null>("photo_details", { id }),
  setStars: (ids: number[], stars: number) => invoke<number>("set_stars", { ids, stars }),
  setFlag: (ids: number[], flag: number) => invoke<number>("set_flag", { ids, flag }),
  setColor: (ids: number[], color: Color | null) => invoke<number>("set_color", { ids, color }),
  startImport: (paths: string[], mode: ImportMode, recursive: boolean) =>
    invoke<number>("start_import", { request: { paths, mode, recursive } }),
  cancelImport: () => invoke<void>("cancel_import"),
  prioritizeThumbnails: (ids: number[]) => invoke<void>("prioritize_thumbnails", { ids }),
  saveUiState: (json: string) => invoke<void>("save_ui_state", { json }),
  // Seleção assistida (análise local; nada sai do computador).
  startAnalysis: (reanalyze: boolean) => invoke<number>("start_analysis", { reanalyze }),
  cancelAnalysis: () => invoke<void>("cancel_analysis"),
  analysisStatus: () => invoke<AnalysisStatus>("analysis_status"),
  setSelectionSettings: (settings: SelectionSettings) => invoke<SelectionSummary>("set_selection_settings", { settings }),
  recomputeSelection: () => invoke<SelectionSummary>("recompute_selection"),
  listRecommendations: () => invoke<RecommendationBrief[]>("list_recommendations"),
  listGroups: () => invoke<GroupRow[]>("list_groups"),
  analysisDetails: (id: number) => invoke<AnalysisDetails>("analysis_details", { id }),
  acceptRecommendations: (ids: number[]) => invoke<number>("accept_recommendations", { ids }),
  acceptGroup: (group: number) => invoke<number>("accept_group", { group }),
};

export const events = {
  importProgress: (cb: (p: ImportProgress) => void): Promise<UnlistenFn> =>
    listen<ImportProgress>("import-progress", (e) => cb(e.payload)),
  importFinished: (cb: (s: ImportSummary) => void): Promise<UnlistenFn> =>
    listen<ImportSummary>("import-finished", (e) => cb(e.payload)),
  thumbnails: (cb: (b: ThumbBatch) => void): Promise<UnlistenFn> =>
    listen<ThumbBatch>("thumbnails", (e) => cb(e.payload)),
  analysis: (cb: (p: AnalysisProgress) => void): Promise<UnlistenFn> =>
    listen<AnalysisProgress>("analysis-progress", (e) => cb(e.payload)),
};

export type ImageLevel = "thumb" | "preview" | "full";

/**
 * URL do protocolo de imagens. O Tauri usa `momento://localhost/` no macOS e no Linux e
 * `http://momento.localhost/` no Windows (WebView2).
 */
export function imageUrl(token: number, level: ImageLevel, id: number, platform = navigator.userAgent): string {
  const base = /Windows/i.test(platform) ? "http://momento.localhost" : "momento://localhost";
  return `${base}/${token}/${level}/${id}`;
}

/** Tecla modificadora principal: Cmd no macOS, Ctrl no Windows/Linux. */
export function isMac(platform = navigator.userAgent): boolean {
  return /Mac|iPhone|iPad/i.test(platform);
}
