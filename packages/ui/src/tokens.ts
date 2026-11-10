export type ThemeName = "dark" | "light";

/**
 * Sistema de design (Fase 14).
 *
 * Cores neutras: a interface de edição fotográfica usa cinzentos sem dominante para não
 * influenciar a perceção de cor das fotografias. A cor de destaque é usada com parcimónia
 * (seleção, ação principal, foco). Os contrastes de texto cumprem WCAG AA (testado).
 */
export const themeTokens: Record<ThemeName, Record<string, string>> = {
  dark: {
    "--bg": "#161616",
    "--surface": "#1e1e1e",
    "--surface-2": "#262626",
    "--surface-3": "#303030",
    "--canvas": "#111111",
    "--border": "#333333",
    "--border-strong": "#474747",
    "--text": "#ececec",
    "--text-muted": "#a3a3a3",
    "--text-faint": "#959595",
    "--accent": "#d4b27a",
    "--accent-strong": "#e3c48f",
    "--accent-soft": "rgba(212, 178, 122, 0.14)",
    "--on-accent": "#1a1a1a",
    "--focus": "#e3c48f",
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
    "--bg": "#f3f3f3",
    "--surface": "#ffffff",
    "--surface-2": "#ececec",
    "--surface-3": "#e1e1e1",
    "--canvas": "#d9d9d9",
    "--border": "#d4d4d4",
    "--border-strong": "#b5b5b5",
    "--text": "#1a1a1a",
    "--text-muted": "#555555",
    "--text-faint": "#636363",
    "--accent": "#8a6424",
    "--accent-strong": "#6f4f19",
    "--accent-soft": "rgba(138, 100, 36, 0.12)",
    "--on-accent": "#ffffff",
    "--focus": "#8a6424",
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
  "--r-sm": "4px",
  "--r-md": "6px",
  "--r-lg": "10px",
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
