# Testes

## Estratégia

| Nível | Ferramenta | Onde | Estado (Fase 1) |
|---|---|---|---|
| Unitários Rust | `cargo test` | `#[cfg(test)]` em cada crate | ✅ 80 testes |
| Integração do núcleo | `cargo test` com projetos temporários | `packages/project-management/tests/full_flow.rs` | ✅ 4 testes (até 625 ficheiros) |
| **Ficheiros reais (Fase 2)** | `MOMENTO_SAMPLES_DIR` | `raw-engine/tests/real_samples.rs`, `project-management/tests/real_formats.rs` | ✅ 26 RAW reais + 65 ficheiros de 25 formatos (relatórios em `docs/reports/`) |
| Unitários TS + componentes | Vitest + Testing Library (jsdom, núcleo simulado) | `apps/desktop/src/**/*.test.ts(x)`, `packages/localization` | ✅ 36 testes |
| Aplicação real (manual automatizada) | binário release + Xvfb + xdotool | ver “Verificação na aplicação real” | ✅ executada (Linux) |
| Compilação para Windows | `cargo check --target x86_64-pc-windows-gnu` + executável release | todo o workspace, incluindo a LibRaw em C++ | ✅ (executável de 19,9 MB ligado) |
| Compilação para macOS | `cargo check -p momento-image-codecs --target aarch64-apple-darwin` | adaptador ImageIO | ✅ (o resto, com C/C++, é compilado no CI macOS) |
| CI | GitHub Actions: macOS 14, Windows 2022, Ubuntu 24.04 | `.github/workflows/ci.yml` | ⏳ configurado, ainda não executado (repositório sem remoto) |
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

## Fase 11 — álbuns

| Teste | O que verifica |
|---|---|
| `album::spec`, `album::layouts` (3) | formatos e orientação, problemas da especificação, preset de laboratório; layouts de 1 a 12 fotografias dentro do spread, sangria só nos lados exteriores, margem interior ≥ metade da dobra, linhas justificadas com espaçamento fixo |
| `album::tests` (5) | **narrativa automática** (abertura, preparação → cerimónia → casal → receção → festa; sem duplicados nem rajadas; baixa resolução fora; máximo por página); **sem rostos na dobra** e recorte centrado nos rostos; verificações (DPI 50 %/80 %, rosto na dobra/fora da zona segura, borda perto do corte, vazio, repetida, por confirmar); composição (sangria, posição em píxeis, guias da prova) e exportação JPEG/TIFF/PDF (páginas, `TrimBox`/`BleedBox`, ICC, prova a 120 DPI); **fotografias pequenas nunca ampliadas** além do DPI do laboratório (regressão do teste real) |
| `album_flow` | caminho real: momentos → criar 20×20 → automático com 12 fotografias pela ordem da narrativa, sem erros → mudar o layout mantém as fotografias → JPEG por spread com a largura exata (sangria incluída) e PDF de impressão pela fila → presets de laboratório → originais intactos |
| Interface (+3) e lógica (+4) | alertas de DPI e por confirmar, exportar bloqueado com erros, aplicar layout, **arrastar da tira para a moldura** (recorte no rosto), teclado, automático com confirmação e narrativa, preset de laboratório, formato 30×40 e personalizado, páginas pares, exportar prova; atalhos da grelha desligados no álbum; "Abrir PDF" na fila; recorte/DPI iguais ao núcleo, full bleed, panorâmica, limites, alinhar, distribuir |
| Aplicação real (Xvfb) + `pypdf` | editor com um casamento de 55 fotografias, automático, arrastar, exportação do PDF de impressão e de prova validados por um leitor PDF independente (páginas, `TrimBox`, `BleedBox`) |

## Fase 10 — galerias

| Teste | O que verifica |
|---|---|
| `gallery::chapters` | 25 momentos → capítulos de casamento por ordem (Preparação, Cerimónia, Sessão, Cocktail, Festa, personalizado, "Mais momentos"); fotografia sem momento vai para o capítulo mais próximo no tempo; inglês; um por momento; sem capítulos |
| `gallery::crypto` | cifra/decifra AES-GCM + PBKDF2; código errado recusado; sal novo a cada geração |
| `gallery::tests` (3) | site completo com JPEG reais: ficheiros, fontes e licenças, título **escapado** (sem injeção), dados sem `</script>`, nomes de download únicos, ZIP completo (pastas por capítulo, alta resolução) + ZIP por capítulo; **protegida**: nada visível sem o código (nomes, local, endereços, capítulos, og:image), `noindex`, logótipo com nome aleatório; downloads desligados/ecrã |
| `gallery::publish` | plano de envio (sem ficheiros ocultos, MIME, cache, SHA-256), envio incremental (só o que mudou, apaga o que saiu), validação de S3/R2/B2/SFTP sem segredos, nenhum serviço contactado |
| `gallery_flow` | caminho real: definições iniciais do projeto, oculta e rejeitada fora, capítulos pelos momentos, geração a partir dos originais (não amplia; download em resolução total), ZIP, copyright sem GPS; voltar a gerar pede confirmação; PIN; **pasta que não é do Momento nunca é tocada**; favoritas do cliente → etiqueta roxa; originais intactos |
| `scripts/test-gallery.cjs` (Chromium/Playwright) | por `file://` e HTTP: fotografias e capítulos; mosaico sem sobreposições (4 colunas no desktop, 3 no tablet, 2 no telemóvel, sem deslocamento horizontal); grelha quadrada e persistente; favoritas (contagem, persistência, filtro, lista, email, ZIP no navegador só por HTTP); visualizador (setas, Esc, endereço direto); apresentação avança e pausa; downloads individuais e ZIP existem; PIN errado/certo e lembrado no separador; sem erros de JavaScript — galerias de demonstração e a gerada pela aplicação real |
| Interface (+2) | diálogo (nomes, capítulos, ocultar seleção, tema, hero da fotografia em foco, PIN só com dígitos, fornecedores como fase futura sem botões, favoritas do cliente, confirmação para substituir); fila com "Abrir galeria" |

## Fase 9 — exportação

| Teste | O que verifica |
|---|---|
| `color-management` (4) | conversão por matriz = LittleCMS com o **perfil embutido** nos 6 espaços (gama e linear); branco e preto preservados; curvas ida e volta; ICC "do laboratório" pela LittleCMS igual ao interno; perfis não RGB recusados |
| `develop::tests` (+1) | `render_linear` (exportação) dá os mesmos bytes que o ecrã (±1) com exposição, contraste, saturação, vinheta e grão |
| `export-engine::quality_tests` (13) | cada formato lido por um **descodificador independente**: JPEG (PSNR > 38 dB a q92, ICC, EXIF lido pelo kamadak-exif, GPS removido, XMP), PNG 8/16 exatos (+ICC P3, PPI), TIFF e BigTIFF 8/16/32f × sem/LZW/ZIP exatos com ICC, WebP sem perdas exato e com perdas, JPEG XL sem perdas em 5 espaços (codificação de cor lida pelo jxl-oxide), JPEG 2000 (8 bits sem perdas exato; 12/16 bits com perdas), OpenEXR half/float com cromaticidades, PSD 8/16 sem/RLE, PDF (tabela xref válida, página pelo PPI), AVIF (caixas `colr` nclx e `pixi`); **gama larga**: P3 → ler com o perfil embutido → cores originais (fora de sRGB); dimensões/nitidez/marca de água; estimativa; HEIF/HEIC/DNG recusados com razão |
| `export-engine` (8) | dimensões (lado longo/curto, caber, preencher 4:5, não ampliar, cm a 300 PPI, megapíxeis); Lanczos mantém cores lisas e a média; nitidez só nos contornos; marca de água no canto pedido; EXIF lido por leitor independente (GPS conforme a opção, modos); XMP escapado; nomes (campos, carateres proibidos, nomes reservados do Windows, subpastas sem `..`); conflitos (renomear, ignorar, substituir — **nunca um original**); 10 receitas e plano "Exportar casamento" |
| `export_store` + `database` (+1) | receitas e planos do utilizador na biblioteca; incluídas protegidas |
| `export_flow` | caminho real: plano "Exportar casamento" → 15 ficheiros em 5 pastas numa operação; estimativa vs. real; resolução total; Instagram 1080×1350; TIFF 16 bits; edição (+1 EV) exportada; GPS removido; segunda exportação renomeia; pausa (não avança) e retoma; cancelar; receita inválida → erro claro; **originais intactos**. RAW Canon 7D real: TIFF 16 bits **5184×3456 a partir do sensor** (não da preview), > 2000 níveis distintos, estimativa a 14 % do real |
| `scripts/validate-exports.py` | 25 amostras de uma fotografia real lidas por **libjxl, libavif, OpenJPEG (C), libwebp, Pillow, tifffile, psd-tools, pypdf e OpenEXR**: dimensões, profundidade, perfil ICC certo, copyright presente, GPS ausente, PSNR contra a referência (sem perdas ≥ 107 dB; JPEG q92 45,6 dB; AVIF 43,7–46,5 dB; EXR 32 bits 121,8 dB) — 25/25 válidos. Encontrou e levou a corrigir: texto do EXR em Latin-1 e JPEG 2000 sem perdas a 12/16 bits (agora limitado a 8 bits) |
| Interface (+3) | diálogo com plano de várias receitas, HEIC desativado, AVIF limita profundidades e espaços, exemplo de nome, marca de água por posição, destino e estimativa, pedido enviado; fila (restantes, tamanho, espaço, tempo, erros, pausa por falta de espaço, retomar, mostrar pasta); lógica de nomes igual ao núcleo |

## Fase 8 — edição localizada, retoque e retrato

| Teste | O que verifica |
|---|---|
| `develop::local` (2) | gradiente linear/radial, somar/subtrair/intersetar, inverter (parte e máscara), densidade, intervalos de luminância e cor (cinzento não conta); pincel: borda suave, fluxo 50 % → 0,5, duas passagens → 0,75, apagar; polígonos |
| `develop::local_tests` (4) | exposição/temperatura/saturação/tons locais só dentro da máscara; textura+nitidez aumentam e ruído/clareza negativa reduzem o detalhe só na máscara; a máscara acompanha o recorte; sobreposição vermelha; máscara desligada e imagem em falta sem efeito; **heal vs clone** (clone traz o escuro, heal iguala o fundo); remoção de objeto; retrato: manchas atenuadas, poros mantidos, olheiras mais claras sem ultrapassar a pele, dentes menos amarelos, **fora do rosto e contorno iguais**, quantidades a 0 = original |
| `develop::sky`, `geometry` (+1), filtros | céu azul com torre (torre e relva fora), interior sem céu; vista ↔ fotografia inversas com recorte, rotação, espelho, perspetiva e distorção |
| `face-analysis::segment` | retrato real: rosto, cabelo, roupa e fundo nos sítios certos; objeto por clique = a pessoa |
| `local_edit_flow` | caminho real: céu guardado em `masks/` e escurecido (relva igual), mapeamento de pontos, origem do heal no céu, repor = original; retrato real: 8 máscaras de IA com coberturas coerentes (pessoa + fundo = 1, sujeito = pessoa), exposição só na pessoa, 1 rosto detetado, retrato só no rosto e removível; **originais intactos** |
| Interface (+3) | máscara de IA, sobreposição no render, ajuste local, gradiente a subtrair desenhado, inverter, eliminar; pincel (fluxo, apagar, Esc); corrigir/clonar/remover mancha, remover objeto; retrato (detetar, ajustar, remover) |

## Fase 7 — presets e consistência

| Teste | O que verifica |
|---|---|
| `presets` (5) | Amount 0/50/100/150 % só nos componentes escolhidos (curvas misturadas, P&B a partir de 50 %, LUT escalada); capturar componentes; 13 internos com os nomes pedidos, todos diferentes, a manter exposição e balanço, P&B só nos P&B; um preset a cores depois de um P&B volta às cores; ficheiro com LUT ida e volta; XMP real: campos interpretados e ignorados (Kelvin, perfil, máscaras, meios-tons) |
| `develop::lut` (2) + render (+1) | LUT identidade e ordem dos canais, 1D, recusa de LUT log e de ficheiros incompletos; inverter cores a 100 % e 50 %; P&B com mistura (vermelho clareia, azul igual); tonalização (sombras azuladas, realces quentes, brilho igual) |
| `develop::consistency` (2) | fotografia 1,3 EV mais escura com luz de tungsténio: rosto, branco do vestido, pele, contraste e saturação iguais à referência, valores diferentes (copiar não chegava); opções limitam o que muda; sem rostos usa a cena |
| `consistency` (1) + `database` (+2) | agrupamento pelos 7 critérios (exceção com flash absorvida, intervalos de 20 min); EV da cena; presets na biblioteca |
| `preset_store` (1) | criar, renomear, duplicar (incl. internos), organizar, eliminar (internos protegidos), importar LUT, exportar/importar com LUT noutra biblioteca, recusa de LUT log e `.lrtemplate` |
| `presets_consistency_flow` | caminho real: medir 6 fotografias, 7 critérios, iluminação e balanço corretos (exterior vs. interior quente), Film Warm a 80 % no casamento completo, preset com LUT (copiada para o projeto), sincronização em fila: brilho e branco iguais à referência, não é cópia, **originais intactos** |
| Interface (+4) | pré-visualização ao passar o rato com Amount, aplicar (histórico "Preset: …"), criar com componentes; gerir (renomear, duplicar, ordenar, eliminar com confirmação, exportar); aplicar à seleção e ao casamento; vista Consistência, referência, sincronizar, medir; sincronizar inteligente vs. copiar valores |

## Fase 6 — edição automática

| Teste | O que verifica |
|---|---|
| `image-processing::develop::auto` (11) | cada fotografia recebe ajustes próprios (escura clareia, clara escurece/recupera, dominante azul aquece, quente arrefece menos); escura e normal convergem (diferença < 50 % da inicial); intensidades ordenadas e "Personalizada" só mexe nos grupos pedidos; ruído medido (limpa ≈ 0, σ ≈ 9/255 detetado), só a ruidosa recebe redução forte, ISO 100 limita, AI Denoise só quando pedido; tabela de sensores; horizonte inclinado 3° corrigido e direito não mexe; aberração cromática estimada (≈ −40) e ausente numa imagem limpa; olhos vermelhos detetados e corrigidos (pupila escura e neutra, pele intacta) e pupilas escuras ignoradas; 3 manchas de pó encontradas num céu em degradê, removidas, e nenhuma numa imagem sem pó; recorte automático mantém a proporção e o rosto; HSL, grão, 90° e curva de pontos do utilizador mantêm-se; bloco `retouch` na receita |
| `face-analysis::denoise` | FFDNet no tract: ruído gaussiano σ = 25/255 → PSNR 20,2 → 39,8 dB; cancelar devolve `false` sem alterar a imagem |
| `auto_edit_flow` | caminho real: pré-visualização não grava; "auto" no histórico e desfazer volta ao original; fila de 8 em pausa não avança, acrescentar à fila, opções diferentes recusadas, retomar → 8 editadas com ajustes diferentes; cancelar não toca nas restantes; AI Denoise no editor não bloqueia o render e avisa quando acaba; RAW real (`MOMENTO_SAMPLES_DIR`); **originais com o mesmo hash** |
| Fotografias reais (`examples/auto_eval`) | 10 fotografias (JPEG e RAW Canon 7D, Fuji X-E1, Nikon D810): comparações antes/depois; **0 olhos vermelhos falsos em 33 rostos** e 2/2 olhos vermelhos simulados detetados e corrigidos |
| Interface (+3) | intensidades e "Personalizada"; Auto (botão e Ctrl+U) com relatório; aplicar AI Denoise recomendado; retoque só de olhos vermelhos; vista dividida (Y); diálogo "Editar automaticamente seleção", fila com pausa/retoma/cancelar/fechar |

Avaliar a edição automática com fotografias próprias (nada é escrito nos originais):

```bash
cargo run --release -p momento-project-management --example auto_eval -- /pasta/fotos /pasta/saida natural
```

## Fase 5 — motor de edição

| Teste | O que verifica |
|---|---|
| `image-processing::develop` (27) | receita neutra = original (±1 nível); +1 EV duplica a luz linear; balanço mais quente e conta-gotas neutraliza um cinzento com dominante; sombras/realces mexem só na sua zona; contraste, brancos/pretos; saturação −100 = cinzento; vibração; HSL por cor (o vermelho não muda ao mexer no azul); curvas (invertida, por canal, paramétrica); recorte, 90°, espelho, endireitar sem cantos vazios, perspetiva; endireitar automático corrige um horizonte inclinado 4°; objetiva (vinheta, distorção, aberração só no vermelho); nitidez aumenta e redução de ruído diminui o detalhe fino, máscara protege zonas lisas, ruído de cor; clareza/textura/dehaze/vinheta/grão (determinístico); histograma; rápido ≈ final; receita só com blocos alterados; desempenho |
| `database::edits` | histórico, desfazer até ao original, refazer, ramificação (o futuro é descartado), instantâneos, cópias virtuais com classificação independente, apagar só cópias |
| `edit_flow` | caminho real com JPEG e RAW real (Nikon D810): render rápido/final, gravar/desfazer/refazer, instantâneo, repor, colar só a luz, cópia virtual a P&B, miniatura/preview editadas, sidecars, resolução total sem ampliar, **originais com o mesmo hash**, reabrir |
| Interface (+5) | arrastar a exposição = renders rápidos sem gravar, largar = gravação + render final; atalhos com o foco num deslizador; desfazer; antes/depois; conta-gotas; endireitar automático; copiar sem geometria; colar; sincronizar; instantâneos; cópia virtual; recorte e proporção |

```bash
MOMENTO_SAMPLES_DIR=/pasta/com/um/raw cargo test -p momento-project-management --release --test edit_flow -- --nocapture
```

## Fase 4 — organização do casamento

| Teste | O que verifica |
|---|---|
| `face-analysis` (runtimes oficiais) | SFace no tract = onnxruntime (cos 1,000000); EfficientNet-Lite2 no tract = TFLite (diferença máx. 5,5e-7); mesma pessoa 0,96–0,98 vs diferentes < 0,08 (`MOMENTO_REF_DIR`, `MOMENTO_FACES_DIR`) |
| `ai-analysis::people` (4) | grupos puros, convidados isolados não viram pessoas, pessoas confirmadas atraem rostos, excluídos ignorados, dois rostos da mesma fotografia nunca juntos, 30 000 rostos em ≈ 7–9 s |
| `ai-analysis::moments` (5) | dia de casamento sintético (25 momentos) todo correto; sem pistas nada é forçado; aprendizagem com exemplos (incl. momento personalizado); GPS separa cenas; 15 000 fotografias em ≈ 0,2 s |
| `ai-analysis::clock` (2) | Canon certa, Sony +3min42s, DJI −1min15s recuperados; sem correspondências não há sugestão |
| `database` (+4) | GPS, relógios (ordem corrigida, EXIF intacto), momentos (utilizador prevalece, personalizados), pessoas (grupos, nomes, juntar, excluir, sobrevivência à reanálise) |
| `organize_flow` | fotografias reais: 2 pessoas agrupadas, identificar noiva/noivo, noivos juntos, sugestão Sony ≈ −3min42s, cronologia reconstruída, momento personalizado aprendido, reabrir |
| Interface (+7) | pesquisas por pessoas, filtro por momento, arrastar para momento, criar momento, gerir pessoas, sincronizar câmaras, desvios de relógio, recorte de rosto |

**Limitação:** a precisão dos momentos foi medida num dia **sintético** (pistas fabricadas
como as dos modelos) e numa aplicação real sem fotografias de casamento. Falta medir com um
casamento real.

## Fase 3 — seleção assistida

| Teste | O que verifica |
|---|---|
| `face-analysis` (referência Google) | deteção, 478 pontos e blendshapes de `portrait.jpg` iguais aos resultados publicados pelo MediaPipe (`MOMENTO_MP_PORTRAIT`) |
| `ai-analysis::metrics` (7) | nitidez, tremido, exposição/clipping, ruído, inclinação em imagens sintéticas controladas |
| `ai-analysis::selection` (8) | rajada do beijo (14), duas câmaras, RAW+JPEG e duplicados, modos, objetivo, piscadela relativa, riso com olhos semicerrados, **15 000 fotografias em ≈ 32 ms** |
| `database::analysis` | gravação/leitura, `u64` dos hashes, grupos, aceitar sem tocar em decisões do utilizador, reanálise |
| `analysis_flow` | importar → analisar com os modelos → rajada agrupada e a mais nítida é MELHOR → modo rigoroso → aceitar grupo → objetivo → reabrir. Com `MOMENTO_FACES_DIR`, usa uma fotografia real com rosto |
| `analysis_throughput_24mp` (ignorado por omissão) | débito com JPEG de 24 MP |
| Interface (6 novos) | análise só a pedido, etiquetas e “1/3” na grelha, filtro por etiqueta, modo/objetivo, sequências + aceitar + comparar, comparação por teclado e “selecionar outra”, explicação e pontuações no painel, frases, matemática do zoom sincronizado |

```bash
MOMENTO_FACES_DIR=/pasta/com/fotos cargo test -p momento-project-management --release --test analysis_flow -- --nocapture
MOMENTO_PERF_N=300 cargo test -p momento-project-management --release --test analysis_flow -- --ignored --nocapture
```

**Limitação conhecida:** não houve acesso a fotografias reais com olhos fechados; a
deteção de olhos fechados está validada indiretamente (os coeficientes coincidem com a
referência da Google para olhos abertos) e por testes do motor com valores sintéticos.

## Fase 2 — ficheiros reais

Fontes (públicas, nunca no Git): 40 ficheiros do repositório `metadata-extractor-images`
(RAW de 15 fabricantes, HEIC/HEIF/AVIF, WebP, PSD, JP2, incluindo ficheiros malformados
que fizeram falhar outras bibliotecas) + 3 do `rawpy` (NASA, domínio público) + 21
produzidos por codificadores independentes (ImageMagick/libwebp/OpenJPEG/libtiff, libjxl,
libavif, OpenEXR 3.5).

Resultados (detalhe em `docs/reports/`):
- 25/26 RAW com metadados, preview embutida, ½ e revelação total (o 26.º é um RW2 truncado
  de 368 bytes → “corrompido”);
- 65 ficheiros pelo caminho da aplicação: 51 importados (46 completos + 5 parciais com
  explicação), 3 duplicados, 9 “codec em falta” (HEIC/AVIF/PSB no Linux), 2 corrompidos,
  0 pendentes, nenhuma falha da aplicação;
- aplicação real (release + Xvfb): relatório com os 4 estados, grelha RAW orientada,
  painel com bits/cor/descodificador, revelação 100 % de um NEF de 36 MP, selos “Parcial”.

Defeitos encontrados e corrigidos: previews embutidas já rodadas pela câmara (Nikon D70,
Kodak DCS) rodadas duas vezes; X3F desativado por omissão na LibRaw; o crate `psd` entrava
em pânico com PSD válidos (substituído por leitor próprio); compilação da LibRaw no
Windows (`M_PI`); contagem de câmaras documentada sem verificação (corrigida por teste).

Não validado: HEIC/HEIF/AVIF em macOS/Windows (adaptadores compilados, sem execução);
formatos RAW sem amostra (NRW, SRF/SR2, 3FR/FFF, IIQ, KDC, MRW, MEF, ProRAW, lossy DNG).

## Verificação na aplicação real — Fase 1 (2026-11-08)

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
