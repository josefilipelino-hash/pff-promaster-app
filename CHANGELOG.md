# Changelog

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/);
versões seguem [SemVer](https://semver.org/lang/pt-BR/) (0.x = em desenvolvimento).

## [Por lançar]

## [0.1.0] — 2026-11-08 — Fase 1: aplicação utilizável

### Adicionado
- Ecrã inicial com projetos recentes (disponibilidade do disco, remover da lista).
- Criar projeto: nome do evento, noivos/cliente, tipo de evento, data, local, pasta dos
  originais e pasta de trabalho; pasta `AAAA-MM-DD_Cliente.momento` criada automaticamente.
- Abrir projetos existentes; estado da interface (filtro, tamanho, fotografia com foco,
  painel) restaurado ao reabrir.
- Importação real de JPEG, PNG e TIFF: ficheiros, múltiplos ficheiros, pastas, subpastas,
  arrastar e largar; modos “manter no local” e “copiar para a pasta dos originais” (por
  câmara, com verificação BLAKE3); deteção de duplicados por conteúdo e de ficheiros já
  importados; relatório com motivos; cancelar; retoma automática após fecho.
- Leitura de EXIF (data/hora, câmara, objetiva, ISO, velocidade, abertura, focal, orientação).
- Miniaturas em segundo plano com prioridade para as visíveis; previews 2560 px e resolução
  total a pedido; orientação EXIF aplicada; perfil ICC preservado.
- Grelha virtualizada com tamanho ajustável, seleção simples/múltipla (Cmd/Ctrl, Shift).
- Visualizador: ajustar, 100 % (píxeis reais do ecrã), zoom com roda/teclado, arrastar,
  anterior/seguinte, ecrã completo.
- Classificação: 0–5 estrelas, favorita, rejeitada, 5 etiquetas de cor; aplicada à seleção.
- Barra lateral: Todas, Favoritas, Rejeitadas, Sem classificação (Momentos, Pessoas e
  Exportação visíveis como desativadas, com a fase em que chegam); filtros por estrelas e cor.
- Painel de informação com metadados; atalhos de teclado e ajuda (tecla ?).
- Catálogo v2 (projeto, fotografias, variantes, classificação, sessões de importação);
  biblioteca global de projetos recentes; cópias de segurança automáticas do catálogo.
- Registos em ficheiro (pasta de registos da aplicação).
- CI em macOS, Windows e Linux, com instaladores `.dmg` (universal) e `.exe` (NSIS).
- Gerador de fotografias sintéticas (`generate-fixtures`) e teste de fluxo completo.

### Corrigido
- Orientações EXIF 5 e 7 (detetado pelos testes antes de chegar à interface).

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
