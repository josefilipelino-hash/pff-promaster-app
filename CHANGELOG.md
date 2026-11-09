# Changelog

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/);
versões seguem [SemVer](https://semver.org/lang/pt-BR/) (0.x = em desenvolvimento).

## [Por lançar]

## [0.6.0] — 2026-11-09 — Fase 6: edição automática

### Adicionado
- **Edição automática** por fotografia (cada uma é medida; nada de parâmetros fixos):
  exposição (com rostos), balanço de brancos, realces, sombras, brancos, pretos, contraste,
  curva, vibração/saturação, redução de ruído (nível medido + ISO + tamanho do sensor),
  nitidez, horizonte, aberração cromática e vinheta da objetiva. Relatório do que foi
  medido. Atalho Ctrl/⌘+U.
- Intensidades: Muito suave, Suave, Natural, Equilibrada, Intensa, Personalizada (força e
  grupos de ajustes).
- **AI Denoise** local opcional (FFDNet, KAIR, MIT) com o nível de ruído medido; redução
  rápida com novo controlo de preservação de detalhe.
- Recorte automático opcional (com pessoas), correção de **olhos vermelhos** e remoção de
  **manchas do sensor** (bloco de receita `retouch`, não destrutivo).
- **Editar automaticamente seleção**: fila em segundo plano com progresso, pausa, retoma
  e cancelamento.
- Antes/depois **dividido** com divisória arrastável (tecla Y).

### Corrigido
- A aberração cromática junto à borda criava uma linha colorida (amostragem fora da imagem).

## [0.5.0] — 2026-11-09 — Fase 5: motor de edição

### Adicionado
- Motor de revelação **não destrutivo** em `f32` (Rec. 2020 linear → sRGB): exposição,
  contraste, realces, sombras, brancos, pretos; temperatura, matiz e conta-gotas de balanço
  de brancos; saturação e vibração; HSL por 8 cores; curva paramétrica, curva de pontos RGB
  e por canal; nitidez (quantidade, raio, detalhe, máscara), ruído de luminância e de cor;
  textura, clareza, remover névoa, vinheta pós-recorte, grão; recorte com proporções,
  rotação de 90°, espelho, endireitar e endireitar automático, perspetiva vertical e
  horizontal; distorção, aberração cromática e vinheta da objetiva.
- RAW editados a partir dos dados lineares do sensor (LibRaw).
- Qualidade dinâmica (render rápido ao arrastar, final ao parar) e histograma RGB.
- Desfazer/refazer, histórico, repor, antes/depois, copiar/colar/sincronizar definições,
  cópias virtuais, instantâneos.
- Miniaturas, previews e resolução total editadas em cache; sidecar JSON na pasta do
  projeto. Catálogo v6.


## [0.4.0] — 2026-11-09 — Fase 4: organização do casamento

### Adicionado
- **Pessoas:** identidade facial local (SFace, OpenCV Zoo, Apache-2.0) com alinhamento por
  5 pontos; agrupamento automático (dois rostos da mesma fotografia nunca são a mesma
  pessoa); identificação com nome e papel (noiva, noivo, mãe/pai da noiva e do noivo,
  irmãos, padrinho, madrinha, pessoa importante); juntar grupos, “não é esta pessoa”,
  ocultar, reagrupar, apagar dados de pessoas. As decisões sobrevivem à reanálise.
- **Pesquisas:** todas as fotografias da noiva/do noivo, noivos juntos, noiva + mãe,
  noivo + pai, qualquer pessoa (combinações com “e”).
- **Momentos:** 25 momentos de casamento classificados automaticamente só com confiança
  suficiente (hora corrigida, cerimónia detetada, GPS, cena/objetos com EfficientNet-Lite2,
  rostos, pessoas identificadas, semelhança com os exemplos do utilizador);
  “Por classificar”; momentos personalizados; arrastar fotografias para um momento; escolha
  no painel; voltar ao automático.
- **Sincronização de câmaras:** desvio por câmara (“Sony +3min42s”), sugestão automática
  pela semelhança visual entre câmaras, cronologia, sequências e momentos reconstruídos;
  a hora EXIF original nunca é alterada.
- GPS lido do EXIF. Catálogo v5.


## [0.3.0] — 2026-11-09 — Fase 3: seleção assistida

### Adicionado
- Análise **local** em segundo plano (retomável): nitidez global, pico de foco e centro,
  tremido, exposição, realces queimados, sombras bloqueadas, ruído, inclinação, pHash/dHash.
- IA local (modelos MediaPipe, Apache-2.0, executados com `tract-onnx` em Rust puro):
  deteção de rostos multiescala, 478 pontos faciais, 52 blendshapes (olhos fechados,
  sorriso, boca aberta), nitidez do rosto e dos olhos, rostos cortados, embedding visual.
- Grupos: pares RAW+JPEG, duplicados quase idênticos, sequências/rajadas (tempo,
  semelhança visual, disposição dos rostos, várias câmaras).
- Recomendações **MELHOR / ALTERNATIVA / REVER / REJEITAR SUGERIDO / DUPLICADA** com
  pontuações individuais guardadas e explicação em frase (“Melhor fotografia desta
  sequência: maior nitidez facial, ambos os rostos visíveis e melhor exposição.”).
- Modos conservador / equilibrado / rigoroso e objetivo de entrega aproximado
  (recalculados sem reanalisar).
- Vista **Sequências**, **comparação lado a lado** (2–4) com zoom e deslocamento
  sincronizados, estrelas, aceitar, rejeitar e “selecionar outra”; tecla **C**.
- “Aceitar sugestões” por fotografia ou grupo (explícito, reversível, só sem decisão
  prévia). Nada é apagado nem alterado automaticamente.
- Catálogo v4 (análise, rostos, embeddings, grupos, recomendações).
- `scripts/prepare-models.py` e passo de modelos no CI; modelos incluídos no instalador.

## [0.2.0] — 2026-11-08 — Fase 2: formatos e RAW

### Adicionado
- RAW via **LibRaw 0.22.3** (submódulo, CDDL-1.0) compilada do código-fonte com camada C
  própria que isola exceções C++; preview embutida primeiro, revelação a meia resolução ou
  completa só quando necessário; metadados, bits, cor do sensor e descodificador.
- Validado com ficheiros reais: Canon CR2/CR3/CRW, Nikon NEF, Sony ARW, Fujifilm RAF
  (Bayer e X-Trans), Panasonic RW2, Leica RWL/DNG, Olympus/OM ORF, Pentax PEF/DNG,
  Samsung SRW, Sigma X3F (incl. Quattro), Kodak DCR, DNG de DJI e de smartphone.
- Novos formatos: WebP, JPEG XL, JPEG 2000, PSD/PSB (imagem composta, leitor próprio),
  BigTIFF, BMP, GIF, OpenEXR, HDR, ICO, PNM/PAM, TGA.
- HEIC/HEIF/AVIF: adaptadores ImageIO (macOS) e WIC (Windows) — compilados, a validar.
- Estados por ficheiro: importado, **suporte parcial** (com explicação), **corrompido**,
  **codec em falta**, não suportado — no relatório, na grelha (selo “Parcial”) e no painel.
- Painel de informação: profundidade de bits, perfil/espaço de cor (descrição ICC),
  descodificador, preview embutida.
- Cache para milhares de RAW: preview de ecrã gerada na mesma leitura que a miniatura;
  limites LRU (previews 10 GB, resolução total 5 GB).
- Testes de capacidade com ficheiros reais (`MOMENTO_SAMPLES_DIR`).
- Catálogo v3 (migração automática a partir da v2).
- `scripts/pin-mature-crates.py`: política de 7 dias também para crates Rust.

### Corrigido
- Previews RAW já rodadas pela câmara eram rodadas duas vezes (Nikon D70, Kodak DCS).
- Compilação da LibRaw no Windows (constantes `M_PI`) — encontrado por compilação cruzada.

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
