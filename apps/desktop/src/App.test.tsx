import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import "./i18n";
import { App } from "./App";

afterEach(cleanup);

describe("App (Fase 0)", () => {
  it("mostra textos em pt-PT vindos dos ficheiros de tradução", async () => {
    render(<App loadInfo={async () => null} />);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Momento Studio");
    expect(await screen.findByText(/fora da aplicação desktop/)).toBeTruthy();
  });

  it("mostra a informação do núcleo quando disponível", async () => {
    const info = { appVersion: "0.0.1", coreVersion: "0.0.1", catalogSchemaVersion: 1, os: "macos", arch: "aarch64" };
    render(<App loadInfo={async () => info} />);
    expect(await screen.findByText("macos / aarch64")).toBeTruthy();
    expect(screen.getByText("Versão do esquema do catálogo")).toBeTruthy();
  });

  it("não contém botões (nenhuma ação sem funcionalidade)", () => {
    render(<App loadInfo={async () => null} />);
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });
});
