import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { themeTokens } from "@momento/ui";
import "./i18n";
import "./styles.css";
import { App } from "./App";

for (const [k, v] of Object.entries(themeTokens.dark)) {
  document.documentElement.style.setProperty(k, v);
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
