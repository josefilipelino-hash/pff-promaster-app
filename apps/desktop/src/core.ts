import { invoke, isTauri } from "@tauri-apps/api/core";

/** Espelho de `AppInfo` em apps/desktop/src-tauri/src/lib.rs. */
export interface AppInfo {
  appVersion: string;
  coreVersion: string;
  catalogSchemaVersion: number;
  os: string;
  arch: string;
}

/** Devolve `null` quando a interface corre num navegador normal (sem núcleo). */
export async function fetchAppInfo(): Promise<AppInfo | null> {
  if (!isTauri()) return null;
  return invoke<AppInfo>("app_info");
}
