import ptPT from "../locales/pt-PT.json";
import en from "../locales/en.json";

/**
 * Idiomas da interface. `pt-PT` é a referência: todas as chaves existem primeiro aqui.
 * Idiomas planeados (ainda sem ficheiro): es, fr, de — ver ROADMAP.md.
 * Um idioma só é adicionado a `resources` quando tem todas as chaves (ver teste).
 */
export const DEFAULT_LANGUAGE = "pt-PT" as const;
export const PLANNED_LANGUAGES = ["pt-PT", "en", "es", "fr", "de"] as const;

export const resources = {
  "pt-PT": { translation: ptPT },
  en: { translation: en },
} as const;

export type SupportedLanguage = keyof typeof resources;
export type TranslationSchema = typeof ptPT;

/** Lista achatada das chaves ("a.b.c") de um objeto de tradução. */
export function flattenKeys(obj: unknown, prefix = ""): string[] {
  if (obj === null || typeof obj !== "object") return [prefix];
  return Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) =>
    flattenKeys(v, prefix ? `${prefix}.${k}` : k),
  );
}
