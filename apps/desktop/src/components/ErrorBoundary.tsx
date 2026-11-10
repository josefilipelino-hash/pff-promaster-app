import { Component, type ErrorInfo, type ReactNode } from "react";
import { api } from "../api";
import i18n from "../i18n";

/** Envia um erro da interface para o registo local da aplicação (nunca para fora). */
export function reportUiError(kind: string, e: unknown) {
  const err = e instanceof Error ? e : new Error(typeof e === "string" ? e : JSON.stringify(e) ?? String(e));
  try {
    void api.logUiError(kind, err.message, err.stack ?? null).catch(() => {});
  } catch {
    // Fora da aplicação (testes, browser): nada a registar.
  }
}

/** Erros globais: exceções não apanhadas e promessas rejeitadas sem tratamento. */
export function installErrorReporting() {
  window.addEventListener("error", (ev) => reportUiError("error", ev.error ?? ev.message));
  window.addEventListener("unhandledrejection", (ev) => reportUiError("unhandledrejection", ev.reason));
}

interface State {
  error: Error | null;
}

/** Uma falha num componente não deixa a janela em branco: mostra o erro e permite
 *  recarregar a interface. O catálogo já está gravado (cada alteração é gravada logo). */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    reportUiError("react", Object.assign(new Error(error.message), { stack: `${error.stack ?? ""}\n${info.componentStack ?? ""}` }));
  }

  render() {
    if (!this.state.error) return this.props.children;
    const t = i18n.t.bind(i18n);
    return (
      <div className="app-error" role="alert">
        <h1>{t("uiError.title")}</h1>
        <p>{t("uiError.body")}</p>
        <pre className="small muted">{this.state.error.message}</pre>
        <button type="button" className="btn primary" onClick={() => window.location.reload()}>
          {t("uiError.reload")}
        </button>
      </div>
    );
  }
}
