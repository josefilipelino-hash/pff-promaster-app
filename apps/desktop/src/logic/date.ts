/**
 * Datas no formato português DD/MM/AAAA ↔ ISO AAAA-MM-DD.
 * Usado em vez de `<input type="date">`, cujo comportamento e formato variam entre o
 * WebKit (macOS), o WebView2 (Windows) e o WebKitGTK, e dependem da região do sistema.
 */

export function isValidIsoDate(iso: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return false;
  const [y, mo, d] = [+m[1]!, +m[2]!, +m[3]!];
  if (y < 1900 || y > 2200 || mo < 1 || mo > 12 || d < 1) return false;
  const days = new Date(Date.UTC(y, mo, 0)).getUTCDate();
  return d <= days;
}

/** `15/06/2026` → `2026-06-15`; vazio → `""`; inválido → `null`. Aceita `-` e `.` como separador. */
export function parsePtDate(text: string): string | null {
  const t = text.trim();
  if (t === "") return "";
  const m = /^(\d{1,2})[/.-](\d{1,2})[/.-](\d{4})$/.exec(t);
  if (!m) return null;
  const iso = `${m[3]}-${m[2]!.padStart(2, "0")}-${m[1]!.padStart(2, "0")}`;
  return isValidIsoDate(iso) ? iso : null;
}

/** `2026-06-15` → `15/06/2026`. */
export function isoToPt(iso: string | null): string {
  if (!iso) return "";
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  return m ? `${m[3]}/${m[2]}/${m[1]}` : "";
}

/** Insere as barras automaticamente enquanto o utilizador escreve só algarismos. */
export function maskPtDate(input: string): string {
  if (/[^\d]/.test(input.replace(/\//g, ""))) return input; // texto livre: não mexer
  const digits = input.replace(/\D/g, "").slice(0, 8);
  if (digits.length <= 2) return digits;
  if (digits.length <= 4) return `${digits.slice(0, 2)}/${digits.slice(2)}`;
  return `${digits.slice(0, 2)}/${digits.slice(2, 4)}/${digits.slice(4)}`;
}
