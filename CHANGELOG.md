# Changelog

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/);
versões seguem [SemVer](https://semver.org/lang/pt-BR/) (0.x = em desenvolvimento).

## [Por lançar]

## [1.1.2-personal] — 2026-11-12

### Corrigido
- Canon EOS C50 (e outras câmaras sem perfil na LibRaw) com **dominante verde/oliva** depois
  da correção da 1.1.1: sem perfil, a LibRaw também não aplica o balanço de brancos da
  câmara. A calibração pela preview passa a usar uma matriz completa (corrige também o
  balanço). Teste com RAW reais sem balanço: erro de cor 0,56 → 0,02–0,06 (o mínimo
  possível, igual ao da própria preview). As edições em cache são refeitas (versão do motor).

## [1.1.1-personal] — 2026-11-12

### Corrigido
- **Câmaras mais recentes do que o motor RAW (ex.: Canon EOS C50) apareciam com uma
  dominante magenta/avermelhada no editor e nas exportações.** A LibRaw 0.22.3 não tem o
  perfil de cor destes modelos. A aplicação passa a detetá-lo e a calibrar as cores de cada
  fotografia pela preview JPEG que a própria câmara grava no RAW (matriz 3×3, cinzentos
  mantidos neutros). Testado com RAW reais (Canon R3, Nikon D810, Sony A7 IV) a simular uma
  câmara desconhecida: as cores ficam tão próximas das corretas como a própria preview.
  Nestas câmaras a temperatura é mostrada na escala relativa (sem Kelvin).

## [1.1.0-personal] — 2026-11-11 — pedidos do utilizador

### Adicionado
- **Classificações para o Lightroom** (Seleção automática → "Classificações para o
  Lightroom…"): Melhor 5★, Alternativa 3★, Rever 2★ em ficheiros `.xmp` ao lado de cada RAW,
  que o Lightroom lê ao importar (ou em Metadados → Ler metadados dos ficheiros). Os RAW nunca
  são alterados; um `.xmp` existente só tem a classificação mudada (cópia
  `.xmp.pff-original`). Opções: manter as estrelas já dadas; aplicar também no PFF ProMaster.
  Um catálogo `.lrcat` não é gerado (formato privado, sem documentação — ver
  `KNOWN_LIMITATIONS.md`).
- **Temperatura em Kelvin para RAW**: calculada a partir do balanço da câmara (LibRaw) e usada
  como referência do balanço de brancos; controlo de 2000 a 50 000 K (escala logarítmica,
  arredondado a 50 K, aceita "5600" ou "5600 K"). JPEG e outros mantêm a escala relativa.
- **Editar várias fotografias ao mesmo tempo**: "Sincronizar auto" no editor (cada alteração
  vai também para as outras selecionadas — sem recorte, máscaras nem retoque); presets no
  editor aplicados a todas as selecionadas; seleção múltipla na tira do editor
  (Cmd/Ctrl+clique, Shift+clique).
- **Novo visual**: destaque coral → sol, cantos mais suaves, barra do fluxo do trabalho
  (Importar → Selecionar → Editar → Entregar) com o progresso real de cada passo.

### Corrigido
- macOS recente recusava abrir a aplicação (sem qualquer assinatura): passa a ter assinatura
  local *ad-hoc*; instruções para a primeira abertura em `INSTALLATION.md`.
- Windows: cópia do catálogo antes de migrar/restaurar ("Access is denied").
- Pilhas de 16 MB nas threads de processamento (esgotavam sob carga, visto em macOS).

## [1.0.0-personal] — 2026-11-10 — Fase 16 + auditoria final

Sem funcionalidades novas além das prometidas na documentação que não existiam. Ver
`RELEASE_NOTES.md`, `FINAL_AUDIT.md` e `KNOWN_LIMITATIONS.md`.

### Auditoria final — corrigido
- Realces e Sombras **locais** (máscaras) não tinham efeito no render.
- Perfil ICC embutido (Adobe RGB, Display P3, ProPhoto) ignorado na edição e exportação
  de ficheiros não-RAW (cores dessaturadas/erradas).
- Restaurar ou importar um catálogo deixava miniaturas de outras fotografias.
- Relatórios de falha sem o fim do registo (nome dos ficheiros de registo mudado na Fase 16).
- Registos com caminhos de projeto (nomes de clientes): agora anonimizados linha a linha.
- Galeria: favoritas do cliente perdidas ao regenerar; a anterior era apagada antes de a
  nova existir.
- Álbum: rosto cortado pelo enquadramento sem aviso.
- Favoritas com "Rejeitar?" sugerido (agora "Protegida"); pesquisas por papel com duas
  pessoas no mesmo papel escondiam fotografias.
- Cópias virtuais com nomes repetidos; "Mover para o Lixo" ignorava cópias virtuais.
- Edições em cache sem a versão do motor.

### Auditoria final — adicionado (prometido em PRIVACY.md e inexistente)
- Segurança e cópias → **Privacidade**: Limpar cache, Apagar dados de pessoas, Eliminar
  projeto (para o Lixo; nunca toca nos originais).
- Captura dos erros da interface (mensagem em vez de janela em branco; registo local).

### Alterado
- Nome **PFF ProMaster**, ícone próprio, identificador `pt.pffpromaster.desktop`.
- Removidos: código morto "Brevemente" da barra lateral e a lista de fornecedores de
  publicação com "Fase futura".
- Documentação alinhada com o comportamento real (SECURITY, ARCHITECTURE, ROADMAP,
  README, PRIVACY, RELEASE_NOTES, INSTALLATION, `.env.example`, `models/manifest.json`).

### Adicionado
- **Atualização segura:** cópia completa do catálogo em `backups/` antes de qualquer
  migração (sem cópia, nada é alterado); visível em "Segurança e cópias" como "antes da
  atualização". Testado com catálogos reais das versões v3 e v7.
- `Info.plist` com as explicações dos pedidos de acesso do macOS (volumes amovíveis,
  Documentos, Secretária, Transferências, rede); informações da aplicação (editor,
  copyright, descrições, janela do `.dmg`).
- `scripts/build-release.sh`: build de produção reproduzível (toolchain fixa, locks,
  modelos verificados por SHA-256, `SOURCE_DATE_EPOCH`, `BUILD_INFO.txt`, `SHA256SUMS.txt`);
  CI gera o `.dmg` universal em cada tag `v*`.
- `RELEASE_NOTES.md`, `docs/SIGNING.md` (assinatura/notarização, separado).

### Alterado
- Versão 1.0.0-personal (pacotes 1.0.0: o macOS e o Windows exigem versão numérica).
- Produção sem source maps (−2,3 MB) e sem informação de depuração (nomes de funções
  mantidos para os relatórios de falhas).
- Registos limitados aos últimos 14 dias.
- Rust fixado em 1.97.0 (`rust-toolchain.toml`).

### Corrigido (encontrado no smoke test do pacote final)
- Miniatura concluída durante o recarregar da lista ficava "pendente" no ecrã.
- "Analisar 0 fotografias" depois de uma importação (contagem não recarregada).
- Dicas com o atalho repetido ("(Mod + Shift + E) (Ctrl+Shift+E)").

## [0.15.0] — 2026-11-10 — Fase 15: estabilização

Sem funcionalidades grandes novas: auditoria dos 18 módulos, casamento completo como
teste de ponta a ponta, condições adversas com discos reais. Relatório completo em
`docs/reports/fase15-auditoria.md`; lista de lançamento em `RELEASE_CHECKLIST.md`.

### Corrigido — podia perder fotografias, edições ou entregar ficheiros errados
- **Importação:** um segundo cartão (ou o mesmo formatado) no mesmo caminho, com os mesmos
  nomes, era ignorado como "já importado" → agora decide-se pelo conteúdo; no modo "manter
  no local" é um erro explicado (encontrado na aplicação real).
- **Original substituído** por outra fotografia no mesmo caminho: marcado "em falta" e
  recusado na exportação (antes saía a fotografia nova com as edições da antiga).
- **Exportação:** "Substituir" podia escrever por cima dos originais (Windows `\\?\`,
  ligações simbólicas no macOS).
- **Develop:** resultados assíncronos (máscara IA, retrato, Auto, desfazer…) depois de mudar
  de fotografia escreviam ajustes de uma fotografia noutra; o último ajuste podia não ser
  gravado.
- **Sincronizar aspeto** copiava máscaras e regiões de retrato da referência.
- **Pessoas:** reanalisar sem modelos apagava nomes e exclusões confirmados.
- **Recuperação:** abrir um catálogo de uma versão mais recente (ou com disco cheio) trocava-o
  por uma cópia antiga.
- **Álbum:** fotografia indisponível dava molduras vazias com o trabalho "Concluído".
- **Auto em lote** revertia edições feitas ao mesmo tempo pelo utilizador.
- **Aceitar sugestões** rejeitava fotografias com estrelas.

### Corrigido — bloqueios
- Exportação para destino só de leitura/sem permissões ficava em pausa "sem espaço" para sempre.
- Erro de gravação durante a importação prendia a importação e o fecho do projeto.
- AI Denoise que falhasse deixava a fotografia "em curso" para sempre.
- Trabalho de exportação acrescentado no instante em que a fila terminava ficava "Na fila".
- Vídeos grandes no cartão eram lidos inteiros para a memória.

### Corrigido — outros
- Retoma de exportação: {seq} continua; receitas em falta de uma fotografia interrompida refeitas.
- LUT/máscara em falta: a fotografia falha com o motivo em vez de sair com outras cores.
- Marca de água grande demais é reduzida (antes desaparecia nos retratos).
- Galeria: destaque/capa oculto ou rejeitado bloqueia a geração.
- Álbum: resolução por moldura pelo pior caso; reexportação para pasta com páginas antigas recusada.
- Pastas ilegíveis e caminhos inexistentes aparecem no relatório de importação.
- Contagem de miniaturas inclui as que estão a ser descodificadas; fotografias ilegíveis já
  não ficam "por analisar".
- Teclas no editor e noutras vistas só agem sobre a fotografia mostrada; clique direito
  numa seleção múltipla mantém-na.
- Preset a 50/150 % mantém a cor da tonalização.

### Testes
- `wedding_e2e` (casamento completo com reinício), `adverse` (disco cheio ×4, só de leitura,
  permissões, fugas, LUT em falta), testes de regressão em cada correção.

## [0.14.0] — 2026-11-10 — Fase 14: UX/UI profissional

### Adicionado
- **Sistema de design** (`packages/ui`): tokens de cor, tipografia, espaço, forma e
  movimento; contraste WCAG verificado por teste. Referência em `DESIGN.md`.
- **Tema escuro profissional** (principal) e **tema claro** opcional, ou o do sistema
  (Mais ⋯ → Tema).
- **Menus contextuais** na grelha e na tira (clique direito, tecla Menu, Shift+F10) com as
  ações reais e o atalho de cada uma visível.
- **Tira de fotografias** virtualizada no visualizador e no Develop.
- **Navegação por teclado completa**: F6/Shift+F6 entre regiões, setas nos separadores,
  menus com setas/Home/End/letra, foco devolvido ao fechar.
- **48 ícones** próprios em SVG, nítidos em Retina/HiDPI; histograma desenhado à densidade
  real do ecrã.
- **Estados claros** (a carregar, a processar, offline, falhou, concluído) na barra de
  estado, na fila de exportação e sobre a fotografia; estados vazios com ícone e ação.
- Auditoria de acessibilidade automática (axe-core, WCAG 2.x A/AA) nos ecrãs principais.

### Alterado
- Barra superior numa linha que se adapta à largura (rótulos → ícones); Importar e opções
  em menus. Barras do visualizador, comparação e Develop com ícones e dicas de teclado
  discretas.
- Estados hover/focus/selecionado/desativado uniformes; foco visível só com teclado; sem
  animações decorativas (`prefers-reduced-motion` respeitado).
- Textos por traduzir: "Amount" → "Intensidade", temas da galeria, "Transferências",
  "Sangria total".

### Corrigido
- "Edição automática concluída: 0 fotografias" aparecia ao fechar um projeto sem fila.
- Acessibilidade: tira com itens de lista, molduras do álbum sem `aria-selected` inválido,
  caminho do ficheiro numa lista de definição válida.

## [0.13.0] — 2026-11-10 — Fase 13: fiabilidade

### Adicionado
- **Segurança e cópias** (barra lateral): estado dos originais, exportações interrompidas
  (retomar/descartar), versões do catálogo (criar, restaurar com confirmação, cópia
  automática a cada 15/30/60/240 min), validar, exportar e importar o catálogo, registo.
- Deteção de sessão interrompida (falha, energia, processo terminado): validação completa,
  limpeza de temporários, aviso e crash log; catálogo danificado restaurado da última
  versão válida (o danificado é guardado).
- Originais **offline** (disco desligado) e **em falta**, com reconexão automática,
  coleção "Indisponíveis", localizar original e relink por ficheiro ou pasta.
- Remover só do catálogo e mover para o lixo do sistema (Delete), sempre com confirmação.
- Importação: verificação das cópias (checksum, opcional) e cópia simultânea para um disco
  de segurança.
- Crash logs locais sem fotografias nem dados pessoais.
- `RELIABILITY.md`: comportamento em cada falha, com testes deliberados.

### Alterado
- Catálogo com `synchronous = FULL`; checkpoints a cada 5 minutos e no fecho.
- Cópias da importação atómicas; a retoma reutiliza cópias completas (sem `_1` órfãos).
- O mesmo projeto não pode ser aberto em duas janelas/instâncias.


## [0.12.0] — 2026-11-10 — Fase 12: desempenho

### Adicionado
- `PERFORMANCE.md`: benchmarks medidos com 15 000 fotografias e a resolução real (antes e
  depois), bottlenecks encontrados por profiling e limitações.
- Benchmark pelo caminho real da aplicação (`examples/perf_bench.rs`).
- Modo de diagnóstico `MOMENTO_PERF=1`: tempo de lançamento, abertura do projeto, FPS do
  scroll e tempo da preview no visualizador, no log.

### Melhorado
- A fotografia que está a ser vista passa à frente do trabalho em segundo plano
  (miniaturas, análise de IA, edição automática, exportação).
- Navegação rápida com as setas: a fotografia em que se para aparece primeiro (−33 %).
- Miniaturas servidas 38× mais depressa (pool de ligações ao catálogo); grelha pronta ao
  abrir um projeto de 15 000 em 242 ms (antes 794 ms).
- Análise de IA 1,85× mais rápida por fotografia.
- Exportação +60 % (19 → 30 fotografias de 24 MP por minuto).
- Índices SQLite em falta (cópias virtuais, apagar fotografias).
- Caches em memória: LRU verdadeira no editor; no máximo um resultado de AI Denoise em
  resolução total.


## [0.11.0] — 2026-11-10 — Fase 11: álbuns

### Adicionado
- **Álbuns fotográficos** por projeto (vista "Álbum"): formatos 20×20, 25×25, 30×30,
  35×35, 20×30, 25×35, 30×40 cm e personalizado; largura, altura, orientação, páginas,
  DPI, sangria, zona segura, dobra/centro, espaço de cor/ICC, laboratório e exportação
  por spread ou por página.
- **Presets de laboratório** personalizados (guardados na biblioteca, para todos os
  projetos). As medidas ficam **por confirmar** até haver preset ou confirmação: não se
  assumem sangrias nem margens universais.
- **Editor:** arrastar e largar da tira de fotografias, layouts de 1 a 6+ fotografias,
  full bleed, panorâmicas, alinhar, distribuir, margens, espaçamento, mover,
  redimensionar, recortar e zoom; guias de sangria, corte, zona segura e dobra; teclado.
- **"Criar álbum automaticamente":** usa a seleção final, momentos, cronologia,
  classificação, orientação, pessoas e relevância; narrativa abertura → preparação →
  cerimónia → casal → família → receção → festa → encerramento; evita repetições,
  páginas cheias, rostos na dobra, maus recortes e baixa resolução.
- **Verificações:** DPI efetivo por fotografia, zona segura, rostos na dobra, bordas
  perto do corte, molduras vazias, repetidas.
- **Exportação** pela fila, a partir dos originais: JPEG por spread/página, TIFF, PDF de
  impressão (sangria, `TrimBox`/`BleedBox`, perfil ICC) e PDF de prova (baixa resolução,
  guias, "PROVA").


## [0.10.0] — 2026-11-10 — Fase 10: galerias

### Adicionado
- **Galeria do cliente** por projeto: pasta HTML estática pronta a publicar (abre também
  diretamente do disco), sem serviços externos.
- Nomes dos noivos, data, local, texto introdutório, imagem hero, capa (pré-visualização
  ao partilhar) e logótipo; idioma português ou inglês.
- Capítulos automáticos pelos momentos: Preparação, Cerimónia, Sessão, Família e amigos,
  Cocktail, Receção, Festa (e momentos personalizados).
- Temas **Light, Dark, Minimal e Editorial**; mosaico e grelha; visualizador com setas,
  gestos, teclado e ecrã inteiro; apresentação automática; telemóvel, tablet e desktop.
- Favoritas (coração; lista copiada ou enviada por email; importação na aplicação com
  etiqueta roxa), downloads individuais e ZIP (completo e por capítulo), marca de água,
  **PIN/palavra-passe** (índice cifrado com AES-GCM) e ocultar fotografias.
- Imagens geradas pela fila de exportação a partir dos originais.
- Arquitetura de publicação (S3, R2, B2, SFTP) sem integração nesta fase.

## [0.9.0] — 2026-11-09 — Fase 9: exportação

### Adicionado
- **Motor de exportação** a partir do original (RAW em resolução total) + receita, nunca de
  previews: JPEG, JPEG XL (sem perdas), JPEG 2000, PNG, TIFF, BigTIFF, WebP, AVIF, OpenEXR,
  PSD (achatado) e PDF; 8, 10, 12, 16 bits e 16/32 bits de vírgula flutuante conforme o
  formato.
- **Cor**: sRGB, Display P3, Adobe RGB, ProPhoto RGB, Rec. 709, Rec. 2020 e perfil ICC
  personalizado / do laboratório (LittleCMS, intento percetual ou relativo), sempre com o
  perfil embutido (ou sinalização nativa em JPEG XL, AVIF e EXR).
- **Receitas**: qualidade, compressão, dimensões (lado longo/curto, largura, altura, caber,
  preencher com recorte, megapíxeis; px/cm/in), PPI, nitidez de saída (ecrã, mate,
  brilhante), metadados (tudo/copyright/nada, remover GPS), copyright, autor, contacto,
  marca de água (texto ou imagem, 9 posições), nome (campos, prefixo, sufixo, sequência),
  subpastas e conflitos (nunca sobrescreve originais).
- 10 receitas incluídas (Alta Resolução, Galeria, Website, Email, Instagram 4:5, Instagram
  Square, Impressão, Album, Arquivo TIFF; Arquivo DNG indisponível) e o plano **"Exportar
  casamento"** (várias receitas numa só operação, cada uma na sua pasta); receitas e planos
  do utilizador guardados na biblioteca.
- **Fila de exportação**: progresso, fotografias restantes, tamanho estimado (corrigido
  com os ficheiros reais), espaço livre, tempo restante, erros, pausa/retoma/cancelar,
  pausa automática sem espaço, "Mostrar pasta". Atalho Mod + Shift + E.

### Não incluído
- HEIF/HEIC (sem codificador HEVC livre; ImageIO no macOS por testar) e DNG de arquivo.

## [0.8.0] — 2026-11-09 — Fase 8: edição localizada, retoque e retrato

### Adicionado
- **Máscaras** (não destrutivas): pincel (tamanho, suavidade, fluxo, apagar), gradiente
  linear e radial, intervalo de luminância e de cor, e máscaras de IA local: sujeito,
  pessoa, rosto, pele, cabelo, roupa, fundo, objeto (por clique); céu por heurística.
  Somar, subtrair, intersetar, inverter (parte e máscara), densidade; ver a máscara a
  vermelho (tecla O).
- **Ajustes locais**: exposição, contraste, realces, sombras, brancos, pretos, temperatura,
  matiz, saturação, textura, clareza, nitidez e ruído.
- **Retoque**: corrigir (heal) e clonar por clique com origem automática, remoção simples
  de objetos com pincel; pó do sensor e olhos vermelhos (Fase 6) no mesmo painel.
- **Retrato** (subtil, por rosto): suavizar pele, olheiras, brilho, dentes, olhos.
- Modelos `segment_multiclass` e `segment_object` (MediaPipe, Apache-2.0).

## [0.7.0] — 2026-11-09 — Fase 7: presets e consistência visual

### Adicionado
- **Presets**: criar a partir da fotografia (escolhendo os componentes), renomear, organizar
  em grupos e ordem, duplicar, eliminar, importar e exportar (`.momentopreset` com LUT);
  pré-visualização ao passar o rato; aplicar a uma fotografia, à seleção, a um momento ou
  ao casamento completo, com **Amount** 0–150 %.
- 13 presets internos: Natural Wedding, Clean Editorial, Fine Art, Romantic Soft, Film Warm,
  Film Clean, Golden Hour, Ceremony Indoor, Reception Night, Flash Clean, B&W Classic,
  B&W Documentary, B&W Fine Art.
- Importação **aproximada** de XMP do Lightroom/Camera Raw com relatório; LUT `.cube`
  (sRGB, 1D/3D); documento `docs/COMPATIBILIDADE-PRESETS.md`.
- Motor: tonalização de sombras/realces, preto e branco com mistura por cor, LUT.
- **Consistência**: vista nova com grupos por luz semelhante, iluminação, câmara, cenário,
  horário, balanço de brancos e flash/natural; fotografia de referência e **sincronização
  inteligente** (exposição, balanço, pele, contraste, cor ajustados medindo o resultado), em
  segundo plano; também no "Sincronizar" do editor.
- Catálogo v7 (`look_feature`), biblioteca v2 (`preset`), EXIF `Flash`.

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
