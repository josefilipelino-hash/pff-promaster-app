# Changelog

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/);
versões seguem [SemVer](https://semver.org/lang/pt-BR/) (0.x = em desenvolvimento).

## [Por lançar]

## [0.0.1] — 2026-11-08 — Fase 0

### Adicionado
- Estrutura do repositório: workspace Cargo (17 crates) e workspace pnpm (3 pacotes).
- Aplicação Tauri 2 com ecrã de diagnóstico em pt-PT (comando `app_info`).
- `momento-core-types`: IDs tipados, `CoreError`/`ErrorPayload`, envelope `EditRecipe`
  com preservação de campos futuros.
- `momento-database`: abertura de catálogo SQLite (WAL), migrações por `user_version`,
  recusa de catálogos mais recentes.
- `momento-image-codecs`: deteção de formatos por assinatura (32 formatos e contentores),
  contrato `ImageCodec` e `CodecRegistry` com prioridades.
- `momento-task-queue`: prioridades e máquina de estados com recuperação após falha.
- `@momento/localization`: pt-PT e en, teste de igualdade de chaves.
- CI (GitHub Actions) para macOS e Linux; `scripts/check.sh`.
- Documentação: arquitetura, roadmap, estado, instalação, desenvolvimento, formatos,
  privacidade, segurança, licenças, testes.
