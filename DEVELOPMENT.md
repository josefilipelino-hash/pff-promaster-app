# Desenvolvimento

## Comandos

| Comando | Efeito |
|---|---|
| `pnpm install` | Instala dependências JS (respeita o lockfile) |
| `pnpm dev` | Aplicação completa (Vite + Tauri) em modo desenvolvimento |
| `pnpm web:dev` | Só a interface no navegador em `http://localhost:1420` (sem núcleo) |
| `pnpm build` | Build de produção e instaladores |
| `pnpm test` | Testes TypeScript |
| `pnpm typecheck` | Verificação de tipos TypeScript |
| `cargo test --workspace` | Testes Rust |
| `./scripts/check.sh` | Todas as verificações (o mesmo que o CI) |

## Convenções

- **Idioma:** código, identificadores e mensagens de commit técnicas em inglês ou
  português, de forma consistente por ficheiro; **documentação e interface em pt-PT**.
- **Textos da interface:** nunca no código. Adicionar a chave em
  `packages/localization/locales/pt-PT.json` e em todos os outros idiomas existentes (o
  teste falha se faltar uma chave).
- **Erros do núcleo** chegam à interface como `ErrorPayload { code, detail }`; `code` é
  uma chave de tradução (`error.*`).
- **Rust:** `cargo fmt`, `clippy -D warnings`, sem `unwrap()` fora de testes no código de
  produção; erros com `thiserror`; nada de `unsafe` fora dos crates `-sys` e módulos de
  plataforma, sempre com comentário `// SAFETY:`.
- **Dependências:** antes de adicionar, verificar licença, manutenção e alternativa já
  existente; registar em `LICENSES.md`. Versões JS com pelo menos 7 dias
  (`minimumReleaseAge`).
- **Base de dados:** nunca editar uma migração publicada; criar uma nova no fim de
  `MIGRATIONS`.
- **Receita de edição:** campos novos opcionais com valor neutro; mudar o significado
  de um campo exige incrementar `RECIPE_SCHEMA_VERSION` e migração.
- **Sem botões falsos:** nenhum controlo na interface sem funcionalidade real (há um teste
  que o verifica no ecrã atual).

## Git

- Branch principal `main`; trabalho de cada fase em `fase-N-<descrição>` quando houver
  risco, com fusão após `./scripts/check.sh` passar.
- Um commit por passo coerente (checkpoint); mensagens no formato
  `tipo(âmbito): descrição` (ex.: `feat(import): retoma de importação`).
- Tags por fase concluída: `fase-0`, `fase-1`, …
- **Nunca** versionar fotografias, modelos de IA, credenciais ou `.env` (`.gitignore` já
  os exclui).

## Adicionar um módulo Rust

1. Criar `packages/<nome>/` com `Cargo.toml` usando `version.workspace = true` etc.
2. Adicionar a `members` no `Cargo.toml` da raiz e, se for usado por outros, a
   `[workspace.dependencies]`.
3. Documentar a responsabilidade em `ARCHITECTURE.md §3`.

## Adicionar um codec

1. Implementar `ImageCodec` num crate adequado (ou novo crate), atrás de uma feature.
2. Se o formato for novo, acrescentar a assinatura em `image-codecs/src/detect.rs` com teste.
3. Registar no `CodecRegistry` na inicialização da aplicação.
4. Atualizar `SUPPORTED_FORMATS.md` — só marcar “testado” com ficheiro real verificado.

## Depuração

- Registos Rust: `RUST_LOG=debug pnpm dev`.
- DevTools da WebView: clique direito → Inspecionar (só em builds de desenvolvimento).
