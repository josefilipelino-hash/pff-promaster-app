import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./i18n";
import "./styles.css";
import { App } from "./App";
import { applyTheme, loadThemePref, watchSystemTheme } from "./logic/theme";
import { startPerfProbe } from "./logic/perfProbe";
import { installKeyboardNav } from "./logic/keyboardNav";

// Tema (Fase 14): escuro por omissão; claro ou o do sistema nas preferências.
applyTheme(loadThemePref());
watchSystemTheme();
installKeyboardNav();

// Menu nativo do WebView desativado: a aplicação tem os seus menus contextuais (Fase 14).
window.addEventListener("contextmenu", (e) => {
  if (!(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) e.preventDefault();
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

void startPerfProbe();
