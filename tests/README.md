# Testes

- Testes unitários Rust: junto ao código (`#[cfg(test)]`) em cada crate de `packages/`.
- Testes TypeScript: ficheiros `*.test.ts(x)` junto ao código.
- `tests/fixtures/`: apenas imagens **sintéticas** geradas por script (Fase 1). Nunca
  fotografias reais ou de clientes.
- Testes com ficheiros RAW reais (Fase 2) leem a pasta indicada em `MOMENTO_SAMPLES_DIR`,
  fora do repositório, e são ignorados quando a variável não está definida.

Ver `TESTING.md`.
