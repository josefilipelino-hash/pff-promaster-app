// Entrada da auditoria visual: a aplicação real com a API simulada (audit/api.ts).
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "../src/i18n";
import "../src/styles.css";
import { App } from "../src/App";
import { applyTheme, loadThemePref } from "../src/logic/theme";
import { useStore } from "../src/store";
import { api } from "./api";

applyTheme(loadThemePref());
(window as unknown as { __store: typeof useStore }).__store = useStore;
(window as unknown as { __openProject: () => Promise<void> }).__openProject = async () => {
  const p = await api.openProject("/trabalho/fazendas.momento" as never);
  useStore.getState().setProject(p as never);
};
window.addEventListener("error", (e) => console.error("ERRO:", e.message));
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
