import { open, save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { formatBytes } from "../logic/format";
import { errorText, useStore } from "../store";
import type { ProjectInfo, SafetyStatus } from "../types";
import { Modal } from "./Modal";

const SCHEDULES = [0, 15, 30, 60, 240];

function when(secs: number, lang: string): string {
  return new Date(secs * 1000).toLocaleString(lang, { dateStyle: "short", timeStyle: "short" });
}

/** Segurança e cópias (Fase 13): estado, versões do catálogo, exportar/importar, registo. */
export function SafetyDialog({ onClose, onReopened }: { onClose: () => void; onReopened: (p: ProjectInfo) => void }) {
  const { t, i18n } = useTranslation();
  const toast = useStore((s) => s.toast);
  const reloadPhotos = useStore((s) => s.reloadPhotos);
  const upsert = useStore((s) => s.upsertExportJob);
  const fail = useCallback((e: unknown) => toast("error", errorText(e, t)), [toast, t]);
  const [tab, setTab] = useState<"status" | "versions" | "catalog" | "log">("status");
  const [st, setSt] = useState<SafetyStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<{ kind: "restore" | "import"; file: string } | null>(null);
  const refresh = useCallback(() => api.safetyStatus().then(setSt).catch(fail), [fail]);
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = async (f: () => Promise<void>) => {
    setBusy(true);
    try {
      await f();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const replace = (kind: "restore" | "import", file: string) =>
    run(async () => {
      const p = kind === "restore" ? await api.restoreBackup(file) : await api.importCatalog(file);
      setConfirm(null);
      toast("info", t(kind === "restore" ? "safety.restored" : "safety.imported"));
      onReopened(p);
      onClose();
    });

  if (!st) return null;
  const a = st.availability;
  const r = st.report;
  return (
    <Modal title={t("safety.title")} onClose={onClose} wide footer={<button type="button" className="btn" data-close onClick={onClose}>{t("common.close")}</button>}>
      <div className="gallery-tabs" role="tablist" aria-label={t("safety.title")}>
        {(["status", "versions", "catalog", "log"] as const).map((k) => (
          <button key={k} type="button" role="tab" aria-selected={tab === k} className={tab === k ? "active" : ""} onClick={() => setTab(k)}>
            {t(`safety.tab.${k}`)}
          </button>
        ))}
      </div>
      {tab === "status" && (
        <div className="safety-section">
          {r.previousUnclean && (
            <p className="notice warn" role="status">
              {t("safety.unclean", { ms: r.validationMs })}
              {r.tempFilesRemoved > 0 && ` ${t("safety.tempRemoved", { count: r.tempFilesRemoved })}`}
            </p>
          )}
          {r.restoredFrom != null && <p className="notice warn">{t("safety.restoredFrom", { when: when(r.restoredFrom, i18n.language) })}</p>}
          <h4>{t("safety.originals")}</h4>
          <p>{t("safety.availability", { ok: a.ok, offline: a.offline, missing: a.missing })}</p>
          <p className="small muted">{t("safety.availabilityHint")}</p>
          <div className="dev-row">
            <button type="button" className="btn small" disabled={busy} onClick={() => void run(async () => { await api.checkAvailability(); await refresh(); await reloadPhotos(); })}>
              {t("safety.checkNow")}
            </button>
            <button
              type="button"
              className="btn small"
              disabled={busy || a.offline + a.missing === 0}
              onClick={() =>
                void run(async () => {
                  const d = await open({ directory: true, multiple: false, title: t("safety.relinkFolder") });
                  if (typeof d !== "string") return;
                  const res = await api.relinkFolder(d);
                  toast("info", t("safety.relinked", { count: res.relinked, left: res.stillUnavailable }));
                  await refresh();
                  await reloadPhotos();
                })
              }
            >
              {t("safety.relinkFolder")}
            </button>
          </div>
          <h4>{t("safety.pendingExports")}</h4>
          {st.pendingExports.length === 0 ? (
            <p className="small muted">{t("safety.noPendingExports")}</p>
          ) : (
            <ul className="safety-list" aria-label={t("safety.pendingExports")}>
              {st.pendingExports.map((p) => (
                <li key={p.id}>
                  <span className="grow">{t("safety.pendingExport", { name: p.name, done: p.done, total: p.total })}</span>
                  <button
                    type="button"
                    className="btn small primary"
                    disabled={busy}
                    onClick={() =>
                      void run(async () => {
                        const id = await api.resumeExport(p.id);
                        const job = (await api.exportJobs()).find((j) => j.id === id);
                        if (job) upsert(job);
                        await refresh();
                      })
                    }
                  >
                    {t("safety.resume")}
                  </button>
                  <button type="button" className="btn small" disabled={busy} onClick={() => void run(async () => { await api.discardExport(p.id); await refresh(); })}>
                    {t("safety.discard")}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      {tab === "versions" && (
        <div className="safety-section">
          <p className="small muted">{t("safety.versionsHint")}</p>
          <div className="dev-row">
            <label className="field inline">
              <span>{t("safety.schedule")}</span>
              <select
                value={st.backupMinutes}
                aria-label={t("safety.schedule")}
                onChange={(e) => void run(async () => { await api.setBackupMinutes(Number(e.target.value)); await refresh(); })}
              >
                {SCHEDULES.map((m) => (
                  <option key={m} value={m}>
                    {m === 0 ? t("safety.scheduleOff") : t("safety.scheduleEvery", { count: m })}
                  </option>
                ))}
              </select>
            </label>
            <button type="button" className="btn small primary" disabled={busy} onClick={() => void run(async () => { await api.createBackup(); toast("info", t("safety.backupCreated")); await refresh(); })}>
              {t("safety.backupNow")}
            </button>
            <button
              type="button"
              className="btn small"
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  const v = await api.validateCatalog();
                  toast(v.integrity.length === 0 && v.foreignKeys === 0 ? "info" : "error", v.integrity.length === 0 && v.foreignKeys === 0 ? t("safety.valid") : t("safety.invalid"));
                })
              }
            >
              {t("safety.validate")}
            </button>
          </div>
          <ul className="safety-list" aria-label={t("safety.versions")}>
            {st.backups.map((b) => (
              <li key={b.file}>
                <span className="grow">{when(b.created, i18n.language)}</span>
                <span className="small muted">{t(`safety.kind.${b.kind}`, { defaultValue: b.kind })}</span>
                <span className="small muted">{formatBytes(b.bytes)}</span>
                {b.kind !== "danificado" && (
                  <button type="button" className="btn small" disabled={busy} onClick={() => setConfirm({ kind: "restore", file: b.file })}>
                    {t("safety.restore")}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
      {tab === "catalog" && (
        <div className="safety-section">
          <p className="small muted">{t("safety.catalogHint")}</p>
          <div className="dev-row">
            <button
              type="button"
              className="btn small"
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  const d = await save({ title: t("safety.exportCatalog"), defaultPath: "catalogo.momento-catalog", filters: [{ name: "Momento", extensions: ["momento-catalog", "db"] }] });
                  if (typeof d !== "string") return;
                  await api.exportCatalog(d);
                  toast("info", t("safety.exported"));
                })
              }
            >
              {t("safety.exportCatalog")}
            </button>
            <button
              type="button"
              className="btn small"
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  const f = await open({ multiple: false, title: t("safety.importCatalog"), filters: [{ name: "Momento", extensions: ["momento-catalog", "db"] }] });
                  if (typeof f === "string") setConfirm({ kind: "import", file: f });
                })
              }
            >
              {t("safety.importCatalog")}
            </button>
          </div>
        </div>
      )}
      {tab === "log" && (
        <ul className="safety-list" aria-label={t("safety.tab.log")}>
          {st.events.length === 0 && <li className="small muted">{t("safety.noEvents")}</li>}
          {st.events.map(([at, kind, detail], i) => (
            <li key={i}>
              <span className="small muted">{when(at, i18n.language)}</span>
              <span className="grow">{t(`safety.event.${kind}`, { defaultValue: kind })}</span>
              <span className="small muted">{detail}</span>
            </li>
          ))}
        </ul>
      )}
      {confirm && (
        <div className="notice warn" role="alertdialog" aria-label={t("safety.confirmTitle")}>
          <p>{t(confirm.kind === "restore" ? "safety.confirmRestore" : "safety.confirmImport")}</p>
          <div className="dev-row">
            <button type="button" className="btn small" onClick={() => setConfirm(null)}>
              {t("common.cancel")}
            </button>
            <button type="button" className="btn small primary" disabled={busy} onClick={() => void replace(confirm.kind, confirm.file)}>
              {t(confirm.kind === "restore" ? "safety.restore" : "safety.importCatalog")}
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}

/** Remover do catálogo / mover para o lixo — sempre com escolha e confirmação explícitas. */
export function DeleteDialog({ keys, onClose }: { keys: number[]; onClose: () => void }) {
  const { t } = useTranslation();
  const toast = useStore((s) => s.toast);
  const reloadPhotos = useStore((s) => s.reloadPhotos);
  const [choice, setChoice] = useState<"catalog" | "trash" | null>(null);
  const [understood, setUnderstood] = useState(false);
  const [busy, setBusy] = useState(false);
  const ok = choice === "catalog" || (choice === "trash" && understood);
  const go = async () => {
    if (!ok) return;
    setBusy(true);
    try {
      const r = choice === "trash" ? await api.moveToTrash(keys, true) : await api.removeFromCatalog(keys, true);
      toast(r.failed.length ? "error" : "info", choice === "trash" ? t("delete.trashed", { count: r.trashed, failed: r.failed.length }) : t("delete.removed", { count: r.removed }));
      await reloadPhotos();
      const st = useStore.getState();
      st.reloadSelection().catch(() => {});
      st.reloadOrganization().catch(() => {});
      onClose();
    } catch (e) {
      toast("error", errorText(e, t));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      title={t("delete.title", { count: keys.length })}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" data-close onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button type="button" className={`btn ${choice === "trash" ? "danger" : "primary"}`} disabled={!ok || busy} onClick={() => void go()}>
            {choice === "trash" ? t("delete.confirmTrash", { count: keys.length }) : t("delete.confirmRemove", { count: keys.length })}
          </button>
        </>
      }
    >
      <fieldset className="options">
        <legend>{t("delete.question")}</legend>
        <label className="radio">
          <input type="radio" name="delete" checked={choice === "catalog"} onChange={() => setChoice("catalog")} />
          <span>
            <strong>{t("delete.catalog")}</strong>
            <small className="muted">{t("delete.catalogHint")}</small>
          </span>
        </label>
        <label className="radio">
          <input type="radio" name="delete" checked={choice === "trash"} onChange={() => setChoice("trash")} />
          <span>
            <strong>{t("delete.trash")}</strong>
            <small className="muted">{t("delete.trashHint")}</small>
          </span>
        </label>
      </fieldset>
      {choice === "trash" && (
        <label className="checkbox">
          <input type="checkbox" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} />
          <span>{t("delete.understand", { count: keys.length })}</span>
        </label>
      )}
    </Modal>
  );
}
