import { describe, expect, it } from "vitest";
import { DEFAULT_LANGUAGE, flattenKeys, resources } from "./index";

describe("traduções", () => {
  const reference = flattenKeys(resources[DEFAULT_LANGUAGE].translation).sort();

  it("pt-PT é o idioma por omissão", () => {
    expect(DEFAULT_LANGUAGE).toBe("pt-PT");
    expect(reference.length).toBeGreaterThan(0);
  });

  for (const [lang, res] of Object.entries(resources)) {
    it(`${lang} tem exatamente as mesmas chaves que pt-PT`, () => {
      expect(flattenKeys(res.translation).sort()).toEqual(reference);
    });

    it(`${lang} não tem textos vazios`, () => {
      const walk = (o: unknown): string[] =>
        typeof o === "string" ? [o] : Object.values(o as object).flatMap(walk);
      for (const s of walk(res.translation)) expect(s.trim()).not.toBe("");
    });
  }

  it("todas as chaves de erro do núcleo existem", () => {
    // Mantém-se sincronizado com `ErrorPayload` em packages/core-types.
    for (const code of ["io", "database", "unsupportedFormat", "corrupt", "catalogTooNew", "other"]) {
      expect(reference).toContain(`error.${code}`);
    }
  });
});
