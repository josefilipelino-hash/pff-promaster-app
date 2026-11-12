// API simulada para a auditoria visual (audit/run.mjs): substitui src/api.ts no browser.
// Volume realista: 89 fotografias (horizontais e verticais), grupos de luz grandes, pessoas.
import { apiMock, project, rows, editViewOf } from "./fixtures";
import type { PhotoRow } from "../src/types";
export { imageUrl, isMac } from "../src/api";
export type { ImageLevel } from "../src/api";

const N = 89;
const photos: PhotoRow[] = rows(N).map((r, i) => ({
  ...r,
  width: i % 5 === 3 ? 4000 : 6000,
  height: i % 5 === 3 ? 6000 : 4000,
  captureTime: `2026-10-28T15:${String(59 + Math.floor(i / 20)).padStart(2, "0")}:${String((i * 7) % 60).padStart(2, "0")}`,
  edited: i < 5,
  flag: 0,
  partial: false,
  format: "cr3",
  fileName: `JI_C${2900 + i}.CR3`,
}));
const audited = {
  ...project,
  name: "Chão de fábrica Fazendas",
  clientName: "Fazendas",
  eventDate: "2026-10-28",
  photoCount: N,
};
const status = (a: number, total: number) => ({
  running: false,
  counts: { total, analyzed: a, failed: 0, pending: total - a },
  modelsAvailable: true,
  settings: { mode: "strict", target: null },
  summary: { best: 40, alternative: 7, review: 16, reject: 0, duplicate: 26, groups: 12, sequences: 8, durationMs: 1 },
});

Object.assign(apiMock, {
  recentProjects: async () => [
    { path: audited.path, name: audited.name, clientName: audited.clientName, eventType: "corporate", eventDate: audited.eventDate, location: "Fazendas", photoCount: N, lastOpened: Date.now(), available: true },
    { path: "/trabalho/casamento.momento", name: "Casamento Vera & Manuel — 25 Anos", clientName: "Vera & Manuel", eventType: "wedding", eventDate: "2026-11-02", location: "Lisboa", photoCount: 2412, lastOpened: Date.now() - 86400000, available: true },
  ],
  openProject: async () => audited,
  listPhotos: async () => photos,
  analysisStatus: async () => status(N, N),
  consistencyGroups: async () => ({
    groups: [
      { id: "combined:0", code: "lighting.bright", tags: ["flash.off", "wb.neutral"], detail: "", moment: null, start: "2026-10-28T15:59:00", end: "2026-10-28T16:10:00", ids: photos.slice(0, 86).map((p) => p.id) },
      { id: "combined:1", code: "lighting.bright", tags: ["flash.off", "wb.neutral"], detail: "", moment: null, start: "2026-10-28T16:37:00", end: "2026-10-28T16:38:00", ids: photos.slice(86).map((p) => p.id) },
    ],
    missing: N,
  }),
  editView: async (key: number) => editViewOf(key),
});

export const api = apiMock;
const noop = async () => () => {};
export const events = {
  importProgress: noop,
  importFinished: noop,
  thumbnails: noop,
  analysis: noop,
  editsChanged: noop,
  autoEdit: noop,
  aiDenoise: noop,
  exportProgress: noop,
  availability: noop,
};
