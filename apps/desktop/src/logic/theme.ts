import { scaleTokens, themeTokens, type ThemeName } from "@momento/ui";

/** Preferência do utilizador: escuro (principal, por omissão), claro, ou o do sistema. */
export type ThemePref = ThemeName | "system";

const KEY = "momento.theme";

export function loadThemePref(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "dark" || v === "light" || v === "system") return v;
  } catch {
    /* armazenamento indisponível: escuro */
  }
  return "dark";
}

export function resolveTheme(pref: ThemePref, systemDark: boolean): ThemeName {
  return pref === "system" ? (systemDark ? "dark" : "light") : pref;
}

function systemDark(): boolean {
  return typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)").matches : true;
}

/** Aplica o tema: variáveis CSS no `:root` e `data-theme` (para regras específicas). */
export function applyTheme(pref: ThemePref): ThemeName {
  const name = resolveTheme(pref, systemDark());
  const root = document.documentElement;
  for (const [k, v] of Object.entries({ ...scaleTokens, ...themeTokens[name] })) {
    if (k === "color-scheme") root.style.colorScheme = v;
    else root.style.setProperty(k, v);
  }
  root.dataset.theme = name;
  return name;
}

export function saveThemePref(pref: ThemePref): ThemeName {
  try {
    localStorage.setItem(KEY, pref);
  } catch {
    /* sem persistência: aplica-se só nesta sessão */
  }
  return applyTheme(pref);
}

/** Segue o sistema quando a preferência é "system". */
export function watchSystemTheme(): void {
  if (typeof matchMedia !== "function") return;
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (loadThemePref() === "system") applyTheme("system");
  });
}
