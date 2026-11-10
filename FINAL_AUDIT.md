# Auditoria final — PFF ProMaster 1.0.0-personal

**Data:** 2026-11-10 · **Âmbito:** implementação real comparada com **toda** a especificação
mestre (§1–§30) e com os pedidos das Fases 0 a 16. A versão 1.0 pessoal só se considera
concluída depois desta auditoria (pedido do utilizador).

## 1. Método

- Cinco auditorias por área, **só por leitura do código** (A–E, abaixo). Cada requisito foi
  seguido de ponta a ponta: botão → `api.ts` → comando Tauri → núcleo Rust → resultado. Um
  botão ou componente visual **não** conta como funcionalidade completa.
- Estados: **COMPLETA** (processamento real de ponta a ponta), **PARCIAL** (incompleta, ou
  só no núcleo sem interface), **NÃO IMPLEMENTADA**, **MOCK** (a interface simula),
  mais **BUG CONHECIDO** e **TESTADA** (nome do teste) / **NÃO TESTADA**.
- Depois da auditoria: correção dos problemas críticos e altos, testes novos para cada
  correção, documentação alinhada com o comportamento real. A matriz da secção 7 já reflete
  as correções (marcadas "Corrigido nesta auditoria").

## 2. Pesquisa de marcadores no código

| Pesquisa | Resultado no código do projeto |
|---|---|
| `TODO`, `FIXME`, `XXX`, `HACK` | **0** (as ocorrências estão só na LibRaw incluída, código de terceiros) |
| `unimplemented!`, `todo!()` | **0** |
| `mock`, `fake`, `dummy`, `stub` | Só em testes (`components.test.tsx` simula o núcleo Tauri; `Fake` codec em `image-codecs/src/registry.rs` testes) e na LibRaw |
| `placeholder` | Só o atributo HTML `placeholder` de campos de texto (texto de exemplo), não funcionalidades simuladas |
| `hardcoded`, "temporary implementation" | **0** no código do projeto |
| Handlers vazios / botões sem ação | **0** (`onClick={() => {}}` inexistente; o único `disabled` fixo é a opção-título de um `<select>`) |
| `console.log/error/warn` na interface | **0** fora dos testes |
| Erros de consola / exceções não tratadas | **Antes:** não eram capturados (janela em branco, sem registo). **Agora:** ErrorBoundary + `window.onerror`/`unhandledrejection` → registo local anonimizado |
| Interface simulada encontrada | (1) **Realces/Sombras locais** gravados mas sem efeito no render (**MOCK de facto**) — corrigido; (2) lista de fornecedores de publicação com "Fase futura" — removida e substituída por texto honesto; (3) código morto "Brevemente" na barra lateral — removido |

## 3. Problemas críticos e altos — corrigidos

| # | Problema | Gravidade | Correção | Teste |
|---|---|---|---|---|
| D1 | Realces e Sombras **locais** (máscaras) não faziam nada; o teste escondia-o | Crítico (simulado) | Aplicados em luz linear com pesos por zona de exposição, dentro da máscara | `local_adjustments_stay_inside_the_mask` (realces e sombras separados) |
| B1/D2 | Perfil ICC embutido **ignorado** na edição e exportação de JPEG/TIFF/PNG/WebP (Adobe RGB/P3/ProPhoto tratados como sRGB → cores erradas na entrega) | Crítico | `InputColor` (LittleCMS) converte o perfil de entrada para Rec. 2020 linear | `embedded_icc_profile_is_honoured`, `embedded_input_profiles_round_trip` |
| A-P3 | Restaurar/importar um catálogo deixava a cache antiga (indexada por id) → miniaturas de **outras** fotografias | Alto | Cache limpa ao substituir o catálogo e ao restaurar automaticamente; miniaturas regeneradas ao abrir | `privacy_and_cleanup_actions` |
| C1/P4 | PRIVACY.md prometia "Eliminar projeto", "Limpar cache" e "apagar dados faciais" — nada disso existia na interface | Alto (promessa falsa) | Separador **Privacidade** em Segurança e cópias: Limpar cache, Apagar dados de pessoas, Eliminar projeto (para o Lixo, recusa se houver originais dentro, nunca toca nos originais) | `privacy_and_cleanup_actions`, `clear_all_only_touches_the_cache` |
| A-P1 | Relatório de falha procurava `momento.log`, mas os registos passaram a `momento.<data>.log` → relatório sem o fim do registo | Alto | Padrão novo (e antigo aceite) | `reports_are_written_without_personal_paths_and_rotated` |
| A-P2 | Registos normais com caminhos de projeto (nomes dos noivos) | Médio (privacidade) | Cada linha passa por `redact` antes de chegar ao disco | `log_lines_never_carry_paths_or_emails` |
| E2 | Regenerar a galeria mudava o id e os tokens → **favoritas do cliente perdidas** | Alto | Id e tokens guardados no catálogo por pasta; reutilizados | `wedding_gallery_end_to_end` |
| E3 | A galeria anterior era **apagada antes** de gerar a nova (falha a meio = cliente sem galeria) | Alto | Gerada numa pasta ao lado e trocada só no fim | `wedding_gallery_end_to_end` (sem pastas temporárias) |
| E4 | Álbum: rosto cortado pelo enquadramento do slot sem aviso | Alto | Aviso "Rosto cortado pelo enquadramento" (>15 % fora) | `preflight_finds_the_problems` |
| C2 | Uma favorita podia aparecer com "Rejeitar?" sugerido | Médio | Etiqueta "Protegida" para favoritas/estrelas | Vitest "uma favorita ou com estrelas aparece como protegida" |
| C7 | Pesquisa "Noivos"/"Noiva + mãe" usava só a 1.ª pessoa de cada papel (fotografias escondidas) | Médio | Com 2+ pessoas no mesmo papel, pede para as juntar | Vitest "papéis com mais do que uma pessoa" |
| D5 | Cópias virtuais com nomes repetidos depois de apagar uma | Baixo | Nome = maior + 1 | `history…virtual_copies` |
| D6 | Edições em cache sem versão do motor (imagens antigas depois de mudar o algoritmo) | Médio | `PIPELINE_VERSION` na chave | (coberto pelos testes de edição) |
| P5 | "Mover para o Lixo" ignorava cópias virtuais selecionadas | Baixo | A cópia sai do catálogo; o original nunca vai para o Lixo por causa de uma cópia | `privacy_and_cleanup_actions` |
| — | Erros JavaScript sem captura | Médio | ErrorBoundary + registo | Vitest "uma falha num componente mostra a mensagem" |

## 4. Documentação corrigida (documento ↔ comportamento real)

| Documento | Dizia | Realidade (agora escrito) |
|---|---|---|
| SECURITY.md, ARCHITECTURE §6.4, ROADMAP F2 | LibRaw num processo auxiliar | Mesmo processo; auxiliar não implementado |
| README, ARCHITECTURE (diagrama) | ONNX Runtime / Core ML, GPU wgpu | tract-onnx em CPU; sem GPU nem Neural Engine |
| ARCHITECTURE §7 | Soft proofing, gamut, perfil do monitor "definidos" | Estado real: ICC de entrada e saída sim; o resto não implementado |
| ARCHITECTURE §12 | Sidecars XMP; versão do pipeline no hash | Sem XMP (só importação de presets); versão do pipeline agora real |
| ROADMAP F2 / F10 | IPTC/XMP lidos; publicação "arquitetura pronta" | Não lidos; publicação não implementada |
| PRIVACY.md | Download de modelos e publicação de galerias pela rede; eliminar projeto/cache/dados faciais | Nenhuma ligação de rede; as três ações existem agora e estão descritas com o que fica |
| RELEASE_NOTES | Exporta HEIF/HEIC e DNG; "15 fabricantes" | Não exportam; fabricantes em SUPPORTED_FORMATS; limitação de 16 bits |
| INSTALLATION | Versão "ao abrir" sempre | Só se a última tiver mais de 60 min |
| `.env.example` | Modelos "descarregados" | Incluídos na aplicação; nada é descarregado |
| `models/manifest.json` | Faltavam `scene_classifier` e `face_identity` | Os 9 modelos |
| Interface (galeria) | 4 fornecedores com "Fase futura" | Texto: publicação direta indisponível nesta versão |

## 5. Renomeação

Pedido do utilizador: a aplicação chama-se **PFF ProMaster**, com o ícone fornecido
(diafragma de 6 lâminas com "Photo" por cima, fonte vetorial em
`apps/desktop/src-tauri/icons/icon-source.svg`). Mudaram: nome do produto e da janela,
identificador (`pt.pffpromaster.desktop` — sem instalações anteriores em macOS), pedidos de
permissão do macOS, metadados "Software"/"Producer" nos ficheiros exportados, galerias,
documentação. Mantidos por compatibilidade (D-109): crates `momento-*`, extensão `.momento`
dos projetos, `.momentopreset`, marcador `.momento-gallery`.

## 5b. Verificação no pacote final

Pacote `PFF ProMaster_1.0.0_amd64.deb` gerado por `scripts/build-release.sh` a partir do
commit da auditoria, instalado com `dpkg -i` e executado (Linux, Xvfb):

| Passo | Resultado |
|---|---|
| Lançamento | ✅ "PFF ProMaster 1.0.0-personal · núcleo 1.0.0-personal · esquema 9"; janela "PFF ProMaster"; dados em `pt.pffpromaster.desktop` |
| Registo | ✅ `aplicação iniciada … dados=<caminho>` (caminho anonimizado) |
| Abrir um projeto real (cópia de um catálogo v7) | ✅ 55 fotografias, miniaturas |
| Segurança e cópias → Privacidade → Limpar cache | ✅ 61 ficheiros (4,8 MB) apagados; 55 miniaturas geradas de novo |
| Eliminar projeto… (com confirmação) | ✅ pasta no Lixo, retirado dos recentes, volta ao início; originais intactos |
| Nota | Num Linux onde o pacote de teste da Fase 16 (`momento-studio`) estava instalado, o `dpkg` recusa instalar por cima (ícone com o mesmo nome): desinstalar o antigo primeiro. Em macOS o `.app` muda de nome (`PFF ProMaster.app`); a pasta de dados nova começa vazia (não houve instalações anteriores em Mac). |

## 6. As três listas

### TOTALMENTE FUNCIONAL (processamento real, de ponta a ponta, com testes)

1. Projetos: criar, abrir, recentes, fechar; catálogo SQLite gravado a cada alteração; uso exclusivo; recuperação de sessões interrompidas (validação completa, restauro automático de uma versão boa).
2. Versões do catálogo (programadas, manual, antes de migrar), validar, exportar/importar catálogo, restaurar (com cache invalidada).
3. Importação de pastas/cartões montados: referência ou cópia verificada (BLAKE3) para a pasta dos originais e para um disco de segurança em simultâneo; duplicados pelo conteúdo; retoma; relatório de tudo o que não entrou; originais nunca alterados.
4. Originais: em falta/offline detetados, religar por ficheiro ou pasta, remover do catálogo, mover para o Lixo (com confirmação; cópias virtuais nunca levam o original).
5. Leitura de RAW (LibRaw, revelação 16 bits linear) dos fabricantes validados em `SUPPORTED_FORMATS.md`, JPEG, TIFF, PNG, WebP, JPEG XL, JPEG 2000, PSD/PSB composto; perfis ICC embutidos respeitados na grelha, no editor e na exportação.
6. Grelha virtualizada (15 000 fotografias), estrelas, favoritas, rejeitadas, cores, filtros, comparação, visualizador, menus contextuais, atalhos, temas escuro/claro.
7. Seleção assistida: nitidez, desfoque de movimento, exposição, ruído, olhos fechados, sorriso, inclinação, rosto cortado, sequências/rajadas, duplicados; sugestões explicadas que nunca alteram decisões do utilizador (favoritas mostradas como "Protegidas").
8. Pessoas: deteção de rostos, agrupamento por identidade facial local, nomes e papéis, juntar, "não é esta pessoa", pesquisas por papel (com aviso quando um papel tem duas pessoas).
9. Momentos do casamento por cena + hora, várias câmaras com sincronização de relógios, atribuição manual e propagação.
10. Revelação não destrutiva: luz, cor, HSL, curvas, detalhe, efeitos, geometria, máscaras (pincel, gradientes, sujeito/céu/pessoa por IA) com ajustes locais (incluindo realces/sombras), retoque, retrato, AI Denoise, histórico, instantâneos, cópias virtuais, copiar/colar/sincronizar.
11. Edição automática por fotografia ou em lote; presets (criar, aplicar, importar XMP aproximado, LUT .cube); consistência entre fotografias.
12. Exportação: JPEG, JPEG XL (sem perdas), JPEG 2000, PNG, TIFF/BigTIFF 8–32 bits, WebP, AVIF, OpenEXR, PDF, PSD; perfis sRGB/P3/Adobe RGB/ProPhoto/Rec.709/Rec.2020 e ICC de laboratório; redimensionar, nitidez de saída, marca de água, metadados com GPS removível; planos com várias receitas; estimativa de espaço; pausar/retomar/cancelar; retoma depois de fechar.
13. Galeria do cliente (site estático): capítulos por momento, temas, mosaico/grelha, visualizador, apresentação, favoritas (que sobrevivem a regenerar), downloads individuais/ZIP, marca de água, PIN/palavra-passe (AES-256-GCM), ocultar fotografias, substituição segura da galeria anterior, importação das favoritas do cliente.
14. Álbum: formatos de laboratório e presets, layouts, editor (arrastar, recorte, zoom, alinhar), criação automática pela narrativa, verificação (DPI, dobra, zona segura, rosto cortado, bordas, slots vazios, repetidas), PDF de impressão com sangria e JPEG/TIFF por spread.
15. Privacidade: tudo local, sem rede; limpar cache; apagar dados de pessoas; eliminar projeto para o Lixo; registos e relatórios de falha sem caminhos nem emails; erros da interface registados.
16. Atualização segura do catálogo (cópia antes de migrar; versão antiga recusa catálogo mais recente); registos com 14 dias.

### PARCIALMENTE FUNCIONAL

1. **macOS:** todo o código compila e é testado em Linux; o `.app`/`.dmg`, o ícone no Dock, os pedidos de permissão e o HEIC/AVIF pelo ImageIO **nunca foram executados num Mac**. Windows: só preparado.
2. Ficheiros não-RAW de 16 bits/HDR (TIFF 16, PNG 16, EXR) revelados a 8 bits; EXR/HDR cortados a sRGB; GIF/APNG só o 1.º fotograma; formatos RAW sem amostra (NRW, SRF, KDC, IIQ…) por validar.
3. Gestão de cor: entrada e saída corretas, mas sem soft proofing, aviso de gamut, perfil do monitor nem P3 no ecrã; intents só percetual/relativo.
4. Projeto: estados (3 de 14, sem interface), campos da especificação em falta, notas sem interface, pasta dos originais só na criação, exportar projeto = só o catálogo, versões só dentro do projeto.
5. Importação: cartões só como pasta montada (sem deteção de volumes/DCIM), estrutura de pastas fixa.
6. Seleção: composição e enquadramento só por inclinação/rosto cortado; expressões só sorriso/boca; contraste medido mas não pontuado; uma só etiqueta "Melhor"; reconhecimento de pessoas não entra na escolha; "Protegida" só na interface.
7. Momentos: classificação ImageNet + regras (não deteção de objetos); renomear/apagar momentos só no núcleo; falta "Entrada dos noivos" na receção.
8. Pessoas: mover um rosto para outra pessoa só no núcleo.
9. Edição: claridade só criativa; vinheta só RAW; color grading sem meios-tons; proporção livre sem razão numérica; olhos vermelhos só automático; presets não editáveis (só renomear).
10. Exportação: JPEG XL só sem perdas; ZIP só na galeria; galeria fora dos planos de exportação; avisos da fila incompletos; trabalhos em série.
11. Interface: histograma só no editor; filmstrip; progresso e monitor de tarefas por área (sem monitor unificado); pausar só exportação/auto.
12. Galeria: capa só como `og:image` relativo.
13. Álbum: panorâmicas como layout (não formato); troca de fotografias e exportação de um só spread em falta.
14. Idiomas: pt-PT completo, en existe mas não é escolhível; mensagens de erro do núcleo sempre em português.
15. Inferência só em CPU (sem GPU/Neural Engine) — funciona, mais lenta.
16. Assinatura/notarização documentadas, não feitas (falta conta Apple).

### AINDA NÃO IMPLEMENTADO

1. Build e teste reais em macOS e Windows (pendente de um Mac/PC).
2. Processo auxiliar para a LibRaw (isolamento de falhas nativas).
3. GPU (wgpu/Metal), Core ML/Neural Engine, limitação de recursos configurável.
4. Idiomas es, fr, de e seletor de idioma.
5. Deteção automática de cartões/DCIM, USB PTP/MTP, renomear na importação, modelos de estrutura de pastas.
6. Leitura de IPTC/XMP dos ficheiros; PCX, SVG e PDF como entrada; pacotes de codecs; conversão para TIFF/DNG.
7. Exportação HEIF/HEIC, DNG, GIF, MP4; ACEScg; limite de tamanho; receitas para redes sociais genéricas/Facebook; registo das exportações feitas.
8. Soft proofing, aviso de gamut, perfil do monitor, calibração, perfis de câmara (DCP), perfis de objetiva.
9. Correções automáticas de microcontraste, textura, perspetiva, distorção e rotação de 90°.
10. Seleção: balanço de brancos, direção do olhar, rostos ocultos, obstruções, estética, relevância narrativa, pessoas importantes fora de foco, classe "problema técnico", bloquear fotografias.
11. Aprendizagem de preferências entre projetos; upscaling; avaliação estética.
12. Pessoas principais por momento; câmara/objetiva nos momentos; várias objetivas/fotógrafos na consistência.
13. Sidecars XMP de edição.
14. Arquivar projeto; revisão final como modo próprio; capa/estado/progresso no ecrã inicial; modo de seleção rápida; dois monitores; "Fotografias editadas".
15. Galerias: publicação direta (S3/R2/B2/SFTP), expiração, cores personalizadas.
16. Álbuns: tipo de papel, grelha.
17. Cópia automática do catálogo para disco externo; pacote de projeto completo.

## 7. Matriz completa

Colunas pedidas: FUNCIONALIDADE · ESTADO · COMPLETA · PARCIAL · NÃO IMPLEMENTADA · MOCK ·
BUG CONHECIDO · TESTADA (nome do teste) · NÃO TESTADA. "Corrigido nesta auditoria" na
coluna de bug indica um defeito encontrado e já corrigido. Linhas sem marca nas quatro
colunas de estado são decisões ou itens não aplicáveis (ex.: "ONNX Runtime não usado").
Nenhuma funcionalidade ficou como MOCK depois das correções.

**Totais:** 479 requisitos · COMPLETA 309 · PARCIAL 91 · NÃO IMPLEMENTADA 66 · MOCK 0 · outro 13 · com bug conhecido por corrigir 10 · testados 314 · não testados 165

### A. Fundação, projeto, importação, privacidade, fiabilidade, distribuição (§1–5, 7, 8, 26–28, 30; Fases 0, 1, 13, 16)

| Secção | FUNCIONALIDADE | ESTADO | COMPLETA | PARCIAL | NÃO IMPLEMENTADA | MOCK | BUG CONHECIDO | TESTADA | NÃO TESTADA |
|---|---|---|:-:|:-:|:-:|:-:|---|---|:-:|
| §1 | Trabalhar com fotografias reais | COMPLETA | ✔ |  |  |  |  | `real_formats::import_real_files_of_all_formats`, `wedding_e2e::real_wedding_end_to_end` |  |
| §1 | Preservar os originais | COMPLETA | ✔ |  |  |  |  | `originals_are_never_modified_in_reference_mode`, `copy_mode_copies_verifies_and_keeps_originals_intact` |  |
| §1 | Edição não destrutiva | COMPLETA (fora da minha área; confirmado só por alto) | ✔ |  |  |  |  | edit_flow.rs |  |
| §1 | Usar CPU, GPU e Neural Engine | PARCIAL: só CPU (rayon + tract-onnx). Sem wgpu, Metal, Core ML ou ONNX Runtime |  | ✔ |  |  |  |  | ✔ |
| §1 | Automatizar tarefas, recomendações, revisão manual | COMPLETA (fora da minha área) | ✔ |  |  |  |  | analysis_flow, auto_edit_flow |  |
| §1 | Nunca apagar originais automaticamente | COMPLETA | ✔ |  |  |  |  | `resilience::disk_removed_goes_offline…` |  |
| §1 | Fluidez com milhares de imagens | Não confirmado aqui (Fase 12) |  |  |  |  |  | analysis_flow (15 000) |  |
| §1 | Guardar o progresso automaticamente | COMPLETA: SQLite com WAL e sincronização completa, checkpoint a cada 5 min, versões automáticas | ✔ |  |  |  |  | `full_flow::create_import_rate_close_reopen_with_hundreds_of_photos` |  |
| §1 | Recuperar projetos interrompidos | COMPLETA | ✔ |  |  |  |  | `resilience::killed_during_import…`, `killed_during_export…`, `corrupted_raw_and_damaged_catalog` |  |
| §2 | macOS Apple Silicon | PARCIAL: o build no Mac nunca foi executado. O smoke test foi feito só com o pacote Linux |  | ✔ |  |  |  |  | ✔ |
| §2 | macOS Intel | PARCIAL: só existe a opção `--universal`, que nunca foi corrida |  | ✔ |  |  |  |  | ✔ |
| §2 | Windows numa fase posterior; arquitetura não o impede | PARCIAL (preparação): há código `cfg(windows)` e a IA é Rust puro |  | ✔ |  |  |  |  | ✔ |
| §2 | Aplicação desktop, sem navegador nem servidores | COMPLETA: Tauri e núcleo Rust local | ✔ |  |  |  |  |  | ✔ |
| §3 | Interface em pt-PT | COMPLETA | ✔ |  |  |  |  | `localization/index.test.ts` |  |
| §3 | Arquitetura de internacionalização | COMPLETA | ✔ |  |  |  |  | o teste verifica que pt-PT e en têm as mesmas chaves |  |
| §3 | Preparar pt, en, es, fr e de | PARCIAL: existem só pt-PT e en (1772 linhas cada). es, fr e de estão em `PLANNED_LANGUAGES`, sem ficheiros. Não há seletor de idioma, por isso o `en` nunca é usado |  | ✔ |  |  |  | sim (só as chaves) |  |
| §3 | Nenhum texto fixo nos componentes | PARCIAL: os componentes estão limpos (só os nomes "Português"/"English" em Gallery.tsx:304), mas as mensagens de erro do núcleo Rust são texto pt fixo e aparecem ao utilizador como `detail` |  | ✔ |  |  |  |  | ✔ |
| §4 | Comparação técnica (Tauri, Electron, SwiftUI, Flutter, ORT, Core ML, Metal, DirectML) | COMPLETA (documento) | ✔ |  |  |  |  |  | ✔ |
| §4 | ONNX Runtime | NÃO USADO: foi substituído por tract-onnx (D-024, com justificação) |  |  |  |  |  |  | ✔ |
| §4 | Core ML, Metal/wgpu, DirectML, CUDA | NÃO USADOS (adiados para a Fase 17) |  |  |  |  |  |  | ✔ |
| §4 | SQLite, LibRaw, LittleCMS | USADOS |  |  |  |  |  |  | ✔ |
| §5.1 | Criar projeto | COMPLETA | ✔ |  |  |  |  | `project::create_and_read_project`; Vitest "formulário de novo projeto…" |  |
| §5.2 | Tipo de evento | COMPLETA (8 tipos) | ✔ |  |  |  |  | `rejects_invalid_input` |  |
| §5.3 | Importar | COMPLETA | ✔ |  |  |  |  | import-engine tests.rs |  |
| §5.4 | Cópia de segurança opcional | COMPLETA | ✔ |  |  |  |  | `adverse::import_with_full_backup_disk`; Vitest "importação com verificação…" |  |
| §5.5–6 | Metadados, miniaturas e previews | COMPLETA | ✔ |  |  |  |  | full_flow |  |
| §5.7 | Sincronizar relógios das câmaras | Existe (fora da minha área) |  |  |  |  |  |  | ✔ |
| §5.8–9 | Duplicados e sequências | Existe: duplicados exatos por hash na importação; grupos na análise |  |  |  |  |  | tests.rs:82 |  |
| §5.10–13 | Qualidade, rostos, momentos, seleção | Existe (fora da minha área) |  |  |  |  |  | analysis_flow, organize_flow |  |
| §5.14–18 | Revisão, correções automáticas, presets, uniformização, ajustes manuais | Existe (fora da minha área) |  |  |  |  |  | vários testes `*_flow` |  |
| §5.19 | Revisão final | PARCIAL: não há modo nem estado próprio; só os filtros e o visualizador genéricos |  | ✔ |  |  |  |  | ✔ |
| §5.20–22 | Exportar, galeria, álbum | Existe (fora da minha área) |  |  |  |  |  | export/gallery/album_flow |  |
| §5.23 | Arquivar o projeto | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §7 | Ficheiros individuais e múltiplos | COMPLETA | ✔ |  |  |  |  | `individual_files_are_imported` |  |
| §7 | Pastas e subpastas (recursivo) | COMPLETA | ✔ |  |  |  |  | `imports_folder_recursively…`, `non_recursive_only_takes_direct_files` |  |
| §7 | Arrastar e largar | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| §7 | Cartões SD/CFexpress e discos externos | PARCIAL: só pela escolha de uma pasta montada. Não há deteção de volumes nem de DCIM |  | ✔ |  |  |  | smoke test (manual) |  |
| §7 | Dispositivos USB (PTP/MTP) | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §7 | Manter os originais no sítio | COMPLETA (modo referência) | ✔ |  |  |  |  | sim |  |
| §7 | Copiar para a pasta do projeto | PARCIAL: só existe se a pasta dos originais foi definida ao criar o projeto, e depois não há forma de a definir |  | ✔ |  |  |  | `copy_mode_copies…` |  |
| §7 | Segunda localização em simultâneo | COMPLETA | ✔ |  |  |  |  | `import_with_full_backup_disk`, wedding_e2e |  |
| §7 | Verificar a integridade da cópia | COMPLETA (BLAKE3, cópia atómica) | ✔ |  |  |  |  | `copy_mode_copies_verifies…` |  |
| §7 | Estrutura de pastas automática (ANO/DATA/NOIVOS/ORIGINAIS/CÂMARA) | PARCIAL: fixa em `<originais>/<câmara>/` e `<data>_<cliente>.momento`. Não há modelos configuráveis |  | ✔ |  |  |  | `missing_paths…camera_names_cannot_escape` |  |
| §7 | Renomear durante a importação | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §7 | Detetar ficheiros já importados e duplicados por hash | COMPLETA | ✔ |  |  |  |  | `second_card_at_same_path…`, `reference_mode_same_path_new_content…` |  |
| §7 | Retomar importações interrompidas | COMPLETA | ✔ |  |  |  |  | `interrupted_import_resumes_pending_items`, `full_flow::import_interrupted_by_close…` |  |
| §8 | Nome, cliente, tipo, data, local, pasta dos originais | COMPLETA (só na criação; não podem ser editados depois) | ✔ |  |  |  |  | `create_and_read_project` |  |
| §8 | Número de fotografias | COMPLETA (calculado) | ✔ |  |  |  |  | full_flow |  |
| §8 | Câmaras e objetivas | PARCIAL: calculadas a partir das fotografias (`list_cameras`). Objetivas não aparecem por projeto |  | ✔ |  |  |  |  | ✔ |
| §8 | Pastas de cache e de backup | PARCIAL: fixas dentro do `.momento`. A pasta de backup da importação não fica guardada no projeto |  | ✔ |  |  |  |  | ✔ |
| §8 | Pasta de exportação | PARCIAL: definida por receita de exportação, não por projeto |  | ✔ |  |  |  |  | ✔ |
| §8 | Fotógrafo principal, segundo fotógrafo, assistente, preset principal, estilo visual, prazos, estado da galeria, estado do álbum | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §8 | Número esperado de fotografias finais | PARCIAL: existe como "target" das definições da seleção, não como campo do projeto |  | ✔ |  |  |  |  | ✔ |
| §8 | Notas | PARCIAL: a coluna existe, mas não há interface nem comando para escrever |  | ✔ |  |  |  |  | ✔ |
| §8 | Estado do trabalho (14 estados) | PARCIAL: só se escrevem `created`, `importing` e `imported`. Os outros 11 não existem. O estado não aparece na interface e não há comando para o mudar |  | ✔ |  |  |  | full_flow.rs:206 |  |
| §26 | Processamento local; nada enviado sem autorização | COMPLETA: não há cliente HTTP no núcleo | ✔ |  |  |  |  |  | ✔ |
| §26 | Não treinar serviços externos; dados faciais locais | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| §26 | Encriptar dados sensíveis | PARCIAL: AES-GCM só na galeria; o catálogo não é cifrado (assumido em PRIVACY.md:97–100) |  | ✔ |  |  |  |  | ✔ |
| §26 | Eliminar o projeto definitivamente | COMPLETA: "Eliminar projeto…" (com confirmação) move a pasta do projeto para o Lixo do sistema; a eliminação definitiva é esvaziar o Lixo. Nunca toca nos originais | ✔ |  |  |  | Corrigido nesta auditoria | `privacy_and_cleanup_actions` |  |
| §26 | Eliminar cache e dados faciais | COMPLETA: Segurança e cópias → Privacidade (limpar cache, apagar dados de pessoas, eliminar projeto para o Lixo) | ✔ |  |  |  | Corrigido nesta auditoria (caixas dos rostos ficam, por serem usadas na seleção — documentado) | `privacy_and_cleanup_actions`, `clear_all_only_touches_the_cache` |  |
| §26 | Credenciais fora do código | COMPLETA: nada encontrado com `git grep`; `.gitignore` cobre `.env`, `.pem` e `.p12` | ✔ |  |  |  |  |  | ✔ |
| §26 | Fotografias fora dos logs | COMPLETA: cada linha do registo passa por `redact` (caminhos e emails removidos) | ✔ |  |  |  | Corrigido nesta auditoria | `log_lines_never_carry_paths_or_emails` |  |
| §26 | Nenhuma fotografia pessoal no GitHub | COMPLETA: nenhuma imagem de câmara no histórico; só capturas de ecrã em docs/screenshots | ✔ |  |  |  |  |  | ✔ |
| §27 | Backup automático do catálogo | COMPLETA: 0/15/30/60/240 min, ao abrir e ao fechar, retenção | ✔ |  |  |  |  | `resilience::newer_catalog…` (create_backup) |  |
| §27 | Backup durante a importação, com checksum | COMPLETA | ✔ |  |  |  |  | `import_with_full_backup_disk` |  |
| §27 | Backup do catálogo para disco externo | PARCIAL: as versões ficam só em `<projeto>/backups`. Para outro disco só há "Exportar catálogo" manual |  | ✔ |  |  |  |  | ✔ |
| §27 | Histórico de versões e recuperação após falha | COMPLETA | ✔ |  |  |  |  | `corrupted_raw_and_damaged_catalog`, `older_catalog_is_backed_up_before_migration` |  |
| §27 | Aviso de originais indisponíveis; relocalizar; reconectar | COMPLETA: verificação a cada 30 s, estado offline ou em falta, relink por ficheiro ou pasta com confirmação do hash | ✔ |  |  |  |  | `disk_removed_goes_offline_and_reconnects…` |  |
| §27 | Exportar o projeto / importar noutro computador | PARCIAL: exporta só o `catalog.db`; "Importar catálogo" substitui o do projeto aberto; não há pacote com cache nem originais |  | ✔ |  |  | Corrigido nesta auditoria (P3: cache limpa ao substituir o catálogo) | `privacy_and_cleanup_actions` (restauro com cache limpa) |  |
| §28 | Estrutura de pastas (apps, packages, models, tests, scripts, docs) | COMPLETA: os 17 pacotes existem. `tests/` só tem o README e `fixtures/.gitkeep`; `docs/adr` está vazio | ✔ |  |  |  |  |  | ✔ |
| §28 | Ficheiros obrigatórios, incluindo `.env.example` e `.gitignore` | COMPLETA (todos os 15 presentes) | ✔ |  |  |  |  |  | ✔ |
| §30.4 | Botões sem funcionalidade | COMPLETA: não há handlers vazios. `UPCOMING` está vazio (código morto) | ✔ |  |  |  |  |  | ✔ |
| §30.8 / 30.10 | Não apagar originais; confirmação antes de destruir | COMPLETA | ✔ |  |  |  |  | `resilience`, `privacy_and_cleanup_actions` |  |
| §30.12 | Checkpoints no Git | COMPLETA (tags fase-0 a fase-16 e v1.0.0-personal) | ✔ |  |  |  |  |  | ✔ |
| §30.13 | PROJECT_STATUS atualizado | COMPLETA (atualizado na auditoria final com os resultados reais) | ✔ |  |  |  | Corrigido nesta auditoria (marcador por preencher) |  | ✔ |
| §30.15 | Sem fotografias, modelos ou credenciais no Git | COMPLETA: `.onnx` ignorado, só `manifest` e `lock` versionados | ✔ |  |  |  |  |  | ✔ |
| Fase 0 | Documentos, comparação, estrutura, Git | COMPLETA (`docs/adr` vazio; decisões na tabela do ARCHITECTURE) | ✔ |  |  |  |  |  | ✔ |
| Fase 1 | Ecrã inicial, criar e abrir projeto, recentes | COMPLETA | ✔ |  |  |  |  | Vitest "mostra ações e projetos recentes em pt-PT" |  |
| Fase 1 | Campos (noivos, evento = nome do projeto, data, local, originais, trabalho) | COMPLETA | ✔ |  |  |  |  | Vitest |  |
| Fase 1 | Importação JPEG/PNG/TIFF, EXIF e orientação | COMPLETA | ✔ |  |  |  |  | tests.rs; full_flow |  |
| Fase 1 | Barra lateral (Todas, Favoritas, Rejeitadas, Sem classificação, Momentos, Pessoas, Exportação) | PARCIAL: "Exportação" é um botão da barra de ferramentas, não da barra lateral |  | ✔ |  |  |  | Vitest |  |
| Fase 1 | Grelha, visualizador, classificação, atalhos, persistência | COMPLETA | ✔ |  |  |  |  | `create_import_rate_close_reopen_with_hundreds_of_photos` |  |
| Fase 13 | Backups, journaling, recuperação de sessão, checkpoints, validação | COMPLETA | ✔ |  |  |  |  | resilience.rs (6 testes) |  |
| Fase 13 | Remover só do catálogo, lixo, localizar original, relink | COMPLETA | ✔ |  |  |  | Corrigido nesta auditoria (cópias virtuais no lixo) | `privacy_and_cleanup_actions`, `read_only_volumes` |  |
| Fase 13 | Disco offline e reconexão automática | COMPLETA | ✔ |  |  |  |  | `disk_removed_goes_offline…` |  |
| Fase 13 | Checksum opcional e cópia dupla | COMPLETA | ✔ |  |  |  |  | sim |  |
| Fase 13 | Exportar e importar catálogo, backup programado, versões | COMPLETA | ✔ |  |  |  | Corrigido nesta auditoria (cache limpa ao restaurar) | `privacy_and_cleanup_actions` (restauro), Vitest |  |
| Fase 13 | Crash logs sem dados sensíveis | COMPLETA | ✔ |  |  |  | Corrigido nesta auditoria (nome dos ficheiros de registo) | `reports_are_written_without_personal_paths_and_rotated` |  |
| Fase 13 | Testes deliberados (import, export, disco, ficheiro em falta, BD bloqueada, RAW corrompido) | COMPLETA | ✔ |  |  |  |  | resilience.rs:136–439 |  |
| Fase 16 | Build reproduzível, ícone, versão, Info.plist, migrações seguras | PARCIAL: o script existe; o build macOS nunca foi executado; as permissões TCC não foram verificadas |  | ✔ |  |  |  | `older_catalog_is_backed_up_before_migration` |  |
| Fase 16 | Versão 1.0.0-personal, RELEASE_NOTES, tag Git | COMPLETA (commit e tag v1.0.0-personal depois desta auditoria; o bundle macOS usa "1.0.0" porque o macOS exige versões numéricas — D-103) | ✔ |  |  |  |  |  | ✔ |
| Fase 16 | Smoke test do build final | PARCIAL: feito só com o pacote Linux (.deb/AppImage) |  | ✔ |  |  |  | manual |  |
| Transversal | Erros da interface (exceções JS, promessas rejeitadas, falha de componente) | COMPLETA: ErrorBoundary + registo local anonimizado | ✔ |  |  |  | Corrigido nesta auditoria (antes: janela em branco sem registo) | Vitest "uma falha num componente mostra a mensagem" |  |
| Transversal | Código morto "Brevemente" na barra lateral | COMPLETA (removido) | ✔ |  |  |  |  |  | ✔ |
| Transversal | Nome PFF ProMaster e ícone próprio | COMPLETA (produto, janela, metadados exportados, Info.plist, identificador pt.pffpromaster.desktop) | ✔ |  |  |  |  |  | ✔ |
| Transversal | Edições em cache invalidadas quando o motor muda | COMPLETA (`PIPELINE_VERSION` na chave) | ✔ |  |  |  | Corrigido nesta auditoria |  | ✔ |
| Transversal | Galeria anterior só substituída quando a nova está completa | COMPLETA (gerada ao lado e trocada no fim) | ✔ |  |  |  | Corrigido nesta auditoria (antes: apagada antes de gerar) | `wedding_gallery_end_to_end` (sem pastas temporárias) |  |

### B. Formatos, exportação, gestão de cor (§6, §20, §21; Fases 2 e 9)

| Secção | FUNCIONALIDADE | ESTADO | COMPLETA | PARCIAL | NÃO IMPLEMENTADA | MOCK | BUG CONHECIDO | TESTADA | NÃO TESTADA |
|---|---|---|:-:|:-:|:-:|:-:|---|---|:-:|
| §6 | Arquitetura de adaptadores/codecs | COMPLETA (só em tempo de compilação, sem plugins) | ✔ |  |  |  |  | `candidates_are_ordered_by_priority_and_filtered`, `falls_back_to_next_codec_on_error` |  |
| §6 | JPEG/JPG | COMPLETA | ✔ |  |  |  | ✔ CMYK convertido sem perfil (é marcado "Parcial") | `probes_and_decodes_all_three_formats` + relatório |  |
| §6 | JPEG 2000 | COMPLETA | ✔ |  |  |  | ✔ profundidade gravada sempre como 8 bits no probe | relatório (balloon.jp2) |  |
| §6 | JXL | COMPLETA | ✔ |  |  |  |  | relatório |  |
| §6 | PNG | COMPLETA | ✔ |  |  |  |  | `transparent_png_is_flattened_on_white` |  |
| §6 | APNG | PARCIAL: só o 1.º fotograma, sem aviso (o GIF tem aviso) |  | ✔ |  |  |  |  | ✔ |
| §6 | TIFF/TIF | COMPLETA | ✔ |  |  |  |  | `sixteen_bit_tiff_is_converted` |  |
| §6 | BigTIFF | COMPLETA | ✔ |  |  |  | ✔ um DNG em BigTIFF é detetado como `bigtiff` raster, não como RAW | relatório (`bigtiff.dng → bigtiff`) |  |
| §6 | BMP, ICO, PPM/PGM/PBM/PAM, TGA | COMPLETA (TGA só pela extensão) | ✔ |  |  |  |  | relatório; `extension_only_formats_are_marked` |  |
| §6 | GIF | PARCIAL: 1.º fotograma |  | ✔ |  |  |  | relatório |  |
| §6 | WebP | COMPLETA (animado: 1.º fotograma) | ✔ |  |  |  |  | relatório |  |
| §6 | AVIF/HEIF/HEIC | PARCIAL: só com os codecs do sistema (ImageIO no macOS, WIC no Windows); "codec em falta" no Linux; macOS e Windows nunca validados |  | ✔ |  |  | ✔ WIC não converte a cor (um HEIC P3 é lido como sRGB) | só a deteção (`isobmff_brands`) |  |
| §6 | PCX | NÃO IMPLEMENTADA: detetado, sem descodificador |  |  | ✔ |  |  |  | ✔ |
| §6 | PSD (composto) | COMPLETA (CMYK parcial) | ✔ |  |  |  |  | relatório |  |
| §6 | PSB | PARCIAL: só um teste sintético |  | ✔ |  |  |  | sintético |  |
| §6 | OpenEXR, HDR/RGBE | PARCIAL: cortados a sRGB de 8 bits, sem mapeamento de tons |  | ✔ |  |  |  | relatório |  |
| §6 | SVG (marcas de água/templates) | NÃO IMPLEMENTADA: recusado na importação; a marca de água só aceita PNG/JPEG |  |  | ✔ |  |  |  | ✔ |
| §6 | PDF (importar gráficos) | NÃO IMPLEMENTADA (a exportação de PDF existe) |  |  | ✔ |  |  |  | ✔ |
| §6 | CR2/CR3/CRW, NEF, ARW, RAF, RW2, RWL, ORF, PEF, SRW, X3F, DCR, DNG (Leica/Pentax/DJI/Adobe/telemóvel) | COMPLETA (LibRaw 0.22.3, revelação 16 bits linear) | ✔ |  |  |  |  | relatório real; `invalid_and_truncated_data_never_crash` |  |
| §6 | NRW, SRF, SR2, RAW (Panasonic), KDC, 3FR, FFF, IIQ, MOS, MEF, MRW, ERF, ProRAW | PARCIAL: código existe, sem nenhuma amostra validada (🔶 no doc) |  | ✔ |  |  |  |  | ✔ |
| §6 | Apple HEIC/HEIF | ver AVIF/HEIF acima |  |  |  |  |  |  | ✔ |
| §6 | DNG linear | não confirmado: não há amostra |  |  |  |  |  |  | ✔ |
| §6 | Outros formatos LibRaw (1 274 câmaras) | COMPLETA | ✔ |  |  |  |  | `version_and_camera_list` |  |
| §6 | Formatos futuros por atualização de codecs | NÃO IMPLEMENTADA: os adaptadores são estáticos e compilados |  |  | ✔ |  |  |  | ✔ |
| §6 | Detetar o formato real, sem depender da extensão | COMPLETA | ✔ |  |  |  |  | `content_wins_over_extension`, `never_panics_on_short_or_random_input` |  |
| §6 | Validar cabeçalho/integridade | COMPLETA (probe na importação; a descodificação completa só na miniatura) | ✔ |  |  |  |  | `corrupt_data_returns_error_not_panic` |  |
| §6 | Ler EXIF | COMPLETA | ✔ |  |  |  |  | metadata/src/lib.rs (testes) |  |
| §6 | Ler IPTC e XMP da fotografia | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §6 | Respeitar ICC embutido | COMPLETA (miniaturas, edição e exportação; fontes a 8 bits por canal) | ✔ |  |  |  | Corrigido nesta auditoria | `embedded_icc_profile_is_honoured`, `icc_profile_is_preserved` |  |
| §6 | Profundidade de cor | COMPLETA (guardada no catálogo) | ✔ |  |  |  | ✔ só 8 bits são usados na edição de não-RAW | relatório |  |
| §6 | Espaço de cor | COMPLETA (descrição ICC/LibRaw) | ✔ |  |  |  |  | `reads_v2_and_v4_descriptions` |  |
| §6 | Transparência | PARCIAL: `has_alpha` no probe, não guardado no catálogo; composto sobre branco |  | ✔ |  |  |  | `transparent_png_is_flattened_on_white` |  |
| §6 | Orientação | COMPLETA | ✔ |  |  |  |  | `flip_mapping`, `pre_rotated_previews_are_normalized` |  |
| §6 | Previews embutidas em RAW | COMPLETA | ✔ |  |  |  |  | relatório |  |
| §6 | Previews de alta qualidade | COMPLETA (revelação total em cache ao fazer zoom) | ✔ |  |  |  |  | relatório |  |
| §6 | Registar ficheiros corrompidos | COMPLETA (tabela `import_item`, estado `corrupt`) | ✔ |  |  |  |  | `Issue 419.rw2` no relatório |  |
| §6 | Informar quando um formato não pode ser processado | COMPLETA (relatório + dica de codec) | ✔ |  |  |  |  | relatório |  |
| §6 | Nunca bloquear por um ficheiro inválido | COMPLETA para o Rust; LibRaw corre no mesmo processo (um segfault em C++ termina a aplicação) | ✔ |  |  |  | ✔ risco residual (ARCHITECTURE D-017) | `panics_in_codecs_become_corrupt_errors` |  |
| §6 | Instalar/atualizar pacotes de codecs | NÃO IMPLEMENTADA: só um texto a sugerir as extensões da Microsoft Store |  |  | ✔ |  |  |  | ✔ |
| §6 | Conversão interna para TIFF/DNG | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| F2 | "Processo auxiliar de descodificação" | NÃO IMPLEMENTADA (contradiz a decisão D-017) |  |  | ✔ |  |  |  | ✔ |
| F2 | Metadados IPTC/XMP | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §20/s10 | JPEG | COMPLETA | ✔ |  |  |  |  | `jpeg_quality_icc_and_metadata` |  |
| §20/s10 | JPEG XL | PARCIAL: só sem perdas, 8/16 bits |  | ✔ |  |  |  | `jpeg_xl_lossless_with_color_encoding` |  |
| §20/s10 | JPEG 2000 | COMPLETA (sem perdas forçado a 8 bits, com nota) | ✔ |  |  |  | ✔ sem metadados | `jpeg2000_lossless_and_lossy` |  |
| §20/s10 | PNG, TIFF, BigTIFF, WebP | COMPLETA | ✔ |  |  |  |  | `png_8_and_16_bit_are_exact`, `tiff_bigtiff_all_depths_and_compressions`, `webp_lossless_exact_and_lossy_good` |  |
| §20/s10 | AVIF | COMPLETA (8/10/12 bits, CICP) | ✔ |  |  |  |  | `avif_container_and_color_signalling` |  |
| §20/s10 | HEIF/HEIC | NÃO IMPLEMENTADA (desativada com explicação honesta) |  |  | ✔ |  |  | `unavailable_formats_refuse_honestly` |  |
| §20/s10 | PSD renderizado | COMPLETA (sem camadas) | ✔ |  |  |  |  | `psd_raw_and_rle` |  |
| §20/s10 | OpenEXR | COMPLETA | ✔ |  |  |  |  | `openexr_half_and_float_linear` |  |
| §20/s10 | DNG / DNG linear | NÃO IMPLEMENTADA (desativada com explicação) |  |  | ✔ |  |  | idem |  |
| §20/s10 | PDF | COMPLETA (uma imagem por página; os álbuns têm PDF próprio) | ✔ |  |  |  |  | `pdf_structure_is_valid` |  |
| §20 | GIF de sequências, MP4 de slideshows | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §20 | Pacote ZIP | PARCIAL: só os downloads da galeria; não é um formato nem uma receita |  | ✔ |  |  |  | `gallery_flow` |  |
| §20 | Galeria HTML estática | PARCIAL: existe (módulo Galeria, passa pela fila de exportação), mas não pode ser incluída num plano de exportação |  | ✔ |  |  |  | `gallery_flow` |  |
| §20 | Não exportar para RAW proprietário | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| §20 | 8 / 10 / 12 / 16 bits, 16f, 32f | COMPLETA "conforme o formato" (10 bits só AVIF; 16f só EXR) | ✔ |  |  |  |  | testes de qualidade |  |
| §20 | sRGB, P3, Adobe RGB, ProPhoto, Rec.709, Rec.2020 | COMPLETA | ✔ |  |  |  |  | `matrix_conversion_matches_lcms_with_embedded_profile`, `wide_gamut_roundtrip_through_embedded_profile` |  |
| §20 | ACEScg | NÃO IMPLEMENTADA ("quando aplicável") |  |  | ✔ |  |  |  | ✔ |
| §20 | ICC personalizado / do laboratório | COMPLETA (só perfis RGB) | ✔ |  |  |  |  | `custom_icc_profile` |  |
| §20/s10 | Qualidade, compressão, com/sem perdas | COMPLETA | ✔ |  |  |  |  | testes de qualidade |  |
| §20/s10 | Resolução, PPI, largura, altura, lado longo/curto, caber, recortar (fill), manter proporção, megapíxeis | COMPLETA | ✔ |  |  |  |  | `sizes` |  |
| §20 | Limite máximo de tamanho (ficheiro) | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §20/s10 | Nome, prefixo, sufixo, numeração, subpastas, conflitos (nunca sobre originais) | COMPLETA | ✔ |  |  |  |  | `names_tokens_and_sanitizing`, `conflicts`, `overwrite_never_targets_an_original...` |  |
| §20/s10 | Metadados, direitos de autor, remover GPS, manter/remover EXIF | COMPLETA | ✔ |  |  |  |  | `exif_is_readable_and_gps_follows_the_option`, `xmp_escapes...` |  |
| §20/s10 | Marca de água (texto/imagem) | COMPLETA (sem SVG) | ✔ |  |  |  |  | `text_watermark_lands_in_the_chosen_corner`, `oversized_watermark_is_scaled_not_dropped` |  |
| §20/s10 | Nitidez de saída | COMPLETA | ✔ |  |  |  |  | `sharpen_increases_edge_contrast_only_when_asked` |  |
| §20 | Fundo para formatos sem transparência | NÃO IMPLEMENTADA (o pipeline nunca tem alfa) |  |  | ✔ |  |  |  | ✔ |
| §20 | Orientação | PARCIAL: aplicada à imagem; só existe a opção `matchOrientation` para o fill |  | ✔ |  |  |  |  | ✔ |
| §20 | Exportação paralela | PARCIAL: pipeline de 2 threads (decode ∥ encode) + rayon; os trabalhos correm em série |  | ✔ |  |  |  |  | ✔ |
| s10 | Usar o RAW/original + edição, nunca uma preview | COMPLETA para RAW; não-RAW a 8 bits por canal (com o perfil ICC embutido aplicado) | ✔ |  |  |  | ✔ Limitação (16 bits não-RAW → 8 bits): ver KNOWN_LIMITATIONS.md | `wedding_export_end_to_end`; `raw_export_uses_full_resolution_original` |  |
| s10 | Alta Res., Galeria, Website, Email, Instagram 4:5 e Square, Impressão, Album, Arquivo TIFF | COMPLETA | ✔ |  |  |  |  | `builtins_are_consistent` |  |
| s10/§20 | Arquivo DNG | NÃO IMPLEMENTADA (desativado com explicação) |  |  | ✔ |  |  | idem |  |
| §20 | Redes sociais (genérico), Facebook, Pré-visualização, Entrega sem metadados privados, Slideshow, Pacote completo do cliente | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| s10 | Multi-export "Exportar casamento" com 5 pastas | COMPLETA | ✔ |  |  |  |  | `wedding_export_end_to_end`; Vitest "diálogo: plano com várias receitas…" |  |
| §20 | Exemplo de 7 saídas (com DNG, HTML, ZIP) | PARCIAL: 5 de 7 |  | ✔ |  |  |  |  | ✔ |
| s10 | Fila de exportação (pausa/retoma/cancelamento, retoma após interrupção) | COMPLETA | ✔ |  |  |  |  | `export_flow`; Vitest "fila: progresso…" |  |
| §20/s10 | Progresso, tempo restante, n.º de ficheiros, fotografias restantes, erros | COMPLETA | ✔ |  |  |  |  | Vitest "fila…" |  |
| §20/s10 | Tamanho estimado, espaço necessário, espaço disponível | COMPLETA | ✔ |  |  |  |  | `wedding_export_end_to_end` |  |
| §20 | Avisos | PARCIAL: só no diálogo (estimativa); `progress.notes` nunca aparece na fila |  | ✔ |  |  |  |  | ✔ |
| §20 | Ficheiros ignorados | PARCIAL: contados em `processed`, mas sem número próprio |  | ✔ |  |  |  |  | ✔ |
| §21 | Leitura de perfis ICC | COMPLETA (descrição, validação e uso como perfil de origem) | ✔ |  |  |  | Corrigido nesta auditoria | `embedded_input_profiles_round_trip` |  |
| §21 | Conversão entre espaços | COMPLETA: entrada pelo perfil ICC embutido → Rec. 2020 linear; saída para o espaço escolhido | ✔ |  |  |  | Corrigido nesta auditoria (antes só na saída) | `embedded_icc_profile_is_honoured`, `embedded_input_profiles_round_trip` |  |
| §21 | Soft proofing | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §21 | Aviso de gamut | NÃO IMPLEMENTADA (o fora-de-gama é cortado em silêncio) |  |  | ✔ |  |  |  | ✔ |
| §21 | Perfis de câmara | PARCIAL: só as matrizes internas da LibRaw; sem DCP nem escolha de perfil |  | ✔ |  |  |  |  | ✔ |
| §21 | Perfis de monitor | NÃO IMPLEMENTADA (fica a cargo do WebView) |  |  | ✔ |  |  |  | ✔ |
| §21 | Perfis de impressão | PARCIAL: ICC de laboratório na exportação; sem prova |  | ✔ |  |  |  | `custom_icc_profile` |  |
| §21 | Rendering intent | PARCIAL: só percetual e relativo, e só com ICC personalizado |  | ✔ |  |  |  |  | ✔ |
| §21 | Compensação do ponto negro | PARCIAL: automática só no intento relativo, sem opção |  | ✔ |  |  |  |  | ✔ |
| §21 | Monitorização Display P3 | NÃO IMPLEMENTADA: o render do editor é cortado a sRGB |  |  | ✔ |  |  |  | ✔ |
| §21 | Monitores calibrados | não confirmado (depende do WebView ou do sistema) |  |  |  |  |  |  | ✔ |
| §21 | Avaliar LittleCMS / OCIO / ColorSync | LittleCMS usada; OCIO e ColorSync só no documento |  |  |  |  |  |  | ✔ |
| §21 | Consistência preview / edição / exportação / impressão | PARCIAL: grelha, editor e exportação usam o perfil embutido; a pré-visualização é sRGB (sem perfil do monitor) |  | ✔ |  |  |  | `embedded_icc_profile_is_honoured` |  |

### C. Momentos, seleção, pessoas, IA (§9–12, §24, §25; Fases 3 e 4)

| Secção | FUNCIONALIDADE | ESTADO | COMPLETA | PARCIAL | NÃO IMPLEMENTADA | MOCK | BUG CONHECIDO | TESTADA | NÃO TESTADA |
|---|---|---|:-:|:-:|:-:|:-:|---|---|:-:|
| §9 / spec_05 | 25 momentos predefinidos | PARCIAL: falta "Entrada dos noivos" na receção (§9). `entrance` é a entrada na cerimónia |  | ✔ |  |  |  | `wedding_day_is_classified…` (moments/tests.rs:179) |  |
| §9 | Usa data e hora (corrigida) | COMPLETA | ✔ |  |  |  |  | moments/tests.rs:179 |  |
| §9 | Usa GPS (cenas >400 m) | COMPLETA | ✔ |  |  |  |  | `gps_jump_splits_scenes` |  |
| §9 | Usa câmara | NÃO IMPLEMENTADA: `MomentPhoto` não tem câmara |  |  | ✔ |  |  |  | ✔ |
| §9 | Usa objetiva | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §9 | Semelhança visual | PARCIAL: só para propagar escolhas do utilizador (cos ≥0,85) |  | ✔ |  |  |  | `user_choices_teach_similar…` |  |
| §9 | Rostos | COMPLETA (contagem, grupo posado, beijo, casal) | ✔ |  |  |  |  | moments/tests.rs |  |
| §9 | Objetos | PARCIAL: classificação ImageNet, não deteção; não há classe "anel" nem "bolo de casamento" |  | ✔ |  |  |  | `identity_and_scene_match_official_runtimes` (só com ficheiros de referência) |  |
| §9 | Cenário | PARCIAL (as mesmas classes ImageNet, "scenic") |  | ✔ |  |  |  | idem |  |
| §9 | Iluminação | COMPLETA de forma simples (brilho <0,3 ou ISO ≥3200) | ✔ |  |  |  |  | moments/tests.rs |  |
| §9 | Intervalos temporais | COMPLETA (cenas separadas por mais de 10 min) | ✔ |  |  |  |  | moments/tests.rs |  |
| §9 | Sequência narrativa | PARCIAL: só "antes/durante/depois da cerimónia" e "fim do dia" |  | ✔ |  |  |  | moments/tests.rs:179 |  |
| §9 | Classificação visual por IA | PARCIAL: classificador ImageNet com regras manuais (D-030/D-031) |  | ✔ |  |  |  |  | ✔ |
| §9 / spec_05 | "Por classificar" quando a confiança é baixa | COMPLETA (mínimo 0,5, margem 0,08) | ✔ |  |  |  |  | `without_ceremony_or_cues_nothing_is_forced`; organize_flow:199 |  |
| §9 / spec_05 | Arrastar entre momentos | COMPLETA | ✔ |  |  |  |  | Vitest "momentos: filtro, por classificar e arrastar…" |  |
| §9 / spec_05 | Momentos personalizados | COMPLETA (criar) | ✔ |  |  |  |  | Vitest "criar momento personalizado…"; `moments_user_wins_over_auto_and_custom_moments` |  |
| §9 | Alterar e personalizar todos os grupos | PARCIAL: `rename_moment` e `delete_moment` existem no núcleo e como comando, mas nenhum componente os chama. Não se reordena |  | ✔ |  |  |  | Rust só |  |
| spec_05 | Sincronização de hora entre câmaras + reconstrução da cronologia | COMPLETA (sugestão por embeddings, correção aplicada em SQL, seleção e momentos recalculados) | ✔ |  |  |  |  | `recovers_sony_plus_3m42_and_dji_minus_1m15`; `offsets_reorder_the_timeline_without_touching_exif`; Vitest "sincronizar câmaras" |  |
| §10 | Nitidez | COMPLETA | ✔ |  |  |  |  | `blur_lowers_sharpness_and_focus` |  |
| §10 | Ponto de foco | COMPLETA (pico 6×4, "missedFocus") | ✔ |  |  |  |  | `focus_peak_finds_the_sharp_region` |  |
| §10 | Nitidez dos rostos e dos olhos | COMPLETA | ✔ |  |  |  |  | analysis_flow `analyze_group_recommend_accept_reopen` |  |
| §10 | Movimento | COMPLETA | ✔ |  |  |  |  | `motion_blur_is_detected` |  |
| §10 | Ruído | COMPLETA | ✔ |  |  |  |  | `noise_estimate_grows_with_noise` |  |
| §10 | Exposição, realces queimados, sombras bloqueadas | COMPLETA | ✔ |  |  |  |  | `exposure_and_clipping` |  |
| §10 | Contraste | PARCIAL: medido e gravado, mas não entra em nenhuma pontuação |  | ✔ |  |  |  |  | ✔ |
| §10 | Balanço de brancos | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §10 | Composição | PARCIAL: só inclinação e rosto cortado |  | ✔ |  |  |  |  | ✔ |
| §10 | Horizonte | COMPLETA | ✔ |  |  |  |  | `tilt_detection_on_rotated_lines` |  |
| §10 | Enquadramento | PARCIAL: só rosto cortado |  | ✔ |  |  |  |  | ✔ |
| §10 | Olhos fechados e piscadelas | COMPLETA (incluindo piscadela relativa por pessoa) | ✔ |  |  |  |  | `relative_blink_within_a_person_track`; `laughing_squint_is_not_closed_eyes` |  |
| §10 | Expressões | PARCIAL: só sorriso e "a falar" (jawOpen) |  | ✔ |  |  |  |  | ✔ |
| §10 | Direção do olhar | NÃO IMPLEMENTADA (os blendshapes `eyeLook*` estão disponíveis mas não são usados) |  |  | ✔ |  |  |  | ✔ |
| §10 | Rostos ocultos | NÃO IMPLEMENTADA (rostos com presença <0,5 são simplesmente descartados) |  |  | ✔ |  |  |  | ✔ |
| §10 | Rostos parcialmente cortados | COMPLETA | ✔ |  |  |  |  | analysis_flow |  |
| §10 / spec_04 | Número de pessoas em foco; pessoas importantes fora de foco | NÃO IMPLEMENTADA: a seleção não usa identidades nem papéis |  |  | ✔ |  |  |  | ✔ |
| §10 | Obstruções | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §10 | Qualidade estética | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §10 | Relevância narrativa | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §10 | Diversidade da seleção | PARCIAL: só com objetivo de entrega definido |  | ✔ |  |  |  | `target_count_is_respected_and_favours_diversity` |  |
| §10 | Escolha principal / Recomendada | PARCIAL: uma só etiqueta "best" ("Melhor") cobre as duas |  | ✔ |  |  |  | `kiss_sequence_is_grouped_and_best_is_explained` |  |
| §10 | Alternativa, Rever, Possível rejeição, Duplicada | COMPLETA | ✔ |  |  |  |  | selection/tests.rs |  |
| §10 | Problema técnico | NÃO IMPLEMENTADA como classe (incluído em "reject") |  |  | ✔ |  |  |  | ✔ |
| §10 | Protegida pelo utilizador | PARCIAL: a interface mostra "Protegida" (favorita ou com estrelas) em vez de rejeição/duplicada sugerida; o núcleo não tem estado próprio |  | ✔ |  |  | Corrigido o sintoma (etiqueta enganadora) | Vitest "uma favorita ou com estrelas aparece como protegida" |  |
| §10 / spec_04 | Nunca eliminar automaticamente | COMPLETA (aceitar só toca em fotografias com flag=0, sem estrelas nem cor) | ✔ |  |  |  |  | `analysis_roundtrip_selection_and_accept` |  |
| §10 / spec_04 | Explicar a recomendação | COMPLETA (razões estruturadas traduzidas em frase) | ✔ |  |  |  |  | Vitest "explicações em linguagem natural", "painel de informação…" |  |
| spec_04 | Modos conservador, equilibrado e rigoroso | COMPLETA | ✔ |  |  |  |  | `modes_change_how_many_are_kept`; Vitest "modo e objetivo…" |  |
| spec_04 | Número aproximado de fotografias a entregar | COMPLETA | ✔ |  |  |  |  | `target_count…` |  |
| spec_04 | Scores individuais gravados | COMPLETA | ✔ |  |  |  |  | `analysis_roundtrip…` |  |
| spec_04 | Desempenho em grupos grandes | COMPLETA | ✔ |  |  |  |  | `performance_with_15000_photos`, `performance_15000_photos`, `performance_30000_faces` |  |
| §11 / spec_04 | Duplicados exatos e cópias com outro nome | COMPLETA, mas na importação (blake3, a cópia não entra no catálogo), não como grupo | ✔ |  |  |  |  |  | ✔ |
| §11 | Versões em resoluções diferentes | COMPLETA (pHash ≤4 + embedding ≥0,97 → "duplicate") | ✔ |  |  |  |  | `raw_jpeg_pairs_and_near_duplicates` |  |
| §11 | RAW e JPEG correspondentes | COMPLETA (mesma pasta e mesmo nome base) | ✔ |  |  |  | ✔ Ficheiros em pastas diferentes não são emparelhados | idem |  |
| §11 | Fotografias semelhantes, rajadas, sequências, enquadramentos | COMPLETA (até 3 s / 15 s, embedding e pHash) | ✔ |  |  |  |  | `kiss_sequence…` |  |
| §11 | Várias câmaras no mesmo instante | COMPLETA (hora corrigida, até 2 s) | ✔ |  |  |  |  | `moments_far_apart_…two_cameras_at_once_are` |  |
| §11 | Técnicas: hash de ficheiro, pHash, embeddings, data, metadados | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| §11 | Técnica: reconhecimento de pessoas | PARCIAL: só o número de rostos, não a identidade |  | ✔ |  |  |  |  | ✔ |
| §11 | Técnica: comparação da composição | PARCIAL (idem) |  | ✔ |  |  |  |  | ✔ |
| §11 | Recomendar a melhor do grupo | COMPLETA | ✔ |  |  |  |  | `kiss_sequence…` |  |
| §11 | Aceitar a recomendação | COMPLETA (por fotografia e por grupo) | ✔ |  |  |  |  | Vitest "vista de sequências…" |  |
| §11 / spec_04 | Escolher outra, manter várias, rejeitar várias | COMPLETA (trocar na comparação, aceitar/rejeitar, seleção múltipla) | ✔ |  |  |  |  | Vitest "comparação: teclas…", "seleção múltipla…" |  |
| §11 / spec_04 | Lado a lado, zoom e pan sincronizados, estrelas | COMPLETA (2 a 4 painéis) | ✔ |  |  |  |  | Vitest "comparação sincronizada" |  |
| §11 | Bloquear fotografias importantes | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §12 | Deteção de rostos | COMPLETA | ✔ |  |  |  |  | `portrait_matches_mediapipe_reference` |  |
| §12 / spec_05 | Agrupamento de rostos | COMPLETA (SFace + centroides) | ✔ |  |  |  |  | `groups_same_people_and_keeps_strangers_apart` (+2 testes) |  |
| §12 / spec_05 | Identificar noivos, pais, irmãos, padrinhos, madrinhas, importantes | COMPLETA (nome e papel) | ✔ |  |  |  |  | Vitest "gerir pessoas…" |  |
| §12 | Corrigir o agrupamento | PARCIAL: juntar e "não é esta pessoa" existem. `assign_faces` (mover um rosto para outra pessoa) não tem comando Tauri |  | ✔ |  |  |  | Rust |  |
| §12 / spec_05 | Pesquisa por pessoa; noivos juntos; noiva + mãe; noivo + pai | COMPLETA (filtro AND; papéis com 2+ pessoas pedem para as juntar) | ✔ |  |  |  | Corrigido nesta auditoria (antes usava só a 1.ª pessoa do papel) | Vitest "papéis com mais do que uma pessoa"; organize_flow |  |
| §12 | Fotografias de grupos | PARCIAL: só como momento "groups" |  | ✔ |  |  |  |  | ✔ |
| §12 | Olhos fechados; nitidez facial | COMPLETA | ✔ |  |  |  |  | ver §10 |  |
| §12 | Pessoas principais em cada momento | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §12 | Dados biométricos guardados localmente | COMPLETA (`catalog.db` dentro da pasta do projeto, cópias de segurança incluídas) | ✔ |  |  |  |  |  | ✔ |
| §12 | Eliminados quando o projeto é eliminado | COMPLETA: Eliminar projeto move a pasta inteira (catálogo, rostos, versões, cache) para o Lixo; Apagar dados de pessoas apaga assinaturas faciais, pessoas e nomes | ✔ |  |  |  | Corrigido nesta auditoria (as caixas dos rostos e os embeddings de imagem ficam até eliminar o projeto — documentado em PRIVACY.md) | `privacy_and_cleanup_actions`; organize_flow (`clear_people`) |  |
| §12 | Nada enviado para fora | COMPLETA (não há código de rede nesta área) | ✔ |  |  |  |  |  | ✔ |
| §24 | Deteção de rostos, agrupamento, olhos fechados, expressões, qualidade facial | COMPLETA (qualidade facial por nitidez e presença) | ✔ |  |  |  |  | testes acima |  |
| §24 | Deteção de objetos | PARCIAL (classificação, não deteção) |  | ✔ |  |  |  |  | ✔ |
| §24 | Segmentação | COMPLETA no núcleo (`segment.rs`, MediaPipe); a interface das máscaras é de outra área | ✔ |  |  |  |  |  | ✔ |
| §24 | Avaliação estética | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §24 | Nitidez e ruído | COMPLETA, mas com métodos clássicos e não modelos | ✔ |  |  |  |  | metrics tests |  |
| §24 | Denoise | COMPLETA no núcleo (FFDNet); a interface é de outra área | ✔ |  |  |  |  |  | ✔ |
| §24 | Upscaling | NÃO IMPLEMENTADA (sem Real-ESRGAN nem alternativa) |  |  | ✔ |  |  |  | ✔ |
| §24 | Organização por momentos; melhor fotografia | COMPLETA, com regras sobre saídas de modelos | ✔ |  |  |  |  | ver acima |  |
| §24 | Recorte inteligente | PARCIAL: heurístico com rostos, no Auto; sem modelo |  | ✔ |  |  |  | `auto_crop_keeps_people_and_improves_composition` |  |
| §24 | Avaliar runtimes, licença, tamanho, velocidade… | COMPLETA para os modelos escolhidos (D-024 a D-030). Real-ESRGAN não foi avaliado | ✔ |  |  |  |  |  | ✔ |
| §25 | Aprendizagem de preferências ("numa fase posterior") | NÃO IMPLEMENTADA. Só existe, dentro de cada projeto, a propagação de momentos a partir de exemplos do utilizador |  |  | ✔ |  |  |  | ✔ |

### D. Edição, auto edit, presets, consistência, catálogo de edição (§13–16, §22; Fases 5–8)

| Secção | FUNCIONALIDADE | ESTADO | COMPLETA | PARCIAL | NÃO IMPLEMENTADA | MOCK | BUG CONHECIDO | TESTADA | NÃO TESTADA |
|---|---|---|:-:|:-:|:-:|:-:|---|---|:-:|
| §13 | Motor auto não destrutivo, analisa cada foto | COMPLETA | ✔ |  |  |  |  | TA `each_photo_gets_its_own_settings`; auto_edit_flow |  |
| §13 | Exposição | COMPLETA (pela chave da cena e pelos rostos) | ✔ |  |  |  |  | TA |  |
| §13 | Contraste | COMPLETA | ✔ |  |  |  |  | TA |  |
| §13 | Realces / recuperação de realces | COMPLETA | ✔ |  |  |  |  | TA |  |
| §13 | Sombras / recuperação de sombras | COMPLETA | ✔ |  |  |  |  | TA |  |
| §13 | Brancos, Pretos | COMPLETA | ✔ |  |  |  |  | TA |  |
| §13 | Temperatura, Matiz, Balanço de brancos | COMPLETA (iluminante por arestas, correção parcial) | ✔ |  |  |  |  | TA |  |
| §13 | Saturação, Vibrance | COMPLETA | ✔ |  |  |  |  | TA |  |
| §13 | Curva de tons | COMPLETA (só um S suave em lights/darks) | ✔ |  |  |  |  | TA |  |
| §13 | Nitidez | COMPLETA | ✔ |  |  |  |  | TA |  |
| §13 | Microcontraste | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §13 | Textura | NÃO IMPLEMENTADA no auto |  |  | ✔ |  |  |  | ✔ |
| §13 | Claridade | PARCIAL: só um extra "criativo"; é 0 em Muito suave, Suave e Natural, e nunca é medida |  | ✔ |  |  |  |  | ✔ |
| §13 | Redução de ruído (luminância e cor) | COMPLETA (medida + ISO + sensor) | ✔ |  |  |  |  | TA `noise_is_measured…`, `sensor_table` |  |
| §13 | Redução de ruído com IA | COMPLETA (FFDNet opcional, calculado em 2.º plano; sem o modelo cai para a redução rápida) | ✔ |  |  |  |  | TA; auto_edit_flow (só se o modelo existir) |  |
| §13 | Aberração cromática | COMPLETA (estimada) | ✔ |  |  |  |  | TA `chromatic_aberration_is_estimated` |  |
| §13 | Distorção da objetiva | NÃO IMPLEMENTADA no auto (sem perfis de objetiva) |  |  | ✔ |  |  |  | ✔ |
| §13 | Vinheta da objetiva | PARCIAL (só em RAW) |  | ✔ |  |  |  |  | ✔ |
| §13 | Perspetiva | NÃO IMPLEMENTADA no auto |  |  | ✔ |  |  |  | ✔ |
| §13 | Horizonte | COMPLETA (só 0,25–10°) | ✔ |  |  |  |  | TA `horizon_is_straightened…` |  |
| §13 | Rotação | PARCIAL (só endireitar; não há rotação automática de 90°) |  | ✔ |  |  |  |  | ✔ |
| §13 | Recorte | COMPLETA (opcional) | ✔ |  |  |  |  | TA `auto_crop…` |  |
| §13 | Olhos vermelhos | COMPLETA, mas depende dos modelos de rosto | ✔ |  |  |  |  | TA `red_eyes…` |  |
| §13 | Tons de pele | PARCIAL: no auto só protege a pele no vibrance; correção real só na consistência |  | ✔ |  |  |  |  | ✔ |
| §13 | Intensidades (Muito suave … Intensa, Personalizada) | COMPLETA | ✔ |  |  |  |  | TA `intensities_are_ordered`; CT "edição automática" |  |
| §14 Luz | Exposição, Contraste, Realces, Sombras, Brancos, Pretos | COMPLETA (todos usados no render) | ✔ |  |  |  |  | TR |  |
| §14 Luz | Curva de tons (paramétrica, pontos RGB e por canal, divisões) | COMPLETA | ✔ |  |  |  |  | TR `curves_parametric…` |  |
| §14 Cor | Temperatura, Matiz (+ seletor de balanço de brancos) | COMPLETA | ✔ |  |  |  |  | TR `white_balance…`; CT "conta-gotas" |  |
| §14 Cor | Saturação, Vibrance, HSL | COMPLETA | ✔ |  |  |  |  | TR |  |
| §14 Cor | Color grading | PARCIAL: sombras e realces + equilíbrio; sem meios-tons nem global |  | ✔ |  |  |  | TR `black_white_mix_grading_and_lut` |  |
| §14 Cor | Calibração | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §14 Cor | Perfis de câmara | NÃO IMPLEMENTADA (não há perfil selecionável) |  |  | ✔ |  |  |  | ✔ |
| §14 Cor | Perfis ICC (de entrada, na edição) | COMPLETA (perfil embutido convertido para o espaço de trabalho; 8 bits) | ✔ |  |  |  | Corrigido nesta auditoria | `embedded_icc_profile_is_honoured`, `embedded_input_profiles_round_trip` |  |
| §14 Detalhe | Nitidez, Raio, Detalhe, Máscara | COMPLETA (só no render final) | ✔ |  |  |  |  | TR `detail_sharpening…` |  |
| §14 Detalhe | Redução de ruído (+ preservação de detalhe), ruído cromático | COMPLETA | ✔ |  |  |  |  | TR |  |
| §14 Detalhe | Denoise por IA | COMPLETA (exige `denoise_ffdnet.onnx`; aplicado à fonte antes de tudo) | ✔ |  |  |  |  | CT `detail.aiDenoise`; auto_edit_flow |  |
| §14 Efeitos | Textura, Claridade, Desembaçar (dehaze), Vinheta (4 parâmetros), Grão (3 parâmetros) | COMPLETA | ✔ |  |  |  |  | TR `effects_clarity_dehaze…` |  |
| §14 Geometria | Recorte, Rotação 90°, Espelhar, Endireitar | COMPLETA | ✔ |  |  |  |  | TR `geometry…` |  |
| §14 Geometria | Alinhamento automático / Horizonte | COMPLETA | ✔ |  |  |  |  | TR `auto_straighten…`; CT |  |
| §14 Geometria | Perspetiva, Correção vertical e horizontal | COMPLETA (sliders manuais; não há "upright" automático) | ✔ |  |  |  |  | TR |  |
| §14 Geometria | Correção de lente (distorção, aberração cromática, vinheta) | COMPLETA manual; sem perfis de objetiva | ✔ |  |  |  |  | TR `lens_corrections…` |  |
| §14 Geometria | Proporção personalizada | PARCIAL: lista fixa + "livre"; não se pode escrever uma razão numérica |  | ✔ |  |  |  | CT "recorte" |  |
| §14 Máscaras / F8 | Pincel (com fluxo, suavidade, apagar) | COMPLETA | ✔ |  |  |  |  | TL; CT "pincel" |  |
| §14 Máscaras / F8 | Gradiente linear, Gradiente radial | COMPLETA | ✔ |  |  |  |  | TL `shapes_and_combination` |  |
| §14 Máscaras / F8 | Luminância, Intervalo de cor | COMPLETA | ✔ |  |  |  |  | TL |  |
| §14 Máscaras / F8 | Pessoa, Sujeito, Rosto, Pele, Cabelo, Roupa, Fundo, Objeto | COMPLETA (IA local: segment_multiclass/segment_object). "Rosto" usa só a classe pele do rosto | ✔ |  |  |  |  | local_edit_flow (só se houver retrato e modelos) |  |
| §14 Máscaras / F8 | Céu | COMPLETA (heurística, sem IA) | ✔ |  |  |  |  | IP/sky.rs `blue_sky…`; local_edit_flow |  |
| F8 | Adicionar, Subtrair, Intersetar, Inverter (parte e máscara), Densidade, Suavização da borda, Fluxo | COMPLETA | ✔ |  |  |  |  | TL; CT |  |
| F8 | Ajustes dentro da máscara: exposição, contraste, brancos, pretos, temperatura, matiz, saturação, textura, claridade, nitidez, ruído | COMPLETA | ✔ |  |  |  |  | TL `local_adjustments…` |  |
| F8 | Realces e Sombras locais | COMPLETA (corrigido: realces/sombras locais agem no render) | ✔ |  |  |  | Corrigido nesta auditoria | `local_adjustments_stay_inside_the_mask` (realces e sombras separados) |  |
| §14 Retoque | Remoção de manchas, Clone, Heal | COMPLETA (sugere a origem automaticamente) | ✔ |  |  |  |  | TL `heal_clone_and_object_removal` |  |
| §14 Retoque | Sensor sujo | COMPLETA (deteção automática + mancha manual) | ✔ |  |  |  |  | TA `sensor_dust…` |  |
| §14 Retoque | Remoção de objetos simples | COMPLETA (patch com pincel: copia uma zona deslocada; não é inpainting) | ✔ |  |  |  |  | TL |  |
| §14 Retoque | Suavização de pele, Olheiras, Brilhos, Dentes, Olhos | COMPLETA (só nas regiões da malha facial; exige os modelos de rosto) | ✔ |  |  |  |  | TL `portrait_retouch_is_subtle_and_local`; CT |  |
| §14 Retoque | Olhos vermelhos | PARCIAL: só deteção automática pelas íris; não há ferramenta manual de clique |  | ✔ |  |  |  | TA |  |
| §14 | Todos os ajustes reversíveis | COMPLETA (receita + histórico) | ✔ |  |  |  |  | edit_flow |  |
| §15 / F7 | Criar/guardar (com escolha de componentes) | COMPLETA | ✔ |  |  |  |  | CT "criar preset"; `store_lifecycle…` |  |
| §15 | Editar | PARCIAL: só renomear ou mudar de grupo; não dá para alterar os valores de um preset |  | ✔ |  |  |  |  | ✔ |
| §15 | Renomear, Organizar (grupos + ordem), Duplicar, Eliminar | COMPLETA | ✔ |  |  |  |  | CT "gerir"; `store_lifecycle…` |  |
| §15 | Importar e exportar (.momentopreset com LUT, .xmp, .cube) | COMPLETA (XMP só importação, aproximada, com relatório) | ✔ |  |  |  |  | `store_lifecycle…`; presets `xmp_import…` |  |
| §15 | Pré-visualizar | COMPLETA (ao passar o rato no editor) | ✔ |  |  |  |  | CT |  |
| §15 | Aplicar a foto, seleção, momento, projeto | COMPLETA | ✔ |  |  |  |  | CT "aplicar preset"; presets_consistency_flow |  |
| §15 | Ajustar intensidade (0–150 %) | COMPLETA | ✔ |  |  |  |  | presets `amount_blends…` |  |
| §15 | Sincronizar parâmetros específicos | COMPLETA (blocos para colar + componentes do preset) | ✔ |  |  |  |  | CT |  |
| §15 | Combinar preset com correção automática | PARCIAL: não há ação combinada; só aplicando em sequência (os presets de "aspeto" não mexem em exposição nem balanço) |  | ✔ |  |  |  |  | ✔ |
| §15 | XMP | PARCIAL (importação aproximada; exportação recusada de propósito) |  | ✔ |  |  |  | sim |  |
| §15 | LUT .cube | COMPLETA (sRGB, 1D/3D, LUT log recusadas) | ✔ |  |  |  |  | lut.rs testes |  |
| §15 | DNG profiles | NÃO IMPLEMENTADA (avaliado: ignorado no XMP) |  |  | ✔ |  |  |  | ✔ |
| §15 | ICC em presets | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| §15 | Perfis próprios (.momentopreset) | COMPLETA | ✔ |  |  |  |  | `file_roundtrip_with_lut` |  |
| §15 | 13 presets base | COMPLETA (nomes B&W em inglês) | ✔ |  |  |  |  | `builtin_presets_are_complete_and_distinct` |  |
| §16 / F7 | Agrupar por iluminação, balanço, flash, câmara, cenário (momento), horário, combinado | COMPLETA | ✔ |  |  |  |  | presets_consistency_flow; CT |  |
| §16 | Várias objetivas / diferentes fotógrafos | NÃO IMPLEMENTADA: "câmara" = só marca + modelo, sem objetiva nem número de série |  |  | ✔ |  |  |  | ✔ |
| §16 | Interior/exterior, dia/noite | PARCIAL (aproximado pelo EV da cena: bright/shade/indoor/dark) |  | ✔ |  |  |  |  | ✔ |
| §16 / F7 | Referência + igualar exposição, balanço de brancos, pele, contraste, cor, aspeto (medido no resultado, não copiado) | COMPLETA | ✔ |  |  |  |  | IP/consistency.rs `syncing_matches_the_result_not_the_values`, `options_limit…` |  |
| §16 | Uniformizar a curva de tons | PARCIAL (copiada da referência dentro de "look", não medida) |  | ✔ |  |  |  |  | ✔ |
| F5 | Undo, Redo, Histórico, Reset | COMPLETA | ✔ |  |  |  |  | `history_undo_redo…`; CT |  |
| F5/§22 | Antes/depois (alternar + vista dividida) | COMPLETA | ✔ |  |  |  |  | CT |  |
| F5/§22 | Copiar, colar, sincronizar ajustes | COMPLETA | ✔ |  |  |  |  | CT |  |
| F5/§22 | Versões e cópias virtuais | COMPLETA | ✔ |  |  |  | Corrigido nesta auditoria (nomes repetidos) | `history…virtual_copies` (nomes únicos depois de apagar) |  |
| F5/§22 | Snapshots | COMPLETA | ✔ |  |  |  |  | idem; CT |  |
| §22 | Restaurar original | COMPLETA (reset fica no histórico) | ✔ |  |  |  |  | edit_flow |  |
| F5 | Histograma RGB | COMPLETA | ✔ |  |  |  |  | TR `histogram_counts…` |  |
| F5 | Preview dinâmica (rápida ao arrastar, final depois) | COMPLETA | ✔ |  |  |  |  | TR `fast_and_final…`, `performance…`; CT "arrastar exposição" |  |
| §22 | Originais nunca alterados | COMPLETA (só leitura) | ✔ |  |  |  |  | edit_flow; auto_edit_flow |  |
| §22 | Sidecars XMP | NÃO IMPLEMENTADA: há sidecar JSON em `<projeto>/edits/` (decisão D-038) |  |  | ✔ |  |  | edit_flow (JSON) |  |
| §22 BD | Caminho, Hash, Metadados | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| §22 BD | Preview | PARCIAL (só `thumb_state`; os ficheiros vivem na cache) |  | ✔ |  |  |  |  | ✔ |
| §22 BD | Ajustes, Máscaras, Histórico | COMPLETA (máscaras na receita; rasters PNG em `<projeto>/masks`) | ✔ |  |  |  |  |  | ✔ |
| §22 BD | Preset | PARCIAL (só na etiqueta "preset:Nome" do histórico) |  | ✔ |  |  |  |  | ✔ |
| §22 BD | Classificação, Estado | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| §22 BD | Relação com duplicados, pessoas, momentos | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| §22 BD | Exportações realizadas | NÃO IMPLEMENTADA (só existe o journal de exportações interrompidas) |  |  | ✔ |  |  |  | ✔ |
| F6 | Fila em 2.º plano com progresso, pausa, retoma, cancelar | COMPLETA (partilhada com a sincronização) | ✔ |  |  |  |  | auto_edit_flow; CT "Editar automaticamente seleção" |  |
| F6 | Proteção contra edição concorrente | COMPLETA | ✔ |  |  |  |  |  | ✔ |

### E. Interface, galerias, álbuns, desempenho (§17–19, §23; Fases 10–12, 14, 15)

| Secção | FUNCIONALIDADE | ESTADO | COMPLETA | PARCIAL | NÃO IMPLEMENTADA | MOCK | BUG CONHECIDO | TESTADA | NÃO TESTADA |
|---|---|---|:-:|:-:|:-:|:-:|---|---|:-:|
| 17 Início | Novo projeto | COMPLETA | ✔ |  |  |  |  | CT "formulário de novo projeto valida…" |  |
| 17 Início | Abrir projeto | COMPLETA | ✔ |  |  |  |  | CT "mostra ações e projetos recentes" |  |
| 17 Início | Projetos recentes | COMPLETA | ✔ |  |  |  |  | idem; database `recent_projects_order_update_and_forget` |  |
| 17 Início | Capa do projeto | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 17 Início | Estado | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 17 Início | Progresso | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 17 Início | Número de fotografias | COMPLETA | ✔ |  |  |  |  | sim |  |
| 17 Início | Data do evento | COMPLETA | ✔ |  |  |  |  | sim |  |
| 17 Início | Última alteração | PARCIAL |  | ✔ |  |  |  |  | ✔ |
| 17 Lateral | Projeto | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 17 Lateral | Todas / Favoritas / Rejeitadas | COMPLETA | ✔ |  |  |  |  | CT "filtro Favoritas e abertura do visualizador" |  |
| 17 Lateral | Momentos | COMPLETA | ✔ |  |  |  |  | CT "momentos: filtro…arrastar" |  |
| 17 Lateral | Pessoas | COMPLETA | ✔ |  |  |  |  | CT "pesquisas: noiva…" |  |
| 17 Lateral | Sequências | COMPLETA | ✔ |  |  |  |  | CT "vista de sequências" |  |
| 17 Lateral | Seleção automática | COMPLETA | ✔ |  |  |  |  | CT "seleção assistida (Fase 3)" |  |
| 17 Lateral | Fotografias editadas | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 17 Lateral | Galeria | COMPLETA | ✔ |  |  |  |  | CT "galeria (Fase 10)" |  |
| 17 Lateral | Álbum | PARCIAL |  | ✔ |  |  |  | CT "já não há secções futuras…" |  |
| 17 Lateral | Exportação | PARCIAL |  | ✔ |  |  |  | CT "exportação (Fase 9)" |  |
| 17 Central | Grelha | COMPLETA | ✔ |  |  |  |  | CT "grelha virtualizada: 3000…" |  |
| 17 Central | Visualização individual | COMPLETA | ✔ |  |  |  |  | CT "filtro Favoritas e abertura do visualizador" |  |
| 17 Central | Comparação | COMPLETA | ✔ |  |  |  |  | CT "comparação: teclas no painel ativo…" |  |
| 17 Central | Antes e depois | COMPLETA | ✔ |  |  |  |  | CT "desfazer, antes/depois…", "vista dividida" |  |
| 17 Central | Vista de referência | PARCIAL |  | ✔ |  |  |  | CT "consistência: grupos, referência…" |  |
| 17 Central | Vista de sequência | COMPLETA | ✔ |  |  |  |  | CT "vista de sequências" |  |
| 17 Central | Modo de seleção rápida | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 17 Direito | Histograma | PARCIAL |  | ✔ |  |  |  |  | ✔ |
| 17 Direito | Pontuação automática | COMPLETA | ✔ |  |  |  |  | CT "painel de informação: explicação e pontuações" |  |
| 17 Direito | Presets, Luz, Cor, Detalhe, Geometria | COMPLETA | ✔ |  |  |  |  | CT "edição (Fase 5)", "presets…" |  |
| 17 Direito | Máscaras | COMPLETA | ✔ |  |  |  |  | CT "máscara de IA (pessoa)…" |  |
| 17 Direito | Metadados | COMPLETA | ✔ |  |  |  |  | CT "painel de informação explica…" |  |
| 17 Direito | Histórico | COMPLETA | ✔ |  |  |  |  | CT "frames…etiquetas do histórico" |  |
| 17 Inferior | Filmstrip | PARCIAL |  | ✔ |  |  |  | CT "tira de fotografias virtualizada…" |  |
| 17 Inferior | Filtros, estrelas, etiquetas de cor | COMPLETA | ✔ |  |  |  |  | parcialmente |  |
| 17 Inferior | Zoom | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 17 Inferior | Progresso de processamento | PARCIAL |  | ✔ |  |  |  | CT "barra de estado: offline…" |  |
| 17 Incluir | Tema escuro e tema claro | COMPLETA | ✔ |  |  |  |  | CT "tema escuro por omissão e claro opcional" |  |
| 17 Incluir | Atalhos de teclado | COMPLETA | ✔ |  |  |  |  | CT "atalhos classificam…" |  |
| 17 Incluir | Arrastar e largar | COMPLETA | ✔ |  |  |  |  | CT "momentos…arrastar" |  |
| 17 Incluir | Menus de contexto | COMPLETA | ✔ |  |  |  |  | CT "menu contextual: clique direito e teclado" |  |
| 17 Incluir | Ecrã completo | COMPLETA | ✔ |  |  |  |  | parcialmente (mock da janela) |  |
| 17 Incluir | Interface responsiva | COMPLETA (não confirmado visualmente) | ✔ |  |  |  |  |  | ✔ |
| 17 Incluir | Monitores de alta resolução | COMPLETA | ✔ |  |  |  |  | logic.test (zoom) |  |
| 17 Incluir | Dois monitores | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 18 | Noivos, data, local, texto introdutório | COMPLETA | ✔ |  |  |  |  | pm `wedding_gallery_end_to_end`; gallery `builds_a_complete_static_site` |  |
| 18 | Capa personalizada | PARCIAL |  | ✔ |  |  | ✔ Sim |  | ✔ |
| 18 | Fotografia de destaque (hero) | COMPLETA | ✔ |  |  |  |  | sim |  |
| 18 | Secções por momento | COMPLETA | ✔ |  |  |  |  | gallery `wedding_chapters_in_order_with_unclassified_by_time` |  |
| 18 | Grelha e mosaico | COMPLETA | ✔ |  |  |  |  | parcialmente |  |
| 18 | Slideshow | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 18 | Favoritas | COMPLETA (id e tokens estáveis: as favoritas sobrevivem a regenerar) | ✔ |  |  |  | Corrigido nesta auditoria | `wedding_gallery_end_to_end` (id e tokens estáveis) |  |
| 18 | Download individual | COMPLETA | ✔ |  |  |  |  | gallery `downloads_off_and_web_size` |  |
| 18 | Download completo (ZIP) | COMPLETA | ✔ |  |  |  |  | `builds_a_complete_static_site` |  |
| 18 | Marca de água | COMPLETA | ✔ |  |  |  |  | export-engine `text_watermark_lands_in_the_chosen_corner`; não testada na galeria |  |
| 18 | Palavra-passe / PIN | COMPLETA | ✔ |  |  |  |  | `protected_gallery_reveals_nothing_without_the_code`, crypto `seal_and_open` |  |
| 18 | Data de expiração | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 18 | Tema claro e escuro (+ Minimal, Editorial) | COMPLETA | ✔ |  |  |  |  | sim |  |
| 18 | Cores personalizadas | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 18 | Logótipo | COMPLETA | ✔ |  |  |  |  | sim |  |
| 18 | Computador, tablet e telemóvel | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 18 | Ocultar fotografias (Fase 10) | COMPLETA | ✔ |  |  |  |  | pm `wedding_gallery_end_to_end` |  |
| 18 | Pacote web estático | COMPLETA | ✔ |  |  |  |  | sim |  |
| 18 (posterior) | Publicação em S3, R2, B2, SFTP, servidor próprio | NÃO IMPLEMENTADA (só o plano incremental no núcleo; a interface diz-o em texto, sem fornecedores "Fase futura") |  |  | ✔ |  |  | `plan_diff_and_validation` (só o plano) |  |
| 18 | Publicação nunca bloqueia a edição | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 19 | Formatos 20×20 a 30×40 | COMPLETA | ✔ |  |  |  |  | album `automatic_album_tells_the_story_in_order` |  |
| 19 | Quadrado, horizontal, vertical | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 19 | Panorâmico (como formato) | PARCIAL |  | ✔ |  |  |  |  | ✔ |
| 19 | Dimensões personalizadas | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 19 | Páginas, DPI/resolução, sangria, margens, zona segura, dobra | COMPLETA | ✔ |  |  |  |  | `preflight_finds_the_problems` |  |
| 19 | Perfil ICC e espaço de cor | COMPLETA | ✔ |  |  |  |  | `compose_and_export_files` |  |
| 19 | Tipo de papel | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 19 | Laboratório (presets próprios) | COMPLETA | ✔ |  |  |  |  | CT "…preset de laboratório" |  |
| 19 | Requisitos de impressão | PARCIAL |  | ✔ |  |  |  |  | ✔ |
| 19 | Montagem automática e seleção automática | COMPLETA | ✔ |  |  |  |  | `automatic_album_tells_the_story_in_order`, pm `album_end_to_end` |  |
| 19 | Ordem cronológica e narrativa por momentos | COMPLETA | ✔ |  |  |  |  | idem |  |
| 19 | Análise de "pessoas" (Fase 11) | PARCIAL |  | ✔ |  |  |  |  | ✔ |
| 19 | Templates de 1 a 6+ | COMPLETA | ✔ |  |  |  |  | `every_count_has_layouts_that_fit_the_page` |  |
| 19 | Arrastar e largar | COMPLETA | ✔ |  |  |  |  | CT álbum "arrastar e largar" |  |
| 19 | Páginas panorâmicas e full bleed | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 19 | Troca de fotografias | PARCIAL |  | ✔ |  |  |  |  | ✔ |
| 19 | Alinhamento e distribuição | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 19 | Guias | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 19 | Grelha | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 19 | Recorte e zoom | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 19 | Aviso de resolução insuficiente | COMPLETA | ✔ |  |  |  |  | `small_photos_are_never_enlarged…` |  |
| 19 | Aviso de fotografia na dobra | COMPLETA | ✔ |  |  |  |  | `no_faces_on_the_fold` |  |
| 19 | Aviso de corte de rostos | COMPLETA (zona segura, dobra e rosto cortado pelo enquadramento) | ✔ |  |  |  | Corrigido nesta auditoria | `preflight_finds_the_problems` |  |
| 19 | Revisão página a página | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 19 | Exportação individual ou completa | PARCIAL |  | ✔ |  |  |  | `compose_and_export_files` |  |
| 19 | JPEG, TIFF, PDF de impressão, PDF de prova | COMPLETA | ✔ |  |  |  |  | CT "exportar o PDF de prova" |  |
| 23 | Processamento em segundo plano, filas, prioridades | COMPLETA | ✔ |  |  |  |  | task-queue `prioritized_items_jump_ahead…`, `priorities_order_interactive_first`, `background_waits_while_foreground_runs` |  |
| 23 | Pausar e retomar | PARCIAL |  | ✔ |  |  |  | CT "pausa, retoma e cancelar", "fila…pausa" |  |
| 23 | Cancelar | COMPLETA | ✔ |  |  |  |  | sim |  |
| 23 | Cache de thumbnails e de previews (LRU) | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 23 | Previews progressivos e RAW incorporados | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 23 | Processamento por lotes | COMPLETA | ✔ |  |  |  |  | sim |  |
| 23 | Gestão inteligente de memória | COMPLETA | ✔ |  |  |  |  | adverse `no_leaks_across_open_close_cycles` |  |
| 23 | Aceleração por GPU | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 23 | Aceleração Apple Silicon | NÃO IMPLEMENTADA (só compilação universal) |  |  | ✔ |  |  |  | ✔ |
| 23 | Processamento paralelo | COMPLETA | ✔ |  |  |  |  | `many_workers_process_each_item_once` |  |
| 23 | Escrita segura na base de dados | COMPLETA | ✔ |  |  |  |  | resilience `*` |  |
| 23 | Recuperação após falha | COMPLETA | ✔ |  |  |  |  | resilience `killed_during_import_resumes…`, `killed_during_export…` |  |
| 23 | Logs | COMPLETA | ✔ |  |  |  |  |  | ✔ |
| 23 | Monitorização de tarefas | PARCIAL |  | ✔ |  |  |  |  | ✔ |
| 23 | Limitação de recursos configurável | NÃO IMPLEMENTADA |  |  | ✔ |  |  |  | ✔ |
| 23 | Interface não bloqueia | COMPLETA | ✔ |  |  |  |  | PERFORMANCE.md:50-54 (medido) |  |
| F12 | Virtualização, LRU, pool SQLite, portão LIFO, índices, métricas pedidas | COMPLETA | ✔ |  |  |  |  | `lifo_gate_serves_the_newest_request_first` |  |
| F14 | Navegação por teclado (F6), acessibilidade, estados | COMPLETA | ✔ |  |  |  |  | CT "acessibilidade (Fase 14)" (axe) |  |
| F15 | E2E de casamento, condições adversas, RELEASE_CHECKLIST | COMPLETA | ✔ |  |  |  |  | `real_wedding_end_to_end`, adverse `export_to_full_disk`, `permissions`, `read_only_volumes` |  |
