import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, events, isMac } from "../api";
import { applyFilter, collectionCounts } from "../logic/filters";
import { clickSelect, moveFocus, restrict, selectAll, targets } from "../logic/selection";
import { keyToAction, type Action } from "../logic/shortcuts";
import { errorText, useStore } from "../store";
import type { Color, ImportMode } from "../types";
import { HelpOverlay } from "./HelpOverlay";
import { ImportDialog, ImportReport } from "./ImportDialogs";
import { InfoPanel } from "./InfoPanel";
import { PhotoGrid, type GridLayout } from "./PhotoGrid";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";
import { Toolbar } from "./Toolbar";
import { Viewer, type ViewerHandle } from "./Viewer";

/** Extensões oferecidas no seletor de ficheiros (a deteção real é feita pelo conteúdo). */
const IMAGE_EXTENSIONS = ["jpg", "jpeg", "jpe", "png", "tif", "tiff", "JPG", "JPEG", "PNG", "TIF", "TIFF"];

function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "SELECT" || el.tagName === "TEXTAREA" || el.isContentEditable);
}

export function ProjectScreen({ onClosed }: { onClosed: () => void }) {
  const { t } = useTranslation();
  const s = useStore();
  const project = s.project!;
  const layoutRef = useRef<GridLayout>({ columns: 1, cell: 180, rowHeight: 212 });
  const viewerRef = useRef<ViewerHandle | null>(null);
  const [dragging, setDragging] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const mac = isMac();

  const visible = useMemo(() => applyFilter(s.photos, s.filter), [s.photos, s.filter]);
  const order = useMemo(() => visible.map((p) => p.id), [visible]);
  const counts = useMemo(() => collectionCounts(s.photos), [s.photos]);
  const focusPhoto = s.selection.focus !== null ? (s.photos[s.index.get(s.selection.focus) ?? -1] ?? null) : null;
  const viewerIndex = s.viewerId !== null ? order.indexOf(s.viewerId) : -1;
  const viewerPhoto = viewerIndex >= 0 ? visible[viewerIndex]! : null;

  const fail = useCallback((e: unknown) => s.toast("error", errorText(e, t)), [s, t]);

  // Carregar fotografias ao abrir.
  useEffect(() => {
    s.reloadPhotos().catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project.path]);

  // Ao mudar o filtro, a seleção mantém apenas fotografias visíveis.
  useEffect(() => {
    const vis = new Set(order);
    const sel = useStore.getState().selection;
    if ([...sel.ids].some((i) => !vis.has(i)) || (sel.focus !== null && !vis.has(sel.focus))) s.setSelection(restrict(sel, vis));
    if (s.viewerId !== null && !vis.has(s.viewerId)) s.openViewer(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [order]);

  // Eventos do núcleo.
  useEffect(() => {
    let lastReload = 0;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const scheduleReload = () => {
      if (timer) return;
      const wait = Math.max(0, 1500 - (Date.now() - lastReload));
      timer = setTimeout(() => {
        timer = null;
        lastReload = Date.now();
        useStore.getState().reloadPhotos().catch(() => {});
      }, wait);
    };
    const un = [
      events.importProgress((p) => {
        const st = useStore.getState();
        st.setImportRunning(true);
        st.setImportProgress(p);
        if (p.imported > 0) scheduleReload();
      }),
      events.importFinished((sum) => {
        const st = useStore.getState();
        st.setImportRunning(false);
        st.setImportProgress(null);
        st.reloadPhotos().catch(() => {});
        if (sum.issues.length > 0 || sum.cancelled || sum.error || sum.counts.pending > 0) st.setImportSummary(sum);
        else st.toast("info", t("report.toast", { count: sum.counts.imported }));
      }),
      events.thumbnails((b) => useStore.getState().applyThumbBatch(b)),
      getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") setDragging(true);
        else if (p.type === "leave") setDragging(false);
        else if (p.type === "drop") {
          setDragging(false);
          if (p.paths.length > 0 && !useStore.getState().importRunning) useStore.getState().setPendingImport({ paths: p.paths, hasFolders: true });
          else if (useStore.getState().importRunning) useStore.getState().toast("info", t("import.busy"));
        }
      }),
    ];
    return () => {
      if (timer) clearTimeout(timer);
      un.forEach((p) => p.then((f) => f()).catch(() => {}));
    };
  }, [t]);

  // Guardar o estado da interface no catálogo (com atraso para agrupar alterações).
  const uiJson = JSON.stringify({ filter: s.filter, thumbSize: s.thumbSize, focusId: s.selection.focus, infoOpen: s.infoOpen });
  useEffect(() => {
    const h = setTimeout(() => api.saveUiState(uiJson).catch(() => {}), 500);
    return () => clearTimeout(h);
  }, [uiJson]);

  const rate = useCallback(
    (kind: "stars" | "flag" | "color", value: number | Color | null, ids?: number[]) => {
      const st = useStore.getState();
      const list = ids ?? (st.viewerId !== null ? [st.viewerId] : targets(st.selection));
      if (list.length === 0) return;
      st.rate(kind, value, list).catch(fail);
    },
    [fail],
  );

  const toggleFullscreen = useCallback(async () => {
    try {
      const w = getCurrentWindow();
      const next = !(await w.isFullscreen());
      await w.setFullscreen(next);
      setFullscreen(next);
    } catch (e) {
      fail(e);
    }
  }, [fail]);

  const chooseImport = async (folder: boolean) => {
    const sel = await open(
      folder
        ? { directory: true, multiple: true, title: t("import.folder") }
        : { directory: false, multiple: true, title: t("import.files"), filters: [{ name: t("import.imagesFilter"), extensions: IMAGE_EXTENSIONS }] },
    );
    const paths = Array.isArray(sel) ? sel : typeof sel === "string" ? [sel] : [];
    if (paths.length > 0) s.setPendingImport({ paths, hasFolders: folder });
  };

  const startImport = async (mode: ImportMode, recursive: boolean) => {
    const pending = s.pendingImport;
    s.setPendingImport(null);
    if (!pending) return;
    try {
      await api.startImport(pending.paths, mode, recursive);
      s.setImportRunning(true);
    } catch (e) {
      fail(e);
    }
  };

  const closeProject = async () => {
    try {
      await api.closeProject();
    } catch (e) {
      fail(e);
    }
    if (fullscreen) await getCurrentWindow().setFullscreen(false).catch(() => {});
    s.setProject(null);
    onClosed();
  };

  const onClickPhoto = useCallback(
    (id: number, e: React.MouseEvent) => {
      const st = useStore.getState();
      const toggle = mac ? e.metaKey : e.ctrlKey;
      st.setSelection(clickSelect(st.selection, order, id, { toggle, range: e.shiftKey }));
    },
    [order, mac],
  );

  // Teclado.
  const handleAction = useCallback(
    (a: Action) => {
      const st = useStore.getState();
      switch (a.type) {
        case "stars":
          return rate("stars", a.value);
        case "flag":
          return rate("flag", a.value);
        case "color": {
          const cur = st.viewerId !== null ? st.photos[st.index.get(st.viewerId)!] : focusPhoto;
          return rate("color", cur?.color === a.value ? null : a.value);
        }
        case "move":
          if (st.viewerId !== null) {
            if (a.dx !== 0) {
              const i = order.indexOf(st.viewerId) + a.dx;
              if (i >= 0 && i < order.length) st.openViewer(order[i]!);
            }
            return;
          }
          return st.setSelection(moveFocus(st.selection, order, a.dx + a.dy * layoutRef.current.columns, a.extend));
        case "open":
          if (st.viewerId === null && st.selection.focus !== null) st.openViewer(st.selection.focus);
          else if (st.viewerId === null && order.length > 0) st.openViewer(order[0]!);
          return;
        case "close":
          if (fullscreen) return void toggleFullscreen();
          if (st.viewerId !== null) st.openViewer(null);
          return;
        case "toggleZoom":
          if (st.viewerId !== null) viewerRef.current?.toggleZoom();
          else if (st.selection.focus !== null) st.openViewer(st.selection.focus);
          return;
        case "zoom":
          if (st.viewerId !== null) viewerRef.current?.zoom(a.dir);
          else st.setThumbSize(st.thumbSize + a.dir * 20);
          return;
        case "fullscreen":
          return void toggleFullscreen();
        case "info":
          return st.toggleInfo();
        case "selectAll":
          return st.setSelection(selectAll(order, st.selection.focus));
        case "import":
          return void chooseImport(false);
        case "help":
          return st.setHelpOpen(true);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [order, rate, focusPhoto, fullscreen, toggleFullscreen],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const st = useStore.getState();
      if (isTyping(e.target) || st.pendingImport || st.importSummary || st.helpOpen) return;
      const a = keyToAction(e, mac);
      if (!a) return;
      e.preventDefault();
      handleAction(a);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [handleAction, mac]);

  const immersive = fullscreen && viewerPhoto !== null;

  return (
    <div className={`project${immersive ? " immersive" : ""}${s.infoOpen ? "" : " no-info"}`}>
      {!immersive && <Sidebar active={s.filter.collection} counts={counts} onSelect={(c) => s.setFilter({ collection: c })} />}
      <section className="main">
        {!immersive && (
          <Toolbar
            project={project}
            importBusy={s.importRunning}
            onImportFiles={() => void chooseImport(false)}
            onImportFolder={() => void chooseImport(true)}
            onClose={() => void closeProject()}
            onHelp={() => s.setHelpOpen(true)}
          />
        )}
        <div className="content">
          {s.photos.length === 0 ? (
            <div className="empty-state">
              <h2>{t("project.emptyTitle")}</h2>
              <p className="muted">{t("project.emptyHint")}</p>
              <div className="home-actions">
                <button type="button" className="btn primary" disabled={s.importRunning} onClick={() => void chooseImport(true)}>
                  {t("import.folder")}
                </button>
                <button type="button" className="btn" disabled={s.importRunning} onClick={() => void chooseImport(false)}>
                  {t("import.files")}
                </button>
              </div>
            </div>
          ) : visible.length === 0 ? (
            <div className="empty-state">
              <p className="muted">{t("project.noMatches")}</p>
            </div>
          ) : (
            <PhotoGrid photos={visible} onClickPhoto={onClickPhoto} onOpen={(id) => s.openViewer(id)} layoutRef={layoutRef} />
          )}
          {viewerPhoto && (
            <Viewer
              photo={viewerPhoto}
              token={project.imageToken}
              index={viewerIndex}
              total={order.length}
              onPrev={() => viewerIndex > 0 && s.openViewer(order[viewerIndex - 1]!)}
              onNext={() => viewerIndex < order.length - 1 && s.openViewer(order[viewerIndex + 1]!)}
              onClose={() => s.openViewer(null)}
              onFullscreen={() => void toggleFullscreen()}
              handleRef={viewerRef}
            />
          )}
          {dragging && <div className="drop-overlay">{t("import.dropHere")}</div>}
        </div>
        {!immersive && <StatusBar total={s.photos.length} visible={visible.length} selected={s.selection.ids.size} onCancelImport={() => api.cancelImport().catch(fail)} showSize={viewerPhoto === null} />}
      </section>
      {s.infoOpen && !immersive && <InfoPanel photo={viewerPhoto ?? focusPhoto} selectedCount={viewerPhoto ? 1 : s.selection.ids.size} onRate={(k, v) => rate(k, v)} />}

      {s.pendingImport && <ImportDialog paths={s.pendingImport.paths} originalsDir={project.originalsDir} onCancel={() => s.setPendingImport(null)} onStart={(m, r) => void startImport(m, r)} />}
      {s.importSummary && <ImportReport summary={s.importSummary} onClose={() => s.setImportSummary(null)} />}
      {s.helpOpen && <HelpOverlay onClose={() => s.setHelpOpen(false)} />}
    </div>
  );
}
