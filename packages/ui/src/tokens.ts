export type ThemeName = "dark" | "light";

/**
 * Sistema de design (Fase 14).
 *
 * Cores neutras: a interface de edição fotográfica usa cinzentos sem dominante para não
 * influenciar a perceção de cor das fotografias. A energia vem do destaque (coral → sol,
 * pedido do utilizador: "alegria e boa vibe, mas prático"), usado nas ações principais, na
 * seleção, no fluxo de trabalho e no foco. Os contrastes de texto cumprem WCAG AA (testado).
 */
export const themeTokens: Record<ThemeName, Record<string, string>> = {
  dark: {
    "--bg": "#141414",
    "--surface": "#1c1c1c",
    "--surface-2": "#252525",
    "--surface-3": "#2f2f2f",
    "--canvas": "#101010",
    "--border": "#2e2e2e",
    "--border-strong": "#454545",
    "--text": "#ececec",
    "--text-muted": "#a3a3a3",
    "--text-faint": "#959595",
    "--accent": "#ff8c69",
    "--accent-strong": "#ffa688",
    "--accent-soft": "rgba(255, 140, 105, 0.15)",
    "--accent-2": "#ffc857",
    "--accent-grad": "linear-gradient(120deg, #ff7a59 0%, #ff9f6e 45%, #ffc857 100%)",
    "--on-accent": "#1c1210",
    "--focus": "#ffa688",
    "--danger": "#ef5a5f",
    "--warn": "#e2b93b",
    "--success": "#4cb35f",
    "--info": "#5aa2e0",
    "--overlay": "rgba(0, 0, 0, 0.62)",
    "--shadow-pop": "0 10px 30px rgba(0, 0, 0, 0.45), 0 0 0 1px rgba(255, 255, 255, 0.04)",
    "--hover": "rgba(255, 255, 255, 0.05)",
    "--pressed": "rgba(255, 255, 255, 0.09)",
    "color-scheme": "dark",
  },
  light: {
    "--bg": "#f5f4f2",
    "--surface": "#ffffff",
    "--surface-2": "#eeedeb",
    "--surface-3": "#e1e1e1",
    "--canvas": "#d9d9d9",
    "--border": "#d4d4d4",
    "--border-strong": "#b5b5b5",
    "--text": "#1a1a1a",
    "--text-muted": "#555555",
    "--text-faint": "#636363",
    "--accent": "#b8401f",
    "--accent-strong": "#963218",
    "--accent-soft": "rgba(232, 96, 60, 0.12)",
    "--accent-2": "#9a6400",
    "--accent-grad": "linear-gradient(120deg, #e8603c 0%, #f08a4b 45%, #f2b33d 100%)",
    "--on-accent": "#ffffff",
    "--focus": "#b8401f",
    "--danger": "#c62d33",
    "--warn": "#8c6a00",
    "--success": "#2f7d3c",
    "--info": "#1f65a8",
    "--overlay": "rgba(20, 20, 20, 0.45)",
    "--shadow-pop": "0 10px 30px rgba(0, 0, 0, 0.16), 0 0 0 1px rgba(0, 0, 0, 0.05)",
    "--hover": "rgba(0, 0, 0, 0.045)",
    "--pressed": "rgba(0, 0, 0, 0.08)",
    "color-scheme": "light",
  },
};

/** Escalas comuns aos dois temas: tipografia, espaço, raios, movimento. */
export const scaleTokens: Record<string, string> = {
  "--font": '-apple-system, BlinkMacSystemFont, "Segoe UI Variable Text", "Segoe UI", system-ui, "Helvetica Neue", sans-serif',
  "--fs-xs": "11px",
  "--fs-sm": "12px",
  "--fs-md": "13px",
  "--fs-lg": "15px",
  "--fs-xl": "18px",
  "--fs-2xl": "24px",
  "--sp-1": "4px",
  "--sp-2": "8px",
  "--sp-3": "12px",
  "--sp-4": "16px",
  "--sp-5": "24px",
  "--sp-6": "32px",
  "--r-sm": "6px",
  "--r-md": "9px",
  "--r-lg": "14px",
  "--r-full": "999px",
  "--control-h": "28px",
  "--control-h-sm": "24px",
  "--toolbar-h": "48px",
  "--dur": "120ms",
};

/** Contraste WCAG entre duas cores "#rrggbb" (1–21). */
export function contrast(a: string, b: string): number {
  const lum = (hex: string) => {
    const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return 0.2126 * c[0]! + 0.7152 * c[1]! + 0.0722 * c[2]!;
  };
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p) as [number, number];
  return (x + 0.05) / (y + 0.05);
}
