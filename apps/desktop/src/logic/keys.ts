import { isMac } from "../api";

/**
 * Etiqueta de um atalho para mostrar (Fase 14): "Mod+Shift+E" → "⇧⌘E" no macOS,
 * "Ctrl+Shift+E" no Windows/Linux. Teclas simples ("D", "?") ficam iguais.
 */
export function shortcutLabel(spec: string, mac = isMac()): string {
  const parts = spec.split("+");
  if (parts.length === 1) return spec;
  const key = parts[parts.length - 1]!;
  const mods = parts.slice(0, -1);
  if (mac) {
    const sym: Record<string, string> = { Mod: "⌘", Shift: "⇧", Alt: "⌥", Ctrl: "⌃" };
    const order = ["Ctrl", "Alt", "Shift", "Mod"];
    return mods.sort((a, b) => order.indexOf(a) - order.indexOf(b)).map((m) => sym[m] ?? m).join("") + key;
  }
  return [...mods.map((m) => (m === "Mod" ? "Ctrl" : m)), key].join("+");
}
