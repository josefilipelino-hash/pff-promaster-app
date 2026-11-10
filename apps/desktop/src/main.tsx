import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { themeTokens } from "@momento/ui";
import "./i18n";
import "./styles.css";
import { App } from "./App";
import { startPerfProbe } from "./logic/perfProbe";

for (const [k, v] of Object.entries(themeTokens.dark)) {
  document.documentElement.style.setProperty(k, v);
}

// Desativa o menu de contexto do WebView (ainda sem menus próprios nesta fase).
window.addEventListener("contextmenu", (e) => {
  if (!(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) e.preventDefault();
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

void startPerfProbe();
