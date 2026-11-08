export type ThemeName = "dark" | "light";

/**
 * Cores neutras: a interface de edição fotográfica deve usar cinzentos sem dominante
 * para não influenciar a perceção de cor das fotografias.
 */
export const themeTokens: Record<ThemeName, Record<string, string>> = {
  dark: {
    "--bg": "#1a1a1a",
    "--surface": "#232323",
    "--surface-2": "#2c2c2c",
    "--border": "#3a3a3a",
    "--text": "#e6e6e6",
    "--text-muted": "#9a9a9a",
    "--accent": "#d9b77e",
  },
  light: {
    "--bg": "#f2f2f2",
    "--surface": "#ffffff",
    "--surface-2": "#e8e8e8",
    "--border": "#cfcfcf",
    "--text": "#1c1c1c",
    "--text-muted": "#5f5f5f",
    "--accent": "#9a7434",
  },
};
