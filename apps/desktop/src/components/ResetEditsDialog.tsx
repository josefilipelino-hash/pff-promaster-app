import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { errorText, useStore } from "../store";
import { Modal } from "./Modal";

/**
 * Confirmação antes de remover a edição de várias fotografias (voltam ao original). Cada
 * fotografia guarda o passo no histórico, por isso pode ser desfeito no editor.
 */
export function ResetEditsDialog({ keys, onClose, onDone }: { keys: number[]; onClose: () => void; onDone?: (n: number) => void }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const run = async () => {
    setBusy(true);
    const st = useStore.getState();
    try {
      const n = await api.resetEdits(keys);
      st.toast("info", t("develop.resetDone", { count: n }));
      await st.reloadPhotos();
      onDone?.(n);
      onClose();
    } catch (e) {
      st.toast("error", errorText(e, t));
      setBusy(false);
    }
  };
  return (
    <Modal
      title={t("develop.resetManyTitle", { count: keys.length })}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose} data-close>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn primary" disabled={busy} onClick={() => void run()}>
            {t("develop.resetManyConfirm")}
          </button>
        </>
      }
    >
      <p>{t("develop.resetManyText")}</p>
    </Modal>
  );
}
