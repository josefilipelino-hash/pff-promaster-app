import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "./api";
import { HomeScreen } from "./components/HomeScreen";
import { ProjectScreen } from "./components/ProjectScreen";
import { Toasts } from "./components/Toasts";
import { useStore } from "./store";

export function App() {
  const { t } = useTranslation();
  const project = useStore((s) => s.project);
  const setProject = useStore((s) => s.setProject);
  const [ready, setReady] = useState(false);
  const [available] = useState(() => api.available());

  // Se o WebView for recarregado com um projeto aberto, retoma-o.
  useEffect(() => {
    if (!available) return setReady(true);
    api
      .currentProject()
      .then((p) => p && setProject(p))
      .catch(() => {})
      .finally(() => setReady(true));
  }, [available, setProject]);

  if (!available) {
    return (
      <main className="home">
        <h1>{t("app.name")}</h1>
        <p className="muted">{t("app.noBackend")}</p>
      </main>
    );
  }
  if (!ready) return null;
  return (
    <>
      {project ? <ProjectScreen onClosed={() => setProject(null)} /> : <HomeScreen onOpened={setProject} />}
      <Toasts />
    </>
  );
}
