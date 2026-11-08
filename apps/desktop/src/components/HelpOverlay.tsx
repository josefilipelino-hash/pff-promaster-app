import { useTranslation } from "react-i18next";
import { isMac } from "../api";
import { SHORTCUT_HELP } from "../logic/shortcuts";
import { Modal } from "./Modal";

export function HelpOverlay({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const mod = isMac() ? "⌘" : "Ctrl";
  return (
    <Modal
      title={t("shortcuts.title")}
      onClose={onClose}
      footer={
        <button type="button" className="btn primary" onClick={onClose}>
          {t("common.close")}
        </button>
      }
    >
      <table className="shortcuts">
        <tbody>
          {SHORTCUT_HELP.map(([keys, desc]) => (
            <tr key={desc}>
              <td>
                <kbd>{keys.replace("Mod", mod)}</kbd>
              </td>
              <td>{t(desc)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Modal>
  );
}
