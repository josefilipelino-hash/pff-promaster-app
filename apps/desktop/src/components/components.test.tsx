import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import "../i18n";
import type { PhotoRow, ProjectInfo } from "../types";

// --- Simulação do núcleo e das APIs Tauri ---------------------------------------------
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
  cancelImport: vi.fn(async () => {}),
  prioritizeThumbnails: vi.fn(async () => {}),
  saveUiState: vi.fn(async () => {}),
};
vi.mock("../api", async (orig) => {
  const real = await orig<typeof import("../api")>();
  const noop = async () => () => {};
  return { ...real, api: apiMock, events: { importProgress: noop, importFinished: noop, thumbnails: noop } };
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
    fileName: `IMG_${String(i + 1).padStart(5, "0")}.jpg`,
    captureTime: null,
    width: 6000,
    height: 4000,
    stars: 0,
    flag: i === 1 ? 1 : 0,
    color: null,
    thumb: "ready" as const,
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

  it("secções futuras estão desativadas e identificadas", async () => {
    await openProject(5);
    for (const name of [/Momentos/, /Pessoas/, /Exportação/]) {
      const b = screen.getByRole("button", { name }) as HTMLButtonElement;
      expect(b.disabled).toBe(true);
    }
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

describe("cálculos de layout", () => {
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
