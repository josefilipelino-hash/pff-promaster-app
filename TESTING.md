# Testes

## Estratégia

| Nível | Ferramenta | Onde | Estado |
|---|---|---|---|
| Unitários Rust | `cargo test` | `#[cfg(test)]` em cada crate | ✅ 20 testes |
| Unitários TS | Vitest | `*.test.ts(x)` | ✅ 9 testes |
| Componentes React | Vitest + Testing Library (jsdom) | `apps/desktop/src` | ✅ incluído acima |
| Integração do núcleo | `cargo test` com catálogos temporários | `packages/*/tests` | Fase 1 |
| Compatibilidade de formatos | ficheiros reais em `MOMENTO_SAMPLES_DIR` (fora do Git) | Fase 2 | — |
| Regressão de imagem | comparação por tolerância (ΔE / PSNR) com imagens de referência sintéticas | Fase 4 | — |
| Desempenho | benchmarks (`criterion`) e projetos sintéticos de 15 000 itens | Fase 1 (grelha), Fase 9 | — |
| E2E | `tauri-driver` (Linux/Windows) + verificação manual no macOS | Fase 1+ | — |

Regras:
- Nunca usar fotografias reais ou de clientes como fixtures versionadas. As fixtures são
  geradas por script (`tests/fixtures/generated/`, ignorada pelo Git).
- Testes que precisam de ficheiros reais são ignorados quando `MOMENTO_SAMPLES_DIR` não
  está definido, e nunca copiam os ficheiros para o repositório.
- Um formato só é “suportado” em `SUPPORTED_FORMATS.md` depois de um teste com ficheiro
  real.

## Executar

```bash
./scripts/check.sh
```

Ou individualmente:

```bash
pnpm test
```

```bash
cargo test --workspace
```

## Testes existentes (Fase 0)

**Rust (20)**
- `core-types` (4): payload de erro usa chave de tradução; IDs serializam como números;
  receita vazia é identidade; campos futuros sobrevivem a ida e volta.
- `database` (3): catálogo novo migrado e em WAL; reabertura idempotente; recusa de
  catálogo de versão superior.
- `image-codecs` (8): rasters comuns; marcas ISOBMFF (HEIC/AVIF/HEIF/CR3, rejeição de MP4);
  assinaturas RAW; RAW sobre TIFF exige assinatura + extensão; conteúdo vence extensão;
  formatos só por extensão marcados; 5 000 entradas aleatórias sem pânico; registo
  ordena por prioridade e filtra.
- `task-queue` (4): ordem de prioridades; estados terminais; recuperação após falha;
  serialização.
- `desktop` (1): `app_info` devolve versões em camelCase.

**TypeScript (9)**
- `localization` (6): pt-PT por omissão; mesmas chaves e sem textos vazios em pt-PT e en;
  chaves de erro do núcleo existem.
- `desktop` (3): textos pt-PT vindos das traduções; diagnóstico do núcleo; nenhum botão
  no ecrã (regra “sem botões sem funcionalidade”).

## Verificação manual (Fase 0)

1. `pnpm dev`.
2. Deve abrir a janela “Momento Studio” com o diagnóstico preenchido (não a mensagem
   “Núcleo indisponível”). Isso confirma a ponte interface ↔ Rust.
3. `pnpm web:dev` e abrir `http://localhost:1420` num navegador: deve mostrar “Núcleo
   indisponível…”, que é o comportamento correto fora da aplicação.
