# Estado do projeto

**Última atualização:** 2026-11-08 · **Fase atual:** 1 — Aplicação utilizável · **Estado:** ✅ concluída
**Versão:** 0.1.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

É possível **criar um projeto → importar fotografias → classificá-las → fechar a aplicação
→ voltar a abrir → encontrar tudo exatamente como foi deixado**. Verificado em testes
automáticos com 625 ficheiros e na aplicação real (binário release, cliques e teclado
reais) com 529 fotografias de 3000×2000 px, incluindo um fecho abrupto (`kill -9`).

Ainda não lê RAW, não tem IA e não edita (Fases 2, 3 e 4).

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Ecrã inicial com projetos recentes (nome, cliente, tipo, data, local, n.º de fotografias, última abertura, aviso de disco desligado, remover da lista) | App real + teste de componente |
| Novo projeto: nome do evento, noivos/cliente, tipo de evento, data (DD/MM/AAAA), local, pasta dos originais, pasta de trabalho; cria `AAAA-MM-DD_Cliente.momento` | App real + testes Rust/TS |
| Abrir projeto existente (seletor de pasta) e a partir dos recentes | App real |
| Importação de JPEG/JPG, PNG e TIFF/TIF: ficheiro, múltiplos ficheiros, pasta, subpastas | Testes Rust + app real (pasta) |
| Arrastar e largar | Implementado com o evento nativo do Tauri (mesmo código da importação) — **não testado automaticamente** |
| Formato detetado pelo conteúdo (não pela extensão) | Testes |
| Duplicados por conteúdo (BLAKE3) e ficheiros já importados | Testes + app real (6 duplicados detetados) |
| Modo “manter no local” e modo “copiar para a pasta dos originais” (por câmara, com verificação) | Testes |
| Relatório de importação com motivos; cancelar; retoma automática após fecho | Testes + app real |
| EXIF: data/hora (+subsegundos, fuso), câmara, objetiva, ISO, velocidade, abertura, focal (+35 mm), n.º de série | Testes + painel na app real |
| Orientação automática (8 orientações EXIF) em miniaturas, previews e 100 % | Testes + app real |
| Miniaturas reais em segundo plano, com prioridade para as visíveis e retoma | Testes + app real (533 em ~3 s) |
| Grelha virtualizada, tamanho ajustável, seleção simples e múltipla (Cmd/Ctrl, Shift, setas, selecionar tudo) | Testes + app real |
| Visualizador: abrir, zoom (roda, +/−, níveis), ajustar, 100 % em píxeis reais, arrastar, anterior/seguinte, ecrã completo | App real |
| Classificação 0–5 estrelas, favorita, rejeitada, 5 etiquetas de cor, aplicada à seleção | Testes + app real |
| Barra lateral: Todas, Favoritas, Rejeitadas, Sem classificação (com contadores) | Testes + app real |
| Momentos, Pessoas, Exportação: visíveis, **desativados** e marcados “Fase 5/6” (não fingem funcionar) | Teste de componente |
| Filtros por estrelas mínimas e etiqueta de cor (barra inferior) | Testes |
| Painel de informação com metadados e classificação | App real |
| Atalhos de teclado + ajuda (tecla ?) | Testes |
| Persistência total em SQLite; estado da interface restaurado; cópias de segurança automáticas do catálogo | Testes + app real com `kill -9` |
| Registos em ficheiro | Implementado |
| Interface 100 % em português de Portugal (traduções em ficheiros; inglês preparado) | Teste de paridade |
| Compilação de todo o workspace para Windows | `cargo check --target x86_64-pc-windows-gnu` ✅ |

## Não funcional / não implementado

- RAW, HEIC, WebP, AVIF e restantes formatos (Fase 2) — registados como “não suportados”.
- IA, seleção automática, duplicados semelhantes (Fase 3); edição (Fase 4); momentos e
  pessoas (Fase 5); exportação (Fase 6); galerias (Fase 7); álbuns (Fase 8).
- Gestão de cor no núcleo (LittleCMS): os perfis ICC são preservados e o WebView gere a cor,
  mas não há conversão nem soft proofing (Fase 2/6).
- Relocalizar originais movidos (a miniatura falha com “original não encontrado”).
- Aviso automático quando o catálogo está num disco de rede.
- Menus de contexto, desfazer/refazer, tema claro, segundo monitor.
- Verificação automática de licenças transitivas (`cargo-deny`).

## Arquitetura (sem alterações à arquitetura principal)

Tauri 2.12 + React 19/TypeScript + Rust + SQLite, conforme `ARCHITECTURE.md`. Decisões
novas desta fase (D-011 a D-015):
- **D-011** Windows e macOS como plataformas-alvo desde já (CI em ambos, compilação cruzada,
  só bibliotecas Rust puras nesta fase).
- **D-012** Trabalho em segundo plano persistido no próprio domínio (`import_item.state`,
  `asset.thumb_state`) em vez de uma tabela genérica `job`.
- **D-013** Miniaturas 640 px; previews 2560 px a pedido; 100 %: original (JPEG/PNG) ou
  JPEG em cache (TIFF).
- **D-014** JPEG com redução DCT (`jpeg-decoder`) para miniaturas rápidas.
- **D-015** Classificação na variante principal (preparado para cópias virtuais).

## Desempenho medido (Linux x86_64, 8 núcleos, ficheiros sintéticos)

| Operação | Volume | Tempo |
|---|---|---|
| Importação (leitura, deteção, hash, EXIF, gravação) | 625 ficheiros de 1200 px | 0,07 s (release) |
| Importação na app real | 548 ficheiros de 3000 px (151 MB) | < 1 s |
| Miniaturas | 622 de 1200 px | 4,6 s |
| Miniaturas na app real | 533 de 3000 px | ~3 s |
| Grelha | 3 000 itens | < 200 elementos no DOM (virtualizada) |

Ficheiros sintéticos comprimem muito melhor do que fotografias reais: com JPEG reais de
24–45 MP (8–25 MB) os tempos serão maiores e dominados pela leitura do disco/cartão.

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Não houve execução em macOS nem Windows reais nesta sessão | Médio | CI em macOS 14 e Windows 2022 pronto; compilação cruzada para Windows sem erros; verificação manual descrita abaixo |
| Fotografias reais (perfis ICC, CMYK, EXIF de fabricante) não testadas | Médio | Amostras pedidas; testes com `MOMENTO_SAMPLES_DIR` na Fase 2 |
| Fidelidade de cor na WebView | Médio | ICC preservado; LittleCMS na Fase 2 |
| Originais em discos externos desligados | Baixo | Projetos marcados “pasta não encontrada”; relocalização na Fase 2 |
| Teste de fluxo completo demora ~90 s em debug | Baixo | Aceitável no CI; versão release em 12 s |

## Comandos exatos

Preparação de macOS ou Windows: ver [`INSTALLATION.md`](INSTALLATION.md).

```bash
pnpm install
```

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

```bash
pnpm build
```

Gerar 500 fotografias de teste:

```bash
cargo run -p momento-test-fixtures --release --bin generate-fixtures -- ~/momento-teste 500 3000
```

## Testes realizados (2026-11-08)

| Verificação | Resultado |
|---|---|
| `cargo test --workspace` | ✅ 67/67 (63 unitários + 4 de integração) |
| `pnpm -r test` | ✅ 34/34 |
| `pnpm -r typecheck` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ |
| `cargo fmt --all --check` | ✅ |
| `cargo check --workspace --all-targets --target x86_64-pc-windows-gnu` | ✅ sem erros nem avisos |
| Executável Windows (`momento-desktop.exe`, compilação cruzada x86_64-pc-windows-gnu, release) | ✅ gerado: PE32+ GUI x86-64, 14 MB — não executado (sem Windows disponível) |
| Aplicação real (release, Linux, Xvfb + xdotool) | ✅ fluxo completo; detalhes em `TESTING.md` |
| macOS / Windows reais | ⏳ não executado (CI configurado) |

## Histórico de fases

| Fase | Estado | Data |
|---|---|---|
| 0 — Fundação técnica | ✅ | 2026-11-08 |
| 1 — Aplicação utilizável | ✅ | 2026-11-08 |
| 2 — Formatos e RAW | ⏸ próxima | — |

## Próximo passo

**Fase 2 — Formatos e RAW**: LibRaw (DNG, CR2, CR3, NEF, ARW, RAF, ORF, RW2…) com preview
embutida na importação; HEIC/HEIF (ImageIO no macOS, alternativa no Windows); WebP, AVIF,
JPEG XL, PSD; IPTC/XMP; LittleCMS para previews; relocalização de originais;
sincronização de hora entre câmaras.

**Critério de conclusão:** cada formato anunciado tem um ficheiro real verificado em
`SUPPORTED_FORMATS.md`; importação de RAW com miniatura em < 50 ms por ficheiro (preview
embutida); CI verde em macOS e Windows.

**Pendente do utilizador:**
1. Repositório GitHub (para o CI correr em macOS e Windows e gerar os instaladores).
2. 5–10 ficheiros RAW/JPEG reais por câmara que uses (nunca entram no Git).
3. Nome definitivo da aplicação (o identificador `pt.momentostudio.desktop` define a pasta
   de dados).
4. Modelo do Mac e versão do Windows que vais usar.
