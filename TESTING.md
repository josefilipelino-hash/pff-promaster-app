# Testes

## Estratégia

| Nível | Ferramenta | Onde | Estado (Fase 1) |
|---|---|---|---|
| Unitários Rust | `cargo test` | `#[cfg(test)]` em cada crate | ✅ 63 testes |
| Integração do núcleo | `cargo test` com projetos temporários | `packages/project-management/tests/full_flow.rs` | ✅ 4 testes (até 625 ficheiros) |
| Unitários TS + componentes | Vitest + Testing Library (jsdom, núcleo simulado) | `apps/desktop/src/**/*.test.ts(x)`, `packages/localization` | ✅ 34 testes |
| Aplicação real (manual automatizada) | binário release + Xvfb + xdotool | ver “Verificação na aplicação real” | ✅ executada (Linux) |
| Compilação para Windows | `cargo check --target x86_64-pc-windows-gnu` | todo o workspace | ✅ sem erros nem avisos |
| CI | GitHub Actions: macOS 14, Windows 2022, Ubuntu 24.04 | `.github/workflows/ci.yml` | ⏳ configurado, ainda não executado (repositório sem remoto) |
| Ficheiros reais de câmara | `MOMENTO_SAMPLES_DIR` (fora do Git) | Fase 2 | — aguarda amostras |
| Regressão de imagem | comparação por tolerância | Fase 4 | — |
| E2E automático com WebDriver | `tauri-driver` | futuro | — |

Regras:
- **Nunca** fotografias reais ou de clientes como fixtures versionadas. As fixtures são
  sintéticas, geradas por `packages/test-fixtures` (gradientes com uma barra vermelha no
  topo e um quadrado azul no canto superior esquerdo, para verificar a orientação).
- Um formato só é “testado com ficheiro real” em `SUPPORTED_FORMATS.md` depois de um teste
  com um ficheiro de câmara real.

## Executar

```bash
./scripts/check.sh
```

```bash
cargo test -p momento-project-management --release -- --nocapture
```

## O que está coberto

**Base de dados (16)**: migrações v1→v2, WAL, recusa de catálogos mais recentes, cópia de
segurança, projeto (validação de data/tipo/nome, um por catálogo), sessões de importação
e retoma, inserção com duplicados por conteúdo e por caminho, classificação persistida após
reabrir, valores inválidos recusados, ordenação por data de captura e dimensões orientadas,
estados das miniaturas, estado da interface, projetos recentes.

**Metadados (5)**: todos os campos EXIF a partir de JPEG; bloco `eXIf` em PNG; EXIF ausente
ou corrompido não é erro; orientação inválida → 1; normalização de datas.

**Codecs (14)**: deteção de 32 formatos por assinatura; conteúdo vence extensão; 5 000
entradas aleatórias sem pânico; JPEG/PNG/TIFF: probe + descodificação; redução DCT do
JPEG; TIFF 16 bits; dados corrompidos devolvem erro; PNG transparente sobre branco;
registo com prioridades e recurso ao adaptador seguinte.

**Miniaturas (4)**: orientação correta nas **8** orientações EXIF; sem ampliação; perfil
ICC preservado; escrita atómica.

**Importação (8)**: pasta recursiva com duplicados e não suportados; reimportação →
“já importado”; não recursiva; ficheiros individuais; modo cópia (verificação, por câmara,
originais intactos); originais nunca modificados (conteúdo e data); suspensão + retoma;
cancelamento não retomado; pasta do projeto nunca importada.

**Fila (8)**: FIFO sem duplicados; prioridade e substituição; fecho liberta threads;
10 000 itens com 8 threads processados uma só vez; modelo de estados.

**Fluxo completo (4)**: criar → importar 625 ficheiros → miniaturas → previews (JPEG
serve o original, TIFF gera cache) → classificar → fechar → reabrir → tudo igual; fecho a
meio da importação e retoma; modo cópia; recusa de pastas que não são projetos.

**Interface (34)**: filtros, seleção (clique, Cmd/Ctrl, Shift, setas), atalhos (Cmd no
macOS / Ctrl no Windows), formatação pt-PT, datas DD/MM/AAAA, URL de imagens por
plataforma, ecrã inicial, validação do formulário, grelha virtualizada com 3 000 itens,
secções futuras desativadas, classificação por teclado e em seleção múltipla, filtro
Favoritas + visualizador, reversão quando o núcleo recusa, paridade de traduções.

## Verificação na aplicação real (2026-11-08)

Binário release em Linux (WebKitGTK) num ecrã virtual 1440×900, conduzido com `xdotool`
(cliques e teclado reais), com 529 ficheiros sintéticos de 3000×2000 px:

1. Ecrã inicial → **Novo projeto** → formulário preenchido → pasta
   `2026-06-15_Ana_e_Pedro.momento` criada.
2. **Importar pasta…** (seletor nativo) → 548 ficheiros analisados em < 1 s: 533
   importadas, 6 duplicadas, 15 não suportadas com motivo; pasta do projeto ignorada.
3. 533 miniaturas geradas em ~3 s, todas orientadas corretamente; 1 falhada (JPEG truncado,
   com mensagem no painel).
4. Classificação por teclado (estrelas, favorita, rejeitada, cores) e por seleção múltipla
   (Shift+clique); contadores da barra lateral atualizados; EXIF no painel.
5. Visualizador: ajustar, 100 % com arrastamento até ao canto (original JPEG com
   orientação EXIF 6 aplicada), seguinte/anterior, ecrã completo.
6. Filtro Favoritas + tamanho das miniaturas.
7. **Processo terminado com `kill -9`** → reaberto → projeto nos recentes → tudo igual
   (classificações, filtro, tamanho), confirmado também diretamente no SQLite.

Defeitos encontrados e corrigidos durante esta verificação: campo de data nativo
inutilizável no WebKitGTK (substituído por DD/MM/AAAA), deslocamento do zoom não
recalculado ao entrar em ecrã completo, texto cortado no painel, barra de ferramentas em
duas linhas.

**Não verificado automaticamente:** arrastar e largar ficheiros do sistema (o mesmo código
de importação é usado, mas o evento nativo de arrastar não é simulável no ecrã virtual) e a
execução real em macOS e Windows.

## Verificação manual sugerida (no teu Mac ou PC)

1. Gerar fotografias de teste (ou usar uma cópia de fotografias tuas):
   `cargo run -p momento-test-fixtures --release --bin generate-fixtures -- ~/momento-teste 500 3000`
2. `pnpm dev` → Novo projeto → Importar pasta… → `~/momento-teste`.
3. Arrastar alguns ficheiros do Finder/Explorador para a janela (arrastar e largar).
4. Classificar com 1–5, P, X, 6–9; abrir com Enter; Z para 100 %; F para ecrã completo.
5. Fechar a aplicação (Cmd+Q / Alt+F4), reabrir, abrir o projeto recente e confirmar que
   está tudo igual.
