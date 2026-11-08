import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { maskPtDate, parsePtDate } from "../logic/date";
import { errorText } from "../store";
import type { NewProject, ProjectInfo } from "../types";
import { Modal } from "./Modal";

/** Igual a `project_folder_name` em packages/project-management/src/paths.rs (pré-visualização). */
export function folderPreview(date: string, client: string, name: string): string {
  const base = client.trim() || name;
  let slug = "";
  for (const c of base.trim()) {
    if (/[\p{L}\p{N}-]/u.test(c)) slug += c;
    else if (c === "&" || c === "+") slug += "_e_";
    else if (!slug.endsWith("_")) slug += "_";
  }
  slug = slug.split("_").filter(Boolean).join("_") || "Projeto";
  return `${date ? `${date}_` : ""}${slug}.momento`;
}

function joinPath(dir: string, name: string): string {
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
}

export function NewProjectDialog({ onClose, onCreated }: { onClose: () => void; onCreated: (p: ProjectInfo) => void }) {
  const { t } = useTranslation();
  const [types, setTypes] = useState<string[]>(["wedding"]);
  const [form, setForm] = useState({ name: "", clientName: "", eventType: "wedding", eventDate: "", location: "", originalsDir: "", workDir: "" });
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api.eventTypes().then(setTypes).catch(() => {});
  }, []);

  const set = (k: keyof typeof form) => (e: { target: { value: string } }) => setForm({ ...form, [k]: e.target.value });

  const pick = async (k: "originalsDir" | "workDir") => {
    const dir = await open({ directory: true, multiple: false, title: t(k === "workDir" ? "newProject.workDir" : "newProject.originalsDir") });
    if (typeof dir === "string") setForm((f) => ({ ...f, [k]: dir }));
  };

  const isoDate = parsePtDate(form.eventDate);
  const folder = useMemo(() => folderPreview(isoDate ?? "", form.clientName, form.name), [isoDate, form.clientName, form.name]);
  const valid = form.name.trim() !== "" && form.workDir.trim() !== "" && isoDate !== null;

  const submit = async () => {
    if (isoDate === null) {
      setError(t("newProject.invalidDate"));
      return;
    }
    if (!valid) {
      setError(t("newProject.required"));
      return;
    }
    setSaving(true);
    setError(null);
    const input: NewProject = {
      name: form.name,
      clientName: form.clientName,
      eventType: form.eventType,
      eventDate: isoDate || null,
      location: form.location,
      originalsDir: form.originalsDir.trim() || null,
      workDir: form.workDir,
    };
    try {
      onCreated(await api.createProject(input));
    } catch (e) {
      setError(errorText(e, t));
      setSaving(false);
    }
  };

  return (
    <Modal
      title={t("newProject.title")}
      onClose={onClose}
      wide
      footer={
        <>
          <button type="button" className="btn" data-close onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn primary" disabled={saving || !valid} onClick={submit}>
            {saving ? t("common.creating") : t("newProject.create")}
          </button>
        </>
      }
    >
      <form
        className="form-grid"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <label>
          <span>{t("newProject.name")} *</span>
          <input value={form.name} onChange={set("name")} placeholder={t("newProject.namePlaceholder")} required />
        </label>
        <label>
          <span>{t("newProject.client")}</span>
          <input value={form.clientName} onChange={set("clientName")} placeholder={t("newProject.clientPlaceholder")} />
        </label>
        <label>
          <span>{t("newProject.eventType")}</span>
          <select value={form.eventType} onChange={set("eventType")}>
            {types.map((ty) => (
              <option key={ty} value={ty}>
                {t(`eventType.${ty}`)}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>{t("newProject.date")}</span>
          <input
            value={form.eventDate}
            inputMode="numeric"
            placeholder={t("newProject.datePlaceholder")}
            aria-invalid={isoDate === null}
            onChange={(e) => setForm({ ...form, eventDate: maskPtDate(e.target.value) })}
          />
          {isoDate === null && form.eventDate.length >= 10 && <small className="form-error">{t("newProject.invalidDate")}</small>}
        </label>
        <label className="span2">
          <span>{t("newProject.location")}</span>
          <input value={form.location} onChange={set("location")} placeholder={t("newProject.locationPlaceholder")} />
        </label>
        <label className="span2">
          <span>{t("newProject.originalsDir")}</span>
          <div className="path-input">
            <input value={form.originalsDir} onChange={set("originalsDir")} placeholder={t("newProject.originalsPlaceholder")} />
            <button type="button" className="btn" onClick={() => pick("originalsDir")}>
              {t("common.choose")}
            </button>
          </div>
          <small className="muted">{t("newProject.originalsHint")}</small>
        </label>
        <label className="span2">
          <span>{t("newProject.workDir")} *</span>
          <div className="path-input">
            <input value={form.workDir} onChange={set("workDir")} placeholder={t("newProject.workPlaceholder")} required />
            <button type="button" className="btn" onClick={() => pick("workDir")}>
              {t("common.choose")}
            </button>
          </div>
          <small className="muted">
            {form.workDir ? t("newProject.willCreate", { path: joinPath(form.workDir, folder) }) : t("newProject.workHint")}
          </small>
        </label>
        {error && (
          <p className="form-error span2" role="alert">
            {error}
          </p>
        )}
        <button type="submit" hidden />
      </form>
    </Modal>
  );
}
