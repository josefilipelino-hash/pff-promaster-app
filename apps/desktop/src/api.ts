import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Album,
  AlbumAutoReport,
  AlbumExportKind,
  AlbumFormat,
  AlbumIssue,
  AlbumPhotoInfo,
  AlbumSpec,
  AlbumSummary,
  LabPreset,
  FavoritesImport,
  GalleryConfig,
  GalleryPreview,
  ExportCaps,
  ExportEstimate,
  ExportPlan,
  ExportProgress,
  ExportRecipe,
  ExportRequest,
  FaceRegions,
  Criterion,
  LookGroup,
  MatchOptions,
  Preset,
  PresetComponent,
  PresetImportResult,
  AiDenoiseProgress,
  AutoEditProgress,
  AutoEditResult,
  AutoOptions,
  DevelopSettings,
  EditView,
  AssetMoment,
  CameraOffset,
  CameraRow,
  FaceRef,
  MomentRow,
  OffsetSuggestion,
  OrganizeSummary,
  PersonRow,
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
  // Organização (pessoas, momentos, câmaras) — dados biométricos só no catálogo local.
  rebuildOrganization: () => invoke<OrganizeSummary>("rebuild_organization"),
  listPersons: () => invoke<PersonRow[]>("list_persons"),
  personFaces: (person: number, limit: number) => invoke<FaceRef[]>("person_faces", { person, limit }),
  photoPersons: () => invoke<[number, number][]>("photo_persons"),
  updatePerson: (id: number, name: string | null, role: string | null, hidden: boolean) => invoke<void>("update_person", { id, name, role, hidden }),
  mergePersons: (from: number, into: number) => invoke<void>("merge_persons", { from, into }),
  excludeFaces: (faces: number[]) => invoke<number>("exclude_faces", { faces }),
  reclusterPeople: () => invoke<number>("recluster_people"),
  clearPeople: () => invoke<void>("clear_people"),
  listMoments: () => invoke<MomentRow[]>("list_moments"),
  assetMoments: () => invoke<AssetMoment[]>("asset_moments"),
  createMoment: (name: string, after: number | null) => invoke<number>("create_moment", { name, after }),
  renameMoment: (id: number, name: string) => invoke<void>("rename_moment", { id, name }),
  deleteMoment: (id: number) => invoke<void>("delete_moment", { id }),
  assignMoment: (ids: number[], moment: number | null) => invoke<number>("assign_moment", { ids, moment }),
  resetMoments: (ids: number[]) => invoke<number>("reset_moments", { ids }),
  reclassifyMoments: () => invoke<[number, number]>("reclassify_moments"),
  listCameras: () => invoke<CameraRow[]>("list_cameras"),
  suggestOffsets: (reference: string) => invoke<OffsetSuggestion[]>("suggest_offsets", { reference }),
    setCameraOffsets: (offsets: CameraOffset[]) => invoke<OrganizeSummary>("set_camera_offsets", { offsets }),
  // Edição não destrutiva (os originais nunca são escritos).
  editView: (key: number) => invoke<EditView>("edit_view", { key }),
  renderEdit: (key: number, settings: DevelopSettings, maxWidth: number, maxHeight: number, quality: "fast" | "final", uncropped: boolean, overlay: number | null = null) =>
    invoke<ArrayBuffer>("render_edit", { key, settings, maxWidth, maxHeight, quality, uncropped, overlay }),
  commitEdit: (key: number, label: string, settings: DevelopSettings) => invoke<EditView>("commit_edit", { key, label, settings }),
  undoEdit: (key: number) => invoke<EditView>("undo_edit", { key }),
  redoEdit: (key: number) => invoke<EditView>("redo_edit", { key }),
  gotoHistory: (key: number, seq: number) => invoke<EditView>("goto_history", { key, seq }),
  resetEdit: (key: number) => invoke<EditView>("reset_edit", { key }),
  createSnapshot: (key: number, name: string) => invoke<EditView>("create_snapshot", { key, name }),
  applySnapshot: (key: number, id: number) => invoke<EditView>("apply_snapshot", { key, id }),
  deleteSnapshot: (key: number, id: number) => invoke<EditView>("delete_snapshot", { key, id }),
  createVirtualCopy: (key: number) => invoke<number>("create_virtual_copy", { key }),
  deleteVirtualCopy: (key: number) => invoke<void>("delete_virtual_copy", { key }),
  pasteSettings: (keys: number[], settings: DevelopSettings, blocks: string[]) => invoke<number>("paste_settings", { keys, settings, blocks }),
  pickWhiteBalance: (key: number, settings: DevelopSettings, u: number, v: number) => invoke<[number, number] | null>("pick_white_balance", { key, settings, u, v }),
  autoStraighten: (key: number, settings: DevelopSettings) => invoke<number | null>("auto_straighten", { key, settings }),
  // Edição automática (cada fotografia é analisada; fila em segundo plano).
  autoEditPhoto: (key: number, options: AutoOptions) => invoke<AutoEditResult>("auto_edit_photo", { key, options }),
  startAutoEdit: (keys: number[], options: AutoOptions) => invoke<number>("start_auto_edit", { keys, options }),
  autoEditStatus: () => invoke<AutoEditProgress>("auto_edit_status"),
  pauseAutoEdit: () => invoke<void>("pause_auto_edit"),
  resumeAutoEdit: () => invoke<void>("resume_auto_edit"),
  cancelAutoEdit: () => invoke<void>("cancel_auto_edit"),
  aiDenoiseAvailable: () => invoke<boolean>("ai_denoise_available"),
  // Presets (biblioteca global) e consistência.
  listPresets: () => invoke<Preset[]>("list_presets"),
  createPreset: (name: string, group: string, components: PresetComponent[], settings: DevelopSettings) =>
    invoke<Preset>("create_preset", { name, group, components, settings }),
  renamePreset: (id: string, name: string) => invoke<Preset>("rename_preset", { id, name }),
  movePreset: (id: string, group: string) => invoke<Preset>("move_preset", { id, group }),
  reorderPresets: (ids: string[]) => invoke<void>("reorder_presets", { ids }),
  duplicatePreset: (id: string, suffix: string) => invoke<Preset>("duplicate_preset", { id, suffix }),
  deletePreset: (id: string) => invoke<void>("delete_preset", { id }),
  importPresetFile: (path: string) => invoke<PresetImportResult>("import_preset_file", { path }),
  exportPresets: (ids: string[], path: string) => invoke<number>("export_presets", { ids, path }),
  presetPreviewSettings: (settings: DevelopSettings, presetId: string, amount: number) =>
    invoke<DevelopSettings>("preset_preview_settings", { settings, presetId, amount }),
  applyPreset: (keys: number[], presetId: string, amount: number) => invoke<number>("apply_preset", { keys, presetId, amount }),
  measureConsistency: () => invoke<number>("measure_consistency"),
  consistencyGroups: (criterion: Criterion) => invoke<{ groups: LookGroup[]; missing: number }>("consistency_groups", { criterion }),
  startSync: (reference: number, keys: number[], options: MatchOptions) => invoke<number>("start_sync", { reference, keys, options }),
  // Edição localizada.
  createMaskRaster: (key: number, kind: string, point: [number, number] | null) =>
    invoke<{ raster: string; kind: string; coverage: number }>("create_mask_raster", { key, kind, point }),
  detectPortrait: (key: number) => invoke<FaceRegions[]>("detect_portrait", { key }),
  mapPoints: (key: number, settings: DevelopSettings, points: [number, number][], toSource: boolean, uncropped: boolean) =>
    invoke<([number, number] | null)[]>("map_points", { key, settings, points, toSource, uncropped }),
  suggestHealSource: (key: number, x: number, y: number, r: number) => invoke<[number, number]>("suggest_heal_source", { key, x, y, r }),
  // Exportação.
  exportCapabilities: () => invoke<ExportCaps[]>("export_capabilities"),
  exportRecipes: () => invoke<ExportRecipe[]>("export_recipes"),
  saveExportRecipe: (recipe: ExportRecipe) => invoke<ExportRecipe>("save_export_recipe", { recipe }),
  deleteExportPreset: (id: string) => invoke<boolean>("delete_export_preset", { id }),
  exportPlans: () => invoke<ExportPlan[]>("export_plans"),
  saveExportPlan: (plan: ExportPlan) => invoke<ExportPlan>("save_export_plan", { plan }),
  inspectIccProfile: (path: string) => invoke<string>("inspect_icc_profile", { path }),
  estimateExport: (request: ExportRequest) => invoke<ExportEstimate>("estimate_export", { request }),
  startExport: (request: ExportRequest) => invoke<number>("start_export", { request }),
  exportJobs: () => invoke<ExportProgress[]>("export_jobs"),
  pauseExport: (id: number, paused: boolean) => invoke<boolean>("pause_export", { id, paused }),
  cancelExport: (id: number) => invoke<boolean>("cancel_export", { id }),
  clearFinishedExports: () => invoke<void>("clear_finished_exports"),
  revealFolder: (path: string) => invoke<void>("reveal_folder", { path }),
  // Galeria.
  galleryConfig: () => invoke<GalleryConfig>("gallery_config"),
  saveGalleryConfig: (config: GalleryConfig) => invoke<void>("save_gallery_config", { config }),
  galleryPreview: (config: GalleryConfig, momentNames: Record<number, string>) => invoke<GalleryPreview>("gallery_preview", { config, momentNames }),
  startGallery: (config: GalleryConfig, momentNames: Record<number, string>, replace: boolean) => invoke<number>("start_gallery", { config, momentNames, replace }),
  setGalleryHidden: (keys: number[], hidden: boolean) => invoke<GalleryConfig>("set_gallery_hidden", { keys, hidden }),
  importClientFavorites: (text: string) => invoke<FavoritesImport>("import_client_favorites", { text }),
  openGallery: (path: string) => invoke<void>("open_gallery", { path }),
  // Álbuns.
  albumFormats: () => invoke<AlbumFormat[]>("album_formats"),
  albumTemplates: (count: number) => invoke<string[]>("album_templates", { count }),
  listAlbums: () => invoke<AlbumSummary[]>("list_albums"),
  getAlbum: (id: string) => invoke<Album>("get_album", { id }),
  createAlbum: (name: string, spec: AlbumSpec) => invoke<Album>("create_album", { name, spec }),
  saveAlbum: (album: Album) => invoke<void>("save_album", { album }),
  deleteAlbum: (id: string) => invoke<boolean>("delete_album", { id }),
  autoAlbum: (id: string, options: { maxPerPage: number; marginMm: number; spacingMm: number; keepPages: boolean }) => invoke<AlbumAutoReport>("auto_album", { id, options }),
  applyAlbumLayout: (album: Album, spread: number, template: string, margin: number, spacing: number, extra: number[]) =>
    invoke<Album>("apply_album_layout", { album, spread, template, margin, spacing, extra }),
  albumPreflight: (album: Album) => invoke<AlbumIssue[]>("album_preflight", { album }),
  albumPhotoInfos: () => invoke<AlbumPhotoInfo[]>("album_photo_infos"),
  startAlbumExport: (id: string, kind: AlbumExportKind, destination: string) => invoke<number>("start_album_export", { id, kind, destination }),
  labPresets: () => invoke<LabPreset[]>("lab_presets"),
  saveLabPreset: (preset: LabPreset) => invoke<LabPreset>("save_lab_preset", { preset }),
  deleteLabPreset: (id: string) => invoke<boolean>("delete_lab_preset", { id }),
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
  editsChanged: (cb: (keys: number[]) => void): Promise<UnlistenFn> => listen<number[]>("edits-changed", (e) => cb(e.payload)),
  autoEdit: (cb: (p: AutoEditProgress) => void): Promise<UnlistenFn> => listen<AutoEditProgress>("auto-edit-progress", (e) => cb(e.payload)),
  aiDenoise: (cb: (p: AiDenoiseProgress) => void): Promise<UnlistenFn> => listen<AiDenoiseProgress>("ai-denoise", (e) => cb(e.payload)),
  exportProgress: (cb: (p: ExportProgress) => void): Promise<UnlistenFn> => listen<ExportProgress>("export-progress", (e) => cb(e.payload)),
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
