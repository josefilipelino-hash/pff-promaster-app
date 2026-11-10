import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, imageUrl } from "../api";
import { formatBytes } from "../logic/format";
import { errorText, useStore } from "../store";
import { ANCHORS, GALLERY_THEMES, type FavoritesImport, type GalleryConfig, type GalleryPreview, type GallerySettings } from "../types";
import { Modal } from "./Modal";
import { momentName } from "./MomentsPanel";

type Tab = "content" | "look" | "features" | "publish";
const TABS: Tab[] = ["content", "look", "features", "publish"];

/** Cores de cada tema (amostra no seletor; iguais ao CSS da galeria). */
const SWATCH: Record<string, [string, string, string]> = {
  light: ["#faf7f2", "#2a2622", "#a8875a"],
  dark: ["#121110", "#efe9e1", "#c9a96e"],
  minimal: ["#ffffff", "#111111", "#111111"],
  editorial: ["#f2ede6", "#1d1b18", "#8c6b4a"],
};

/** Galeria do projeto: textos, capa/hero/logótipo, tema, funcionalidades, PIN e geração. */
export function GalleryDialog({ selected, focus, onClose }: { selected: number[]; focus: number | null; onClose: () => void }) {
  const { t } = useTranslation();
  const toast = useStore((s) => s.toast);
  const moments = useStore((s) => s.moments);
  const project = useStore((s) => s.project);
  const upsert = useStore((s) => s.upsertExportJob);
  const [cfg, setCfg] = useState<GalleryConfig | null>(null);
  const [tab, setTab] = useState<Tab>("content");
  const [preview, setPreview] = useState<GalleryPreview | null>(null);
  const [confirmReplace, setConfirmReplace] = useState(false);
  const [busy, setBusy] = useState(false);
  const [favText, setFavText] = useState("");
  const [favResult, setFavResult] = useState<FavoritesImport | null>(null);
  const fail = (e: unknown) => {
    // Problemas de configuração vêm do núcleo como chaves i18n.
    const m = /gallery\.problem\.\w+/.exec(errorText(e, t));
    toast("error", m ? t(m[0]) : errorText(e, t));
  };
  const momentNames = useMemo(() => Object.fromEntries(moments.map((m) => [m.id, momentName(m, t)])), [moments, t]);

  useEffect(() => {
    api.galleryConfig().then(setCfg).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Antevisão e gravação automática (as definições ficam no catálogo do projeto).
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => {
    if (!cfg) return;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      api.saveGalleryConfig(cfg).catch(() => {});
      api.galleryPreview(cfg, momentNames).then(setPreview).catch(() => setPreview(null));
    }, 300);
    return () => window.clearTimeout(timer.current);
  }, [cfg, momentNames]);

  if (!cfg) {
    return (
      <Modal title={t("gallery.title")} onClose={onClose}>
        <p className="muted">{t("gallery.loading")}</p>
      </Modal>
    );
  }
  const s = cfg.settings;
  const set = (p: Partial<GallerySettings>) => setCfg({ ...cfg, settings: { ...s, ...p } });
  const wm = (p: Partial<GalleryConfig["watermark"]>) => setCfg({ ...cfg, watermark: { ...cfg.watermark, ...p } });

  const hide = async (hidden: boolean) => {
    try {
      const next = await api.setGalleryHidden(selected, hidden);
      setCfg({ ...cfg, settings: { ...s, hidden: next.settings.hidden } });
      toast("info", t(hidden ? "gallery.hiddenToast" : "gallery.shownToast", { count: selected.length }));
    } catch (e) {
      fail(e);
    }
  };

  const pickDest = async () => {
    const dir = await open({ directory: true, multiple: false, title: t("gallery.destination") });
    if (typeof dir === "string") setCfg({ ...cfg, destination: dir });
  };
  const pickLogo = async () => {
    const path = await open({ multiple: false, filters: [{ name: t("gallery.logoFilter"), extensions: ["png", "svg", "jpg", "jpeg", "webp"] }] });
    if (typeof path === "string") set({ logoPath: path });
  };

  const generate = async (replace: boolean) => {
    setBusy(true);
    try {
      await api.saveGalleryConfig(cfg);
      const id = await api.startGallery(cfg, momentNames, replace);
      const job = (await api.exportJobs()).find((j) => j.id === id);
      if (job) upsert(job);
      toast("info", t("gallery.started", { count: preview?.photos ?? 0 }));
      onClose();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
      setConfirmReplace(false);
    }
  };

  const importFavs = async () => {
    try {
      const r = await api.importClientFavorites(favText);
      setFavResult(r);
      await useStore.getState().reloadPhotos();
    } catch (e) {
      fail(e);
    }
  };

  const problems = preview?.problems ?? [];
  const thumb = (key: number | null) =>
    key != null && project ? <img className="gallery-pick" src={imageUrl(project.imageToken, "thumb", key)} alt="" /> : <span className="gallery-pick empty" />;

  return (
    <Modal
      title={t("gallery.title")}
      onClose={onClose}
      wide
      footer={
        <>
          <span className="export-summary small" role="status">
            {preview
              ? t("gallery.summary", { photos: preview.photos, chapters: preview.chapters.length, size: formatBytes(preview.estimateBytes) })
              : ""}
            {problems.length > 0 && <strong className="warn"> · {t(problems[0]!)}</strong>}
          </span>
          {confirmReplace ? (
            <>
              <span className="small warn">{t("gallery.replaceAsk")}</span>
              <button type="button" className="btn" onClick={() => setConfirmReplace(false)}>
                {t("common.cancel")}
              </button>
              <button type="button" className="btn primary" disabled={busy} onClick={() => void generate(true)}>
                {t("gallery.replace")}
              </button>
            </>
          ) : (
            <>
              <button type="button" className="btn" onClick={onClose} data-close>
                {t("common.close")}
              </button>
              <button type="button" className="btn primary" disabled={busy || problems.length > 0} onClick={() => (preview?.exists ? setConfirmReplace(true) : void generate(false))}>
                {t("gallery.generate", { count: preview?.photos ?? 0 })}
              </button>
            </>
          )}
        </>
      }
    >
      <div className="gallery-tabs" role="tablist" aria-label={t("gallery.title")}>
        {TABS.map((k) => (
          <button key={k} type="button" role="tab" aria-selected={tab === k} className={tab === k ? "active" : ""} onClick={() => setTab(k)}>
            {t(`gallery.tab.${k}`)}
          </button>
        ))}
      </div>
      <div className="gallery-body" role="tabpanel" aria-label={t(`gallery.tab.${tab}`)}>
        {tab === "content" && (
          <div className="export-grid">
            <label className="field compact span2">
              <span>{t("gallery.couple")}</span>
              <input value={s.couple} onChange={(e) => set({ couple: e.target.value })} aria-label={t("gallery.couple")} />
            </label>
            <label className="field compact">
              <span>{t("gallery.date")}</span>
              <input type="date" value={s.date} onChange={(e) => set({ date: e.target.value })} aria-label={t("gallery.date")} />
            </label>
            <label className="field compact">
              <span>{t("gallery.location")}</span>
              <input value={s.location} onChange={(e) => set({ location: e.target.value })} aria-label={t("gallery.location")} />
            </label>
            <label className="field compact span-all">
              <span>{t("gallery.intro")}</span>
              <textarea rows={4} value={s.intro} onChange={(e) => set({ intro: e.target.value })} aria-label={t("gallery.intro")} placeholder={t("gallery.introHint")} />
            </label>
            <label className="field compact">
              <span>{t("gallery.selection")}</span>
              <select value={s.selection} onChange={(e) => set({ selection: e.target.value as GallerySettings["selection"] })} aria-label={t("gallery.selection")}>
                {(["picks", "notRejected", "stars", "all"] as const).map((x) => (
                  <option key={x} value={x}>
                    {t(`gallery.selectionOption.${x}`)}
                  </option>
                ))}
              </select>
            </label>
            {s.selection === "stars" && (
              <label className="field compact">
                <span>{t("gallery.minStars")}</span>
                <input type="number" min={1} max={5} value={s.minStars} onChange={(e) => set({ minStars: Number(e.target.value) })} aria-label={t("gallery.minStars")} />
              </label>
            )}
            <label className="field compact">
              <span>{t("gallery.chapters")}</span>
              <select value={s.chapters} onChange={(e) => set({ chapters: e.target.value as GallerySettings["chapters"] })} aria-label={t("gallery.chapters")}>
                {(["grouped", "detailed", "none"] as const).map((x) => (
                  <option key={x} value={x}>
                    {t(`gallery.chaptersOption.${x}`)}
                  </option>
                ))}
              </select>
            </label>
            <div className="span-all">
              <ol className="gallery-chapters" aria-label={t("gallery.chapterList")}>
                {(preview?.chapters ?? []).map((c, i) => (
                  <li key={i}>
                    <span>{c.title || t("gallery.single")}</span>
                    <span className="muted">{c.count}</span>
                  </li>
                ))}
              </ol>
            </div>
            <div className="span-all dev-row wrap">
              <span className="small">{t("gallery.hiddenCount", { count: s.hidden.length })}</span>
              <button type="button" className="btn small" disabled={selected.length === 0} onClick={() => void hide(true)}>
                {t("gallery.hideSelection", { count: selected.length })}
              </button>
              <button type="button" className="btn small" disabled={selected.length === 0} onClick={() => void hide(false)}>
                {t("gallery.showSelection", { count: selected.length })}
              </button>
              <button type="button" className="btn small" disabled={s.hidden.length === 0} onClick={() => set({ hidden: [] })}>
                {t("gallery.showAll")}
              </button>
            </div>
          </div>
        )}

        {tab === "look" && (
          <>
            <div className="theme-cards" role="radiogroup" aria-label={t("gallery.theme")}>
              {GALLERY_THEMES.map((th) => {
                const [bg, fg, ac] = SWATCH[th]!;
                return (
                  <button key={th} type="button" role="radio" aria-checked={s.theme === th} className={`theme-card${s.theme === th ? " active" : ""}`} onClick={() => set({ theme: th })}>
                    <span className={`theme-sample ${th}`} style={{ background: bg, color: fg }}>
                      <span className="ts-names">Ana &amp; Pedro</span>
                      <span className="ts-line" style={{ background: ac }} />
                    </span>
                    <strong>{t(`gallery.themeName.${th}`)}</strong>
                    <span className="small muted">{t(`gallery.themeHint.${th}`)}</span>
                  </button>
                );
              })}
            </div>
            <div className="export-grid">
              <div className="field compact">
                <span>{t("gallery.hero")}</span>
                <div className="dev-row">
                  {thumb(s.hero)}
                  <button type="button" className="btn small" disabled={focus == null} onClick={() => set({ hero: focus })}>
                    {t("gallery.useFocus")}
                  </button>
                  {s.hero != null && (
                    <button type="button" className="btn ghost small" onClick={() => set({ hero: null })}>
                      {t("gallery.remove")}
                    </button>
                  )}
                </div>
              </div>
              <div className="field compact">
                <span>{t("gallery.cover")}</span>
                <div className="dev-row">
                  {thumb(s.cover)}
                  <button type="button" className="btn small" disabled={focus == null} onClick={() => set({ cover: focus })}>
                    {t("gallery.useFocus")}
                  </button>
                  {s.cover != null && (
                    <button type="button" className="btn ghost small" onClick={() => set({ cover: null })}>
                      {t("gallery.remove")}
                    </button>
                  )}
                </div>
              </div>
              <div className="field compact">
                <span>{t("gallery.logo")}</span>
                <div className="dev-row">
                  <span className="small ellipsis grow">{s.logoPath || t("gallery.noLogo")}</span>
                  <button type="button" className="btn small" onClick={() => void pickLogo()}>
                    {t("export.choose")}
                  </button>
                  {s.logoPath && (
                    <button type="button" className="btn ghost small" onClick={() => set({ logoPath: "" })}>
                      {t("gallery.remove")}
                    </button>
                  )}
                </div>
              </div>
              <label className="field compact">
                <span>{t("gallery.layout")}</span>
                <select value={s.layout} onChange={(e) => set({ layout: e.target.value as GallerySettings["layout"] })} aria-label={t("gallery.layout")}>
                  <option value="masonry">{t("gallery.layoutMasonry")}</option>
                  <option value="grid">{t("gallery.layoutGrid")}</option>
                </select>
              </label>
              <label className="field compact">
                <span>{t("gallery.language")}</span>
                <select value={s.language} onChange={(e) => set({ language: e.target.value as "pt" | "en" })} aria-label={t("gallery.language")}>
                  <option value="pt">Português</option>
                  <option value="en">English</option>
                </select>
              </label>
            </div>
            <p className="small muted">{t("gallery.heroHint")}</p>
          </>
        )}

        {tab === "features" && (
          <div className="export-grid">
            <label className="check span-all">
              <input type="checkbox" checked={s.favorites} onChange={(e) => set({ favorites: e.target.checked })} /> {t("gallery.favorites")}
            </label>
            <label className="field compact">
              <span>{t("gallery.downloads")}</span>
              <select value={s.downloads} onChange={(e) => set({ downloads: e.target.value as GallerySettings["downloads"] })} aria-label={t("gallery.downloads")}>
                {(["none", "individual", "all"] as const).map((x) => (
                  <option key={x} value={x}>
                    {t(`gallery.downloadsOption.${x}`)}
                  </option>
                ))}
              </select>
            </label>
            {s.downloads !== "none" && (
              <label className="field compact">
                <span>{t("gallery.downloadSize")}</span>
                <select value={s.downloadSize} onChange={(e) => set({ downloadSize: e.target.value as "web" | "high" })} aria-label={t("gallery.downloadSize")}>
                  <option value="high">{t("gallery.sizeHigh")}</option>
                  <option value="web">{t("gallery.sizeWeb")}</option>
                </select>
              </label>
            )}
            {s.downloads === "all" && (
              <label className="check">
                <input type="checkbox" checked={s.chapterZips} onChange={(e) => set({ chapterZips: e.target.checked })} /> {t("gallery.chapterZips")}
              </label>
            )}
            <label className="field compact">
              <span>{t("gallery.slideshow")}</span>
              <input type="number" min={2} max={20} value={s.slideshowSeconds} onChange={(e) => set({ slideshowSeconds: Number(e.target.value) })} aria-label={t("gallery.slideshow")} />
            </label>
            <h4 className="span-all">{t("gallery.protection")}</h4>
            <label className="field compact">
              <span>{t("gallery.protectionMode")}</span>
              <select value={s.protection} onChange={(e) => set({ protection: e.target.value as GallerySettings["protection"] })} aria-label={t("gallery.protectionMode")}>
                {(["none", "pin", "password"] as const).map((x) => (
                  <option key={x} value={x}>
                    {t(`gallery.protectionOption.${x}`)}
                  </option>
                ))}
              </select>
            </label>
            {s.protection !== "none" && (
              <label className="field compact">
                <span>{t(s.protection === "pin" ? "gallery.pin" : "gallery.password")}</span>
                <input
                  value={s.secret}
                  inputMode={s.protection === "pin" ? "numeric" : undefined}
                  maxLength={s.protection === "pin" ? 8 : undefined}
                  onChange={(e) => set({ secret: s.protection === "pin" ? e.target.value.replace(/\D/g, "") : e.target.value })}
                  aria-label={t(s.protection === "pin" ? "gallery.pin" : "gallery.password")}
                />
              </label>
            )}
            {s.protection !== "none" && <p className="small muted span-all">{t(`gallery.protectionHint.${s.protection}`)}</p>}
            <h4 className="span-all">{t("export.section.watermark")}</h4>
            <label className="check span-all">
              <input type="checkbox" checked={cfg.watermark.enabled} onChange={(e) => wm({ enabled: e.target.checked, kind: "text" })} /> {t("gallery.watermark")}
            </label>
            {cfg.watermark.enabled && (
              <>
                <label className="field compact">
                  <span>{t("export.watermark.text")}</span>
                  <input value={cfg.watermark.text} onChange={(e) => wm({ text: e.target.value })} aria-label={t("export.watermark.text")} />
                </label>
                <label className="field compact">
                  <span>{t("export.watermark.opacity")}</span>
                  <input type="number" min={0} max={100} value={cfg.watermark.opacity} onChange={(e) => wm({ opacity: Number(e.target.value) })} aria-label={t("export.watermark.opacity")} />
                </label>
                <label className="field compact">
                  <span>{t("export.watermark.size")}</span>
                  <input type="number" min={0.5} max={50} step={0.5} value={cfg.watermark.size} onChange={(e) => wm({ size: Number(e.target.value) })} aria-label={t("export.watermark.size")} />
                </label>
                <label className="field compact">
                  <span>{t("export.watermark.position")}</span>
                  <select value={cfg.watermark.anchor} onChange={(e) => wm({ anchor: e.target.value as GalleryConfig["watermark"]["anchor"] })} aria-label={t("export.watermark.position")}>
                    {ANCHORS.map((a) => (
                      <option key={a} value={a}>
                        {t(`export.anchor.${a}`)}
                      </option>
                    ))}
                  </select>
                </label>
              </>
            )}
            <h4 className="span-all">{t("gallery.contact")}</h4>
            <label className="field compact">
              <span>{t("gallery.photographer")}</span>
              <input value={s.photographer} onChange={(e) => set({ photographer: e.target.value })} aria-label={t("gallery.photographer")} />
            </label>
            <label className="field compact">
              <span>{t("gallery.email")}</span>
              <input type="email" value={s.contactEmail} onChange={(e) => set({ contactEmail: e.target.value })} aria-label={t("gallery.email")} />
            </label>
          </div>
        )}

        {tab === "publish" && (
          <>
            <label className="field compact">
              <span>{t("gallery.destination")}</span>
              <div className="dev-row">
                <input className="grow" readOnly value={cfg.destination} placeholder={t("export.destinationNone")} aria-label={t("gallery.destination")} />
                <button type="button" className="btn small" onClick={() => void pickDest()}>
                  {t("export.choose")}
                </button>
              </div>
            </label>
            {preview && cfg.destination && (
              <p className="small">
                {t("gallery.folder")}: <code>{preview.folder}</code>
                {preview.exists && <span className="warn"> · {t("gallery.existsNote")}</span>}
              </p>
            )}
            <p className="small muted">{t("gallery.publishInfo")}</p>
            <h4>{t("gallery.providers")}</h4>
            <ul className="gallery-providers">
              {["s3", "r2", "b2", "sftp"].map((p) => (
                <li key={p}>
                  <strong>{t(`gallery.provider.${p}`)}</strong>
                  <span className="badge">{t("gallery.providerLater")}</span>
                </li>
              ))}
            </ul>
            <p className="small muted">{t("gallery.providersInfo")}</p>
            <h4>{t("gallery.clientFavorites")}</h4>
            <p className="small muted">{t("gallery.clientFavoritesInfo")}</p>
            <textarea className="gallery-fav" rows={4} value={favText} onChange={(e) => setFavText(e.target.value)} aria-label={t("gallery.clientFavorites")} placeholder="IMG_0001&#10;IMG_0042" />
            <div className="dev-row">
              <button type="button" className="btn small" disabled={!favText.trim()} onClick={() => void importFavs()}>
                {t("gallery.importFavorites")}
              </button>
              {favResult && (
                <span className="small" role="status">
                  {t("gallery.importResult", { count: favResult.matched })}
                  {favResult.unmatched.length > 0 && <span className="warn"> · {t("gallery.importUnmatched", { names: favResult.unmatched.slice(0, 5).join(", ") })}</span>}
                </span>
              )}
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
