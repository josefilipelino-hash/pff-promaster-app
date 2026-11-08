import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { fetchAppInfo, type AppInfo } from "./core";

type CoreState = { kind: "loading" } | { kind: "ready"; info: AppInfo } | { kind: "unavailable" };

export function App({ loadInfo = fetchAppInfo }: { loadInfo?: () => Promise<AppInfo | null> }) {
  const { t } = useTranslation();
  const [core, setCore] = useState<CoreState>({ kind: "loading" });

  useEffect(() => {
    let alive = true;
    loadInfo()
      .then((info) => alive && setCore(info ? { kind: "ready", info } : { kind: "unavailable" }))
      .catch(() => alive && setCore({ kind: "unavailable" }));
    return () => {
      alive = false;
    };
  }, [loadInfo]);

  return (
    <main className="foundation">
      <header>
        <h1>{t("app.name")}</h1>
        <p className="muted">{t("app.tagline")}</p>
      </header>
      <section className="card">
        <span className="badge">{t("foundation.phase")}</span>
        <p>{t("foundation.notice")}</p>
      </section>
      <section className="card">
        <h2>{t("foundation.diagnostics")}</h2>
        {core.kind === "loading" && <p className="muted">{t("foundation.loading")}</p>}
        {core.kind === "unavailable" && <p className="muted">{t("foundation.noBackend")}</p>}
        {core.kind === "ready" && (
          <dl>
            <dt>{t("foundation.appVersion")}</dt>
            <dd>{core.info.appVersion}</dd>
            <dt>{t("foundation.coreVersion")}</dt>
            <dd>{core.info.coreVersion}</dd>
            <dt>{t("foundation.schemaVersion")}</dt>
            <dd>{core.info.catalogSchemaVersion}</dd>
            <dt>{t("foundation.platform")}</dt>
            <dd>
              {core.info.os} / {core.info.arch}
            </dd>
          </dl>
        )}
      </section>
    </main>
  );
}
