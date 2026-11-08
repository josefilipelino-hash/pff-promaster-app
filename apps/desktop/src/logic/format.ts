/** Formatação de valores fotográficos para pt-PT (e outros idiomas via `locale`). */

export function formatExposure(seconds: number | null): string | null {
  if (seconds === null || !(seconds > 0)) return null;
  if (seconds >= 0.3) {
    const s = Math.round(seconds * 10) / 10;
    return `${String(s).replace(".", ",")} s`;
  }
  return `1/${Math.round(1 / seconds)} s`;
}

export function formatAperture(f: number | null): string | null {
  if (f === null || !(f > 0)) return null;
  const v = Math.round(f * 10) / 10;
  return `f/${String(v).replace(".", ",")}`;
}

export function formatFocal(mm: number | null, mm35: number | null): string | null {
  if (mm === null || !(mm > 0)) return null;
  const v = `${Math.round(mm * 10) / 10} mm`.replace(".", ",");
  return mm35 && Math.round(mm35) !== Math.round(mm) ? `${v} (${mm35} mm eq.)` : v;
}

export function formatBytes(n: number, locale = "pt-PT"): string {
  const units = ["B", "KB", "MB", "GB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toLocaleString(locale, { maximumFractionDigits: i === 0 ? 0 : 1 })} ${units[i]}`;
}

/** `2026-06-15T14:03:22.12` → “15/06/2026, 14:03:22” (hora local da câmara, sem conversão). */
export function formatCaptureTime(iso: string | null, locale = "pt-PT"): string | null {
  if (!iso) return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})/.exec(iso);
  if (!m) return iso;
  const d = new Date(Date.UTC(+m[1]!, +m[2]! - 1, +m[3]!, +m[4]!, +m[5]!, +m[6]!));
  // Campos explícitos: resultado idêntico no WebKit (macOS) e no WebView2 (Windows).
  return d.toLocaleString(locale, {
    timeZone: "UTC",
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

/** `2026-06-15` → “15 de junho de 2026”. */
export function formatEventDate(date: string | null, locale = "pt-PT"): string | null {
  if (!date) return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  if (!m) return date;
  return new Date(Date.UTC(+m[1]!, +m[2]! - 1, +m[3]!)).toLocaleDateString(locale, { timeZone: "UTC", dateStyle: "long" });
}

export function formatDateTimeMillis(ms: number, locale = "pt-PT"): string {
  return new Date(ms).toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" });
}

export function cameraName(make: string | null, model: string | null): string | null {
  if (!make && !model) return null;
  if (!make) return model;
  if (!model) return make;
  // Muitas câmaras repetem a marca no modelo (“Canon” + “Canon EOS R5”).
  const firstWord = make.split(" ")[0]!.toLowerCase();
  if (model.toLowerCase().startsWith(firstWord)) return model;
  const shortMake = make.replace(/\s+(CORPORATION|Corporation|CO\.,?\s*LTD\.?|Co\.,?\s*Ltd\.?|IMAGING.*)$/, "");
  return `${shortMake} ${model}`;
}
