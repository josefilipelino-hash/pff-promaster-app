import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { formatDateTimeMillis, formatEventDate } from "../logic/format";
import { errorText, useStore } from "../store";
import type { AppInfo, ProjectInfo, RecentProject } from "../types";
import { NewProjectDialog } from "./NewProjectDialog";

export function HomeScreen({ onOpened }: { onOpened: (p: ProjectInfo) => void }) {
  const { t, i18n } = useTranslation();
  const toast = useStore((s) => s.toast);
  const [recent, setRecent] = useState<RecentProject[] | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    api.recentProjects().then(setRecent).catch((e) => toast("error", errorText(e, t)));
  }, [t, toast]);

  useEffect(() => {
    load();
    api.appInfo().then(setInfo).catch(() => {});
  }, [load]);

  const openPath = async (path: string) => {
    setBusy(path);
    try {
      onOpened(await api.openProject(path));
    } catch (e) {
      toast("error", errorText(e, t));
      load();
    } finally {
      setBusy(null);
    }
  };

  const pickProject = async () => {
    const dir = await open({ directory: true, multiple: false, title: t("home.openTitle") });
    if (typeof dir === "string") await openPath(dir);
  };

  return (
    <main className="home">
      <header className="home-header">
        <div>
          <h1>{t("app.name")}</h1>
          <p className="muted">{t("app.tagline")}</p>
        </div>
        <div className="home-actions">
          <button type="button" className="btn primary" onClick={() => setCreating(true)}>
            {t("home.newProject")}
          </button>
          <button type="button" className="btn" onClick={pickProject}>
            {t("home.openProject")}
          </button>
        </div>
      </header>

      <section>
        <h2 className="section-title">{t("home.recent")}</h2>
        {recent === null && <p className="muted">{t("common.loading")}</p>}
        {recent?.length === 0 && <p className="empty">{t("home.noRecent")}</p>}
        <ul className="recent-list">
          {recent?.map((p) => (
            <li key={p.path} className={`recent-card${p.available ? "" : " unavailable"}`}>
              <button type="button" className="recent-main" disabled={!p.available || busy !== null} onClick={() => openPath(p.path)} title={p.path}>
                <span className="recent-name">{p.name}</span>
                <span className="recent-client">{p.clientName}</span>
                <span className="recent-meta">
                  {[t(`eventType.${p.eventType}`), formatEventDate(p.eventDate, i18n.language), p.location].filter(Boolean).join(" · ")}
                </span>
                <span className="recent-meta">
                  {t("home.photoCount", { count: p.photoCount })} · {t("home.lastOpened", { date: formatDateTimeMillis(p.lastOpened, i18n.language) })}
                </span>
                {!p.available && <span className="badge warn">{t("home.unavailable")}</span>}
                {busy === p.path && <span className="badge">{t("common.opening")}</span>}
              </button>
              <button
                type="button"
                className="btn small ghost"
                title={t("home.forgetHint")}
                onClick={async () => {
                  await api.forgetRecent(p.path);
                  load();
                }}
              >
                {t("home.forget")}
              </button>
            </li>
          ))}
        </ul>
      </section>

      {info && (
        <footer className="home-footer muted">
          {t("app.name")} {info.appVersion} · {t("home.core")} {info.coreVersion} · {t("home.schema")} {info.catalogSchemaVersion} · {info.os}/{info.arch}
        </footer>
      )}

      {creating && (
        <NewProjectDialog
          onClose={() => setCreating(false)}
          onCreated={(p) => {
            setCreating(false);
            onOpened(p);
          }}
        />
      )}
    </main>
  );
}
