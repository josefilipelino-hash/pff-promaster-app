# Arquitetura — Momento Studio

> Documento de referência técnica. Qualquer alteração à arquitetura principal tem de ser
> registada em §21 (Registo de decisões) com a necessidade e o impacto.
>
> Estado das secções: **[implementado]** existe código testado; **[definido]** decisão
> tomada, sem código; **[a validar]** decisão dependente de prova de conceito.

Versão do documento: Fase 2 · 2026-11-08

---

## 0. Visão geral

```
┌──────────────────────────── Processo da aplicação (Tauri 2) ────────────────────────────┐
│                                                                                         │
│  WebView do sistema (WKWebView no macOS · WebView2 no Windows · WebKitGTK no Linux)     │
│  ┌───────────────────────────────────────────────────────────────────────────────────┐  │
│  │ React 19 + TypeScript                                                             │  │
│  │ Ecrãs · grelha virtualizada · loupe · painéis · editor de álbum · i18n (i18next)  │  │
│  └──────────┬─────────────────────────────▲─────────────────────────▲─────────────────┘  │
│   comandos  │ invoke() JSON               │ eventos (progresso)     │ momento://… bytes  │
│             ▼                             │                         │ (thumbs/previews)  │
│  ┌─────────────────────────────────────────────────────────────────────────────────────┐ │
│  │ apps/desktop/src-tauri — ponte fina: comandos, eventos, protocolo de imagens         │ │
│  ├─────────────────────────────────────────────────────────────────────────────────────┤ │
│  │ Núcleo Rust (packages/*)                                                            │ │
│  │  project-management · database · task-queue · import-engine · metadata             │ │
│  │  image-codecs ─┬─ adaptador image-rs / zune / turbojpeg                             │ │
│  │                ├─ adaptador raw-engine (LibRaw, FFI C++)                            │ │
│  │                ├─ adaptador ImageIO (macOS) · libheif (Windows/Linux)               │ │
│  │                └─ …                                                                 │ │
│  │  image-processing (CPU f32 → GPU wgpu) · color-management (LittleCMS 2)             │ │
│  │  ai-analysis · face-analysis (ONNX Runtime · Core ML/Vision no macOS)              │ │
│  │  presets · export-engine · gallery · album                                         │ │
│  └─────────────────────────────────────────────────────────────────────────────────────┘ │
│        │ SQLite (catálogo por projeto)     │ cache de previews (ficheiros)               │
└────────┼───────────────────────────────────┼─────────────────────────────────────────────┘
         ▼                                   ▼
   <projeto>.momento/catalog.db       <projeto>.momento/cache/…        Originais: intocados
```

**Princípios invioláveis**

1. Os originais nunca são escritos, movidos ou apagados pela aplicação sem confirmação explícita.
2. A interface nunca descodifica nem processa imagens: só recebe bytes já prontos.
3. Toda a edição é uma receita de parâmetros; previews e exportações são derivadas.
4. Todo o trabalho pesado corre fora da thread da interface, numa fila persistente.
5. Um ficheiro inválido nunca bloqueia nem termina a aplicação.
6. Nada sai do computador sem autorização explícita (ver `PRIVACY.md`).

---

## 1. Arquitetura desktop

### 1.1 Comparação técnica

| Critério | **Tauri 2 + React + Rust** | Electron + React | SwiftUI (nativo) | Flutter Desktop |
|---|---|---|---|---|
| Linguagem do núcleo | Rust nativo, no mesmo processo | Node.js; processamento real exigiria addons N-API em C++/Rust | Swift nativo | Dart; C/C++ via `dart:ffi` |
| Memória em repouso (ordem de grandeza) | ~80–150 MB | ~250–500 MB (Chromium incluído) | ~60–120 MB | ~150–250 MB |
| Tamanho do instalador | ~10–20 MB + codecs | ~100–150 MB | ~10–20 MB | ~30–60 MB |
| macOS Apple Silicon / Intel | Sim / Sim (binário universal) | Sim / Sim | Sim / Sim | Sim / Sim |
| Windows 10/11 | Sim (WebView2) | Sim | **Não** | Sim |
| Interface rica e rápida de construir | Alta (ecossistema React) | Alta | Média | Média |
| Grelha de 15 000 itens | Boa com virtualização | Boa com virtualização | Excelente | Boa |
| FFI com LibRaw / LittleCMS (C/C++) | Direta (`cc`, `bindgen`) | Indireta (addon nativo) | Direta | Possível, menos madura |
| ONNX Runtime | `ort` (Rust), EPs CoreML/DirectML/CUDA | `onnxruntime-node` | Via C API ou Core ML nativo | Pacotes comunitários |
| GPU de processamento | `wgpu` → Metal/DX12/Vulkan | WebGPU no renderer (limitado a contexto web) | Metal | Limitada |
| Cor e HDR na pré-visualização | **Risco médio**: WebView gere ICC; sem garantia de 10 bits/EDR | Chromium gere ICC; HDR parcial | **Melhor** (ColorSync, EDR) | Gestão de cor limitada |
| Segurança | Comandos e permissões explícitos (capabilities) | Superfície maior (Node + Chromium) | Sandbox nativa | Boa |
| Testes E2E da interface | WebDriver (`tauri-driver`) só Linux/Windows | Playwright | XCUITest | integration_test |

### 1.2 Decisão

**Tauri 2 (estável, 2.12.x) + React 19 + TypeScript + Rust.** **[implementado]**

- O núcleo em Rust corre no mesmo processo, sem serialização entre linguagens no caminho
  crítico, com segurança de memória (um ficheiro malformado não corrompe a memória do
  processo, exceto em código C/C++ via FFI — ver §6.4).
- Mantém o Windows aberto (SwiftUI exclui-o).
- Electron obrigaria a escrever o mesmo núcleo nativo, com mais memória e mais complexidade.
- Flutter tem o ecossistema mais fraco para imagem/cor/IA no desktop.

**Versão:** o Tauri 3 existe apenas em alpha (3.0.0-alpha.6 à data). Fica fixado o
Tauri 2.12.5 (`=2.12.5` no `Cargo.toml`). A migração para o 3 só será avaliada quando for
estável e trouxer benefício concreto.

### 1.3 Ponto fraco conhecido e mitigação

A WebView não garante saída de 10 bits nem HDR/EDR. Mitigação por camadas:

1. **[definido]** As previews de edição são renderizadas pelo núcleo já convertidas para o
   perfil do monitor (ou Display P3) e codificadas com o perfil ICC embutido. O WKWebView
   no macOS respeita perfis ICC embutidos em imagens.
2. **[a validar, Fase 4]** Medir com um alvo de teste (gradientes 16 bits, patches P3) se
   há banding ou desvio visível.
3. **Plano B [definido, não implementado]:** uma superfície nativa `wgpu`/Metal sobreposta
   à área de visualização (janela filha), mantendo React para todo o resto. A arquitetura
   permite-o porque a renderização já está no núcleo; só muda o destino dos píxeis.

---

## 2. Frontend **[implementado: esqueleto]**

| Aspeto | Escolha |
|---|---|
| Framework | React 19.3, TypeScript 5.9 (strict, `noUncheckedIndexedAccess`) |
| Bundler | Vite 7 (porta fixa 1420, alvo `safari15`, compatível com WKWebView do macOS 12+) |
| i18n | i18next + react-i18next. **Nenhum texto no código**; ficheiros em `packages/localization/locales/*.json`; `pt-PT` é a referência; um teste garante a igualdade de chaves entre idiomas |
| Estado | Zustand (estado da interface) + TanStack Query (cache de pedidos ao núcleo) — Fase 1 |
| Grelha | TanStack Virtual — só os itens visíveis existem no DOM — Fase 1 |
| Componentes acessíveis | Radix UI Primitives (MIT), adicionados apenas quando necessários |
| Testes | Vitest + Testing Library (jsdom) |
| Tipos partilhados | `ts-rs` gera interfaces TS a partir dos tipos Rust (Fase 1), eliminando divergências manuais (hoje `AppInfo` é espelhado à mão, com teste) |

TypeScript 7 (compilador nativo) já existe, mas ficou fixado o 5.9 por compatibilidade
com as ferramentas (Vite, Vitest, futuros linters). A atualização é independente da
arquitetura.

Pacotes: `apps/desktop` (aplicação), `packages/ui` (componentes partilhados e tokens de
tema neutros — cinzentos sem dominante, para não influenciar a perceção de cor),
`packages/localization`.

---

## 3. Backend / motor de processamento

Workspace Cargo com um crate por responsabilidade. Regras:

- `core-types` não depende de nenhum crate interno (tipos, IDs, erros, receita).
- O crate da aplicação (`apps/desktop/src-tauri`) é uma ponte fina: valida argumentos,
  chama o núcleo, converte erros em `ErrorPayload { code, detail }`, onde `code` é uma
  chave de tradução.
- Concorrência: **tokio** para I/O assíncrono e coordenação; **rayon** para CPU
  intensiva. Nunca se bloqueia o runtime tokio com descodificação.
- Registos: `tracing` com ficheiro rotativo na pasta de logs da aplicação. Nunca se
  registam píxeis, thumbnails ou dados faciais.

| Crate | Responsabilidade | Fase |
|---|---|---|
| core-types | IDs tipados, erros, `EditRecipe` | 0 **[implementado]** |
| database | Abertura, pragmas, migrações, repositórios | 0 **[implementado: base]** |
| task-queue | `WorkQueue` com prioridade + modelo de estados | 1 **[implementado]** |
| image-codecs | Deteção por assinatura, adaptadores, registo; JPEG/PNG/TIFF | 1 **[implementado]** |
| project-management | Projetos, pastas, cópias de segurança, miniaturas em segundo plano, previews, sessão | 1 **[implementado]** |
| import-engine | Importação, duplicados, cópia verificada, retoma | 1 **[implementado]** |
| metadata | EXIF (Fase 1 **[implementado]**); IPTC/XMP (Fase 2) | 1–2 |
| image-processing | Miniaturas/previews, orientação, escrita atómica (Fase 1 **[implementado]**); revelação (Fase 4) | 1–4 |
| test-fixtures | Gerador de imagens sintéticas com EXIF para testes | 1 **[implementado]** |
| raw-engine | LibRaw | 2 |
| color-management | LittleCMS | 2 e 6 |
| image-processing | Pipeline de revelação | 4 |
| ai-analysis / face-analysis | Análise técnica e IA | 3 e 5 |
| presets | Presets | 4 |
| export-engine | Exportação | 6 |
| gallery | Galeria estática | 7 |
| album | Álbuns | 8 |

---

## 4. Base de dados **[implementado: base]**

### 4.1 Decisões

- **SQLite** via `rusqlite` 0.40 com `bundled` (SQLite compilado do código-fonte, versão
  idêntica em todas as plataformas). **Apenas um crate de acesso a SQLite** no workspace:
  `sqlx`/`diesel` ficam proibidos, porque crates diferentes ligam `libsqlite3-sys` em
  versões incompatíveis.
- **Um catálogo por projeto**: `<projeto>.momento/catalog.db`. Torna os projetos
  portáteis (arquivar, mover para outro computador) e isola a corrupção.
- **Biblioteca global** pequena (`library.db` na pasta de suporte da aplicação): projetos
  recentes, caminhos, preferências, presets globais.
- Pragmas: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `busy_timeout=5000`.
- **Escritor único**: uma thread dedicada recebe as escritas em lote (transações de
  centenas de linhas); as leituras usam ligações próprias em paralelo (WAL permite).
- Migrações sequenciais por `PRAGMA user_version`, cada uma numa transação. **Uma
  migração publicada nunca é editada.** Um catálogo de versão superior é recusado
  (`CatalogTooNew`) em vez de corrompido.
- **Backup do catálogo** com a API de backup online do SQLite (`rusqlite::backup`),
  rotativo, sem parar a aplicação (Fase 1).

### 4.2 Incompatibilidade crítica

O modo WAL **não funciona corretamente em sistemas de ficheiros de rede** (SMB, NFS, AFP).
O catálogo tem de estar num disco local (interno ou externo USB/Thunderbolt). Os originais
podem estar em qualquer lado, incluindo NAS. A aplicação deve detetar e avisar (Fase 1).

### 4.3 Esquema alvo (introduzido por migrações nas fases indicadas)

| Tabela | Conteúdo principal | Fase |
|---|---|---|
| `catalog_info` | chave/valor do catálogo | 0 ✅ |
| `project` | nome, cliente/noivos, tipo de evento, data, local, equipa (JSON), estado, pastas, preset principal, estilo, n.º esperado de finais, prazos, estado da galeria/álbum, notas | 1 |
| `source_root` | pasta raiz de originais, UUID do volume, rótulo, online | 1 |
| `asset` | root_id, caminho relativo, tamanho, mtime, `blake3`, formato, dimensões, orientação, bits, espaço de cor, ICC, alfa, hora original, hora corrigida, câmara, objetiva, exposição, GPS, estado (ok/em falta/corrompido/não suportado), `pair_id` RAW+JPEG | 1 |
| `variant` | asset_id, nome, `is_master` (cópias virtuais) | 1 |
| `rating` | variant_id, estrelas, cor, flag, protegida | 1 |
| `import_session`, `import_item` | estado por ficheiro (retoma) | 1 |
| `preview` | variant, tipo, origem, `recipe_hash`, caminho | 1 |
| `job` | fila persistente | 1 |
| `camera`, `lens` | inclui `time_offset_ms` por corpo | 2 |
| `metadata_blob` | EXIF/IPTC/XMP brutos | 2 |
| `analysis`, `phash`, `embedding` | métricas e vetores | 3 |
| `group`, `group_member`, `recommendation` (com razões JSON) | duplicados, sequências, recomendações | 3 |
| `edit_state`, `edit_history`, `snapshot` | edição não destrutiva | 4 |
| `preset`, `preset_folder` | presets | 4 |
| `person`, `face`, `moment`, `moment_asset` | pessoas e momentos | 5 |
| `export_recipe`, `export_job`, `export_item` | exportação | 6 |
| `gallery` | galerias | 7 |
| `album`, `album_page`, `album_slot` | álbuns | 8 |
| `log_event` | eventos técnicos (sem conteúdo de imagens) | 1 |

Volume esperado para 15 000 fotografias: ~15 000 assets + ~50 000 linhas de análise e
grupos + vetores (15 000 × 512 floats ≈ 30 MB). Catálogo estimado: 50–150 MB. Bem dentro
das capacidades do SQLite.

---

## 5. Pipeline de imagem **[implementado na Fase 5 — ver D-034 a D-039; automático na Fase 6 — D-040 a D-045; presets e consistência na Fase 7 — D-046 a D-051]**

### 5.0 O que está implementado (`momento-image-processing::develop`)

```
fonte em memória (Rec. 2020 linear f32, orientada, pirâmide ½ ¼ …)
  RAW: LibRaw linear 16 bits Rec. 2020 (meia resolução para editar; total para 100 %/exportar)
  outros: sRGB 8 bits → linear → Rec. 2020 (lado maior ≤ 3072 para editar)
        │
        ├─ guia 384 px (mesma imagem): luminância local (sombras/realces), dehaze
        │  (canal escuro), base da clareza — igual para render rápido e final
        ▼
por píxel (rayon): geometria + objetiva numa reamostragem (recorte, endireitar, perspetiva,
  90°/espelho, distorção, aberração cromática) → vinheta da objetiva → balanço de brancos
  (Planck) × exposição → dehaze → sombras/realces locais → curva sRGB →
  pretos/brancos/contraste/curva paramétrica/curva de pontos/curvas por canal (tabelas) →
  clareza → vibração/saturação/HSL
        ▼
filtros à resolução do render: textura · ruído de cor · ruído de luminância · nitidez
  (raio definido em píxeis da resolução total; ruído/nitidez só no render final)
        ▼
vinheta (pós-recorte) · grão (grelha da resolução total) → Rec. 2020 → sRGB 8 bits RGBA
  + histograma RGB
```

Receita neutra = original (±1 nível, testado). O texto seguinte (§5.1) é o plano da Fase 0;
LittleCMS e saída para o perfil do monitor ficam para a exportação/gestão de cor (Fase 6).

### 5.1 Plano original (Fase 0)


```
Ficheiro ─► detect() ─► CodecRegistry ─► adaptador
                                           │
                 ┌─────────────────────────┴─────────────────────────┐
                 │ RAW: dados do sensor + matriz da câmara           │ Raster: RGB + ICC
                 ▼                                                   ▼
        [Fase cena-referida — linear, f32, espaço de trabalho de gama larga]
        balanço de brancos · exposição · recuperação de realces · denoise ·
        aberração cromática · correção de lente · geometria
                 ▼
        [Fase de tons e cor — f32]
        realces/sombras/brancos/pretos · curvas · HSL · color grading · máscaras ·
        efeitos (textura, claridade, desembaçar, vinheta, grão)
                 ▼
        [Transformação de saída — LittleCMS]
        espaço de trabalho → perfil do monitor (preview) | perfil de exportação
        nitidez de saída · dithering · quantização (8/10/12/16 bits, 16f, 32f)
                 ▼
        Codificador (preview JPEG/WebP com ICC | receita de exportação)
```

- **Espaço de trabalho:** RGB linear com primárias **Rec.2020**, em `f32`. Justificação:
  gama larga o suficiente para cores de câmara saturadas, primárias reais (ao contrário do
  ProPhoto, que tem primárias imaginárias e torna mais difícil a gestão de cores fora do
  gamut), e coincide com espaços de saída HDR futuros.
- **Um único pipeline** para preview e exportação: as previews são a mesma função com
  menor resolução de entrada (escala antes das operações dependentes de vizinhança, com
  parâmetros ajustados à escala, como faz o Lightroom).
- **Representação:** buffer planar ou entrelaçado `f32` em tiles (ex.: 256×256) para
  paralelizar com rayon e, na Fase 9, enviar para GPU.
- **GPU (Fase 9, [a validar]):** as mesmas operações reescritas como shaders WGSL em
  `wgpu`; a implementação CPU fica como referência e para testes de regressão (comparação
  por tolerância).

---

## 6. Pipeline RAW **[implementado na Fase 2 — ver D-016, D-017, D-021]**

Implementação atual (resumo; o desenho original segue abaixo):
- `packages/raw-engine/libraw-sys`: LibRaw 0.22.3 (submódulo) + `shim/momento_libraw.{h,cpp}`
  compilados com `cc` (`USE_ZLIB`, `USE_X3FTOOLS`, `LIBRAW_NODLL`, `_USE_MATH_DEFINES`);
  verificado em Linux (gcc) e por compilação cruzada para Windows (MinGW, com executável
  ligado). macOS (clang) é verificado no CI.
- `packages/raw-engine`: `RawFile` (abertura segura, metadados, preview embutida, revelação
  sRGB 8 bits sem rotação) e `RawCodec` (adaptador do registo).
- Previews embutidas já rodadas pela câmara (ex.: Nikon D70, Kodak DCS) são detetadas pela
  proporção e normalizadas para a orientação do sensor.
- Ficheiros cujo descodificador a LibRaw marca como não suportado (DNG com JPEG XL, NEF
  High Efficiency, GoPro VC5) são importados com **suporte parcial** (só preview embutida).

### 6.1 Biblioteca

**LibRaw** (≥ 0.21; suporta CR3, NEF de alta eficiência, RAF X-Trans, DNG 1.6 e centenas
de câmaras). Licença dupla **LGPL-2.1 ou CDDL-1.0**: optamos pela **CDDL-1.0**, que
permite ligação estática com copyleft limitado aos ficheiros da própria LibRaw (sem
obrigação de ligação dinâmica substituível exigida pela LGPL).

**Integração:** crate próprio `raw-engine` com um submódulo `-sys` que compila a LibRaw
a partir do código-fonte oficial (vendorizado como submódulo Git, versão fixada) com o
crate `cc`, e bindings gerados com `bindgen`. Os crates `libraw-sys`/`rsraw` existentes
foram avaliados mas são pouco mantidos; preferimos controlar a versão e as flags de
compilação (ex.: suporte a DNG com compressão JPEG/deflate, que exige zlib e libjpeg).

Alternativa avaliada: `rawler` (Rust puro, projeto dnglab, LGPL-2.1) — menor cobertura de
câmaras do que a LibRaw. Fica como possível segundo adaptador.

### 6.2 Fluxo

1. **Importação:** `libraw_open_file` + `libraw_unpack_thumb` → JPEG embutido (tipicamente
   1600–6000 px) → thumbnail/preview imediata. Não se descodifica o sensor nesta fase.
2. **Metadados:** LibRaw fornece marca, modelo, ISO, exposição, objetiva, hora e
   orientação de forma uniforme para todos os RAW.
3. **Revelação:** `libraw_unpack` → dados do sensor → demosaico (AHD/DHT para Bayer,
   3-pass para X-Trans) → matriz câmara→XYZ (coeficientes da LibRaw/Adobe) → espaço de
   trabalho linear. O balanço de brancos passa a ser um parâmetro da receita.
4. **Fase 4+:** perfis DCP (parser próprio do formato DNG Camera Profile), correções de
   lente embutidas (opcodes DNG, metadados Sony/Fuji) e demosaico em GPU (Fase 9).

### 6.3 Desempenho esperado [a validar com ficheiros reais na Fase 2]

- Extração de preview embutida: ~10–40 ms por ficheiro → 5 000 RAW em ~1 minuto num Mac
  de 8+ núcleos.
- Revelação completa de 45 MP em CPU: ~1–2 s; por isso nunca é feita na importação.

### 6.4 Isolamento de falhas

Código C/C++ pode falhar com ficheiros malformados de formas que o Rust não consegue
apanhar (segfault). Estratégia: descodificação de ficheiros de origem desconhecida com
LibRaw corre **num processo auxiliar** (`momento-worker`, o mesmo binário com outro modo
de arranque) quando executada em massa (importação). Uma falha termina só o auxiliar; o
ficheiro é marcado como corrompido e o trabalho continua. **[a validar na Fase 2]**: medir
o custo e decidir entre processo auxiliar permanente (pool) ou só para a primeira leitura.

---

## 7. Gestão de cor **[definido]**

- **LittleCMS 2** (crate `lcms2`, MIT, que compila a lcms2 em C, também MIT): leitura de
  perfis ICC, conversões, rendering intent (perceptual, relativo colorimétrico com
  compensação do ponto negro, saturação, absoluto), soft proofing e aviso de gamut.
- **Perfil do monitor:** no macOS via ColorSync/CoreGraphics (`CGDisplayCopyColorSpace`
  através dos crates `objc2-*`); no Windows via WCS (Fase 9). O perfil é reavaliado
  quando a janela muda de ecrã (suporte a dois monitores).
- **Concorrência:** as transformações LittleCMS são criadas uma vez por combinação
  (origem, destino, intent) e guardadas em cache; usam-se contextos por thread
  (`ThreadContext`) para segurança entre threads.
- **OpenColorIO** (BSD) fica para ACES/ACEScg na exportação, apenas se for necessário. Não
  é dependência inicial.
- **Consistência:** preview, exportação e prova de impressão usam a mesma transformação de
  saída; só muda o perfil de destino.
- Perfis embutidos na aplicação: sRGB, Display P3, Adobe RGB (compatível), ProPhoto,
  Rec.709, Rec.2020 — gerados a partir das primárias e curvas publicadas (sem copiar
  ficheiros ICC de terceiros com licença incerta).

---

## 8. Cache e previews **[definido]**

Estrutura em `<projeto>.momento/cache/` (pode ser apagada; é sempre regenerável):

| Nível | Tamanho | Formato | Origem | Uso |
|---|---|---|---|---|
| `thumb` | lado maior 320 px | JPEG q80 (sRGB) | preview embutida ou original | grelha |
| `grid` | 640 px | JPEG q85 | idem | grelha em ecrãs Retina/zoom |
| `preview` | 2560 px | JPEG q90 + ICC Display P3 | renderização com receita | loupe, comparação |
| `full` | 1:1, por tiles | em memória (LRU) | renderização | zoom 100% |

- Caminho: `cache/<nível>/<2 primeiros hex>/<asset_id>-<recipe_hash>.jpg` (sharding
  evita pastas com 15 000 entradas).
- Invalidação: `recipe_hash` = hash BLAKE3 da receita normalizada + versão do pipeline.
  Mudar a receita gera uma chave nova; as antigas são limpas por recolha periódica.
- **Entrega à interface:** protocolo URI próprio registado com
  `register_asynchronous_uri_scheme_protocol` do Tauri 2 (`momento://thumb/<id>`), com
  cabeçalhos de cache. Evita base64 e JSON para imagens. Nota de compatibilidade: no
  Windows (WebView2) os protocolos próprios são servidos como
  `http://momento.localhost/…`; o frontend obtém o URL base a partir de uma função única.
- **Prioridade:** o frontend informa o núcleo do intervalo visível na grelha; esses
  thumbnails sobem para prioridade `Visible`.
- **Limite configurável** de tamanho de cache por projeto, com remoção LRU dos níveis
  `preview`.

---

## 9. Filas de processamento **[implementado na Fase 1, ver D-012]**

Implementação atual:
- **Importação:** pool rayon (metade dos núcleos) processa ficheiros; um único escritor grava
  em lotes de até 32 itens ou a cada 250 ms. Cancelar termina a sessão; fechar a aplicação
  **suspende-a** e ela é retomada na próxima abertura.
- **Miniaturas:** `WorkQueue` partilhada por N−1 threads; a grelha envia os IDs visíveis
  (`prioritize_thumbnails`) que passam à frente; uma thread grava os resultados em lote e
  emite eventos para a interface a cada ~150 ms. Pânicos num descodificador são apanhados
  (`catch_unwind`) e a miniatura fica marcada como falhada.

Desenho original (mantido como objetivo para trabalhos futuros sem estado de domínio):

- Tabela `job` no catálogo (persistente): tipo, payload JSON, prioridade, estado,
  progresso, tentativas, último erro, timestamps.
- Prioridades: `Interactive` > `Visible` > `Normal` > `Background`.
- Estados e transições validadas: `Queued → Running → Completed | Failed | Cancelled`,
  com `Paused`. **Recuperação:** no arranque, tarefas `Running` voltam a `Queued`
  (implementado e testado em `task-queue`).
- Executor: um despachante tokio que retira tarefas por prioridade e envia trabalho CPU
  para um pool rayon dedicado, com número de threads configurável (limitação de recursos).
- Cancelamento cooperativo (token verificado entre passos) e pausa global.
- Tarefas são **idempotentes**: repetir uma tarefa interrompida produz o mesmo resultado
  (ex.: escrita atómica por ficheiro temporário + `rename`).
- Progresso emitido como eventos Tauri agregados (no máximo ~10/s) para não saturar a
  interface.

---

## 10. Sistema de plugins / codecs **[implementado — Fase 2: 7 adaptadores]**

Adaptadores registados em `momento_project_management::codecs::full_registry()`: `raster`
(image), `jxl-oxide`, `psd` (leitor próprio), `openjp2`, `libraw`, `imageio` (macOS), `wic`
(Windows). Cada operação de um adaptador corre protegida por `catch_unwind`; um erro de um
adaptador faz o registo tentar o seguinte por prioridade.

- `detect(head, extension)` identifica o formato pelo **conteúdo** (assinaturas). A
  extensão só desambigua dentro do contentor TIFF (NEF/ARW/DNG/PEF/…), devolvendo
  `Confidence::SignatureAndExtension`; o `raw-engine` confirma depois pelas tags TIFF.
  Formatos sem assinatura (TGA) são marcados `ExtensionOnly`. Testes incluem 5 000
  entradas pseudoaleatórias para garantir que nunca entra em pânico.
- Contrato `ImageCodec` (`id`, `priority`, `capabilities(format)`, `probe`; descodificação
  na Fase 1) e `CodecRegistry`, que devolve candidatos por prioridade para permitir
  recurso (ex.: HEIC: ImageIO → libheif).
- **Pacotes de codecs:** na primeira versão, adaptadores são compilados no binário e
  ativados por *features* Cargo. Plugins dinâmicos (bibliotecas carregadas em tempo de
  execução com ABI C versionada) ficam para depois da Fase 9, apenas se houver
  necessidade real — evita complexidade e problemas de assinatura de código no macOS.

Adaptadores planeados:

| Adaptador | Formatos | Licença | Fase |
|---|---|---|---|
| image-rs (+ zune-jpeg, png, tiff, gif, webp, exr…) | JPEG, PNG, APNG, TIFF, BigTIFF, BMP, GIF, WebP, ICO, PNM/PAM, TGA, HDR, OpenEXR, QOI | MIT/Apache | 1 |
| turbojpeg (libjpeg-turbo) | JPEG rápido (descodificação escalada para thumbnails) | BSD/IJG/zlib | 1 (se necessário) |
| raw-engine (LibRaw) | RAW | CDDL-1.0 | 2 |
| ImageIO (macOS, via `objc2-image-io`) | HEIC, HEIF, AVIF, JPEG 2000, e mais | sistema | 2 |
| libheif (+ libde265, dav1d) | HEIC/HEIF/AVIF no Windows/Linux | LGPL-3 / LGPL / BSD | 9 |
| jxl-oxide | JPEG XL (leitura) | MIT/Apache | 2 |
| psd | PSD/PSB (imagem composta) | MIT | 2 |
| resvg | SVG (marcas de água, templates) | Apache/MIT | 6 |
| PDF (pdfium) | importação de grafismos em PDF | BSD-3/Apache | 8 (se necessário) |

---

## 11. IA local e seleção assistida **[implementado na Fase 3 — ver D-024 a D-028]**

### 11.0 O que está implementado (Fase 3)

```
fotografia ──► imagem de trabalho ≤ 2048 px (preview em cache ou original reduzido, orientada)
   │
   ├─ métricas clássicas (momento-ai-analysis::metrics): nitidez global/pico/centro
   │  (variância do Laplaciano, grelha 6×4), tremido (coerência do tensor de estrutura),
   │  exposição e clipping, ruído (Immerkær), inclinação (histograma de orientações)
   ├─ pHash + dHash
   └─ modelos locais (momento-face-analysis, tract-onnx, Rust puro)
        ├─ BlazeFace (multiescala: imagem + mosaicos 2×2 e 3×3) → rostos pequenos
        ├─ malha de 478 pontos → presença, olhos (nitidez da zona dos olhos), rosto cortado
        ├─ 52 blendshapes → olhos fechados, sorriso, boca aberta
        └─ MobileNetV3 → embedding visual (1024-D)
   ▼
catálogo: analysis · face · embedding   (estado por fotografia em asset.analysis_state)
   ▼
motor de seleção (momento-ai-analysis::selection), recalculado em ms a partir do catálogo
   ├─ grupos: RAW+JPEG (mesmo nome), duplicados (pHash ≤ 4 + embedding ≥ 0,97 + cópia),
   │  sequências (tempo + semelhança visual + disposição dos rostos + várias câmaras)
   ├─ pontuações individuais: foco, tremido, exposição, ruído, olhos, expressão,
   │  composição, técnica, pessoas, global (foco e ruído relativos dentro do grupo)
   ├─ piscadelas relativas à própria pessoa na sequência
   ├─ etiquetas MELHOR / ALTERNATIVA / REVER / REJEITAR SUGERIDO / DUPLICADA por modo
   │  (conservador, equilibrado, rigoroso) e objetivo de entrega aproximado
   └─ razões estruturadas (código + valor) → frases traduzidas na interface
   ▼
catálogo: photo_group · recommendation   (nunca altera classificações nem ficheiros)
```

O texto seguinte (§11.1) é o plano original da Fase 0; as diferenças estão nas decisões
D-024 a D-028.

### 11.1 Plano original (Fase 0)

- **Runtime:** ONNX Runtime via crate `ort` 2.0 (MIT/Apache; ORT 1.31). Execution
  providers: **CoreML** (macOS, usa Neural Engine/GPU quando os operadores são suportados),
  **DirectML** (Windows), **CUDA** (opcional), CPU sempre como recurso.
- **Core ML e Vision diretos (macOS):** para tarefas onde a Apple oferece modelos de
  sistema (landmarks e qualidade de captura facial com `VNDetectFaceCaptureQualityRequest`,
  segmentação de pessoas), via `objc2-vision`/`objc2-core-ml`. Implementado como backend
  alternativo atrás do mesmo trait, para não perder o Windows.
- **Distribuição de modelos:** nunca no Git. `models/manifest.json` lista URL, SHA-256,
  tamanho e licença; download explícito pelo utilizador; verificação de hash.
- **Abordagem em camadas:**
  1. Fase 3 começa **sem modelos**: nitidez (variância do Laplaciano por zonas), ruído,
     histograma/clipping, dominante de cor, pHash/dHash, intervalos temporais. São rápidas,
     explicáveis e determinísticas.
  2. Modelos pré-treinados (sem treino de raiz):

| Tarefa | Candidato predefinido | Licença | Notas |
|---|---|---|---|
| Deteção de rostos | YuNet (OpenCV Zoo) | MIT | muito leve |
| Reconhecimento facial | SFace (OpenCV Zoo) | Apache-2.0 | embeddings 128-D |
| Olhos fechados / expressão | MediaPipe Face Landmarker (malha 478 pontos → rácio de abertura do olho) | Apache-2.0 | conversão para ONNX a validar |
| Embeddings, momentos (zero-shot), semelhança | OpenCLIP ViT-B/32 | MIT (código); pesos por modelo, a verificar | |
| Estética | LAION Aesthetic Predictor (cabeça sobre CLIP) | MIT | |
| Objetos | RT-DETR | Apache-2.0 | YOLO da Ultralytics excluído (AGPL-3.0) |
| Segmentação | MediaPipe Selfie Segmentation, MobileSAM | Apache-2.0 | |
| Denoise | SCUNet / NAFNet | Apache-2.0 / MIT | em RGB linear após demosaico |
| Upscaling | Real-ESRGAN | BSD-3 | |
| Alternativa facial de maior precisão | InsightFace (SCRFD/ArcFace) | código MIT, **pesos só não comerciais** | opcional, nunca predefinido |

- Cada recomendação guarda **razões estruturadas** (JSON: métrica, valor, comparação com
  o grupo) traduzidas na interface para frases do tipo “Recomendada porque…”.
- **Incompatibilidade a verificar [a validar, Fase 3]:** binários pré-compilados do ONNX
  Runtime para macOS Intel (x86_64). Se não existirem para a versão escolhida, no Intel
  usa-se compilação própria do ORT ou só CPU; a funcionalidade continua disponível.
- **Incompatibilidade de build:** por omissão, `ort` descarrega binários do ORT durante a
  compilação. Para builds reproduzíveis, os binários serão fixados por versão e
  verificados (ou compilados) no CI.

---

## 12. Edição não destrutiva **[envelope implementado]**

- **Receita** (`EditRecipe` em `core-types`): JSON versionado (`v`), blocos por área
  (`light`, `color`, `detail`, `effects`, `geometry`, `masks`, `retouch`, `preset`,
  `auto`). Campos desconhecidos de versões futuras são preservados (testado), para que uma
  versão antiga não destrua edições novas.
- **Valores neutros por omissão**: receita vazia = original.
- **Histórico:** `edit_history` regista operações (`{bloco, antes, depois, etiqueta}`) com
  sequência; desfazer/refazer percorre a sequência; `snapshot` guarda receitas completas
  com nome.
- **Cópias virtuais:** a receita e a classificação pertencem a `variant`, não a `asset`.
- **Copiar/colar/sincronizar:** operam sobre um subconjunto de caminhos da receita
  (ex.: só `light.exposure` e `color.wb`).
- **Máscaras:** cada máscara é geométrica (pincel como lista de traços vetoriais,
  gradientes) ou gerada por IA (referência a um mapa guardado na cache + parâmetros que o
  regeneram). Nunca se guardam bitmaps no catálogo.
- **Sidecars XMP:** exportação opcional de um subconjunto mapeável para `crs:` (Adobe
  Camera Raw) — aproximação documentada, não equivalência. O formato próprio (JSON) é a
  fonte de verdade sem perdas.
- **Versão do pipeline** incluída no `recipe_hash`: uma alteração ao algoritmo invalida as
  previews antigas, sem alterar receitas.

---

## 13. Presets **[implementado na Fase 7 — D-046 a D-049]**

- Um preset é uma **receita parcial** + metadados (nome, pasta, descrição, miniatura de
  referência, versão do esquema, blocos incluídos).
- **Aplicação com intensidade**: interpolação por parâmetro entre o estado atual e o do
  preset (ex.: 80%), com regras próprias para parâmetros não lineares (temperatura
  interpolada em mireds, curvas por ponto).
- **Combinação com correção automática**: a receita guarda separadamente `auto`
  (correção por fotografia) e `preset` (estilo); o resultado final é `auto ⊕ preset ⊕
  ajustes manuais`, por esta ordem. Assim o mesmo preset dá resultados coerentes em
  fotografias com condições diferentes.
- **Formatos:** próprio (`.momentopreset`, JSON), importação de XMP Lightroom (mapeamento
  parcial e documentado), LUT `.cube` (1D/3D) aplicada como bloco `lut` com intensidade,
  perfis DCP (Fase 4+).
- Presets base (Natural Wedding, Clean Editorial, Fine Art, …) definidos como ficheiros
  JSON versionados no repositório, criados por nós (sem cópia de presets comerciais).

---

## 14. Motor de exportação **[definido]**

- **Receita de exportação** = formato + profundidade + espaço de cor/perfil + intent +
  dimensionamento + nitidez de saída + nome (modelo com tokens) + pasta + metadados (manter,
  remover GPS, remover privados, copyright) + marca de água + política de conflitos.
- **Lote de receitas** (“Exportar casamento”): cada fotografia é renderizada **uma vez** na
  resolução máxima necessária em `f32`; cada receita deriva daí (redimensionar, converter,
  codificar). Reduz o tempo total em lotes múltiplos.
- **Pré-verificação:** estimativa de espaço necessário vs. disponível, conflitos de nomes,
  originais em falta — antes de começar.
- **Escrita atómica** (ficheiro temporário + `rename`), retoma por item (`export_item`),
  relatório final com erros, avisos e ficheiros ignorados.
- **Paralelismo** controlado pela fila (limite de memória: cada render de 45 MP em f32
  RGBA ocupa ~720 MB; o executor limita renders simultâneos por memória disponível).
- Codificadores: `jpeg-encoder`/mozjpeg (JPEG), `png`, `tiff` (8/16 bits, float),
  `webp`/libwebp, `ravif` (AVIF, BSD), libjxl (JPEG XL, BSD), `exr`, DNG linear (escrita
  própria sobre `tiff`, segundo a especificação DNG pública), ImageIO no macOS (HEIC),
  `pdf-writer`/`krilla` (PDF), `gif`, `zip`. MP4 de slideshows via AVFoundation (macOS) ou
  FFmpeg externo.
- **Nunca** se exporta para formatos RAW proprietários (CR3, NEF, ARW…).

Ver `EXPORT_FORMATS.md`.

---

## 15. Galerias **[definido]**

- Gerador de **site estático** em Rust com `minijinja` (Apache-2.0): HTML/CSS/JS próprio,
  sem frameworks, sem CDN, funciona offline e em qualquer alojamento.
- Conteúdo: capa, destaque, texto, secções por momento, grelha responsiva, slideshow,
  downloads (individual e ZIP pré-gerado), marca de água, temas claro/escuro, cores,
  logótipo.
- **Palavra-passe:** cifra real das imagens e índice com AES-GCM; chave derivada da
  palavra-passe com PBKDF2 (WebCrypto no navegador do cliente). Protege enquanto a
  palavra-passe não for divulgada. Documentado honestamente: a proteção é do conteúdo, não
  do acesso aos ficheiros cifrados.
- **Limitações honestas do estático:** a expiração é apenas indicativa sem servidor; as
  favoritas do cliente ficam no `localStorage` dele e são devolvidas ao fotógrafo como
  código/ficheiro de lista que a aplicação importa.
- **Publicação (Fase 7):** S3 / R2 / B2 / S3 compatível (API S3), SFTP. Credenciais no
  porta-chaves do sistema (Keychain/Credential Manager via crate `keyring`), nunca no
  catálogo nem no código.

---

## 16. Editor de álbuns **[definido]**

- **Modelo:** álbum → páginas duplas (spreads) → slots (retângulos em mm com recorte da
  fotografia em coordenadas normalizadas) + elementos (texto, grafismos SVG).
- Configuração: formato (presets 20×20 … 30×40 e personalizado), páginas, DPI, sangria,
  margens, zona segura, dobra central, perfil ICC, papel, laboratório.
- **Motor de layout em Rust:** templates JSON; montagem automática por momentos e ordem
  cronológica; recorte inteligente que evita cortar rostos (usa as caixas de
  `face-analysis`) e evita rostos na dobra.
- **Validações:** DPI efetivo por slot, rostos na dobra, elementos fora da zona segura,
  falta de sangria.
- **Editor:** React + SVG (precisão vetorial, zoom, guias, alinhamento, distribuição);
  imagens via protocolo de previews.
- **Exportação:** PDF com perfil de saída ICC (objetivo PDF/X-4 quando o laboratório
  aceitar), JPEG/TIFF por página ou spread, renderizados pelo mesmo pipeline de cor.

---

## 17. Estratégia para Apple Silicon

- Binário nativo `aarch64-apple-darwin`; **binário universal** (`universal-apple-darwin`)
  para incluir Intel, se os testes confirmarem viabilidade de todas as dependências nativas.
- CPU: rayon usa núcleos P e E; trabalho interativo com prioridade (QoS) mais alta nas
  threads do despachante.
- Memória unificada: `wgpu` sobre Metal sem cópias explícitas host↔GPU desnecessárias
  (Fase 9).
- Neural Engine: via CoreML EP do ONNX Runtime ou Core ML direto quando o modelo o
  justificar (operadores não suportados pela CoreML EP caem para CPU — medir por modelo).
- Codecs do sistema: ImageIO para HEIC/HEIF/AVIF (aceleração por hardware, sem custos de
  patentes HEVC).
- macOS mínimo: 12 (Monterey) — limite do WKWebView moderno usado pelo Tauri 2 e do alvo
  `safari15` do Vite.
- Distribuição: `.dmg`. Sem conta Apple Developer, a aplicação não é assinada/notarizada;
  instruções de abertura no `INSTALLATION.md`.

## 18. Estratégia futura para Windows

- Tauri usa **WebView2** (incluído no Windows 11; instalador bootstrap no Windows 10).
- Diferenças já previstas no desenho:
  - protocolo de imagens servido como `http://momento.localhost/` (função única de URL no
    frontend);
  - caminhos: sempre `PathBuf`, nunca concatenação de strings; caminhos longos (`\\?\`);
    nomes de ficheiro reservados (`CON`, `PRN`…) na exportação;
  - HEIC/AVIF via libheif (ou WIC quando as extensões HEIF da Microsoft estão instaladas);
  - IA via DirectML EP; GPU via `wgpu`/DX12;
  - perfil do monitor via WCS.
- Código específico de plataforma isolado em módulos `#[cfg(target_os = …)]` atrás de
  traits; o resto do núcleo não depende do sistema operativo.
- O CI ganhará um runner Windows na Fase 9; até lá, o código evita APIs exclusivas de
  Unix fora dos módulos de plataforma.

---

## 19. Incompatibilidades verificadas na Fase 0

| # | Problema | Decisão |
|---|---|---|
| 1 | Tauri 3 só em alpha | Tauri 2.12.5 fixado |
| 2 | Vários crates SQLite → conflito de `libsqlite3-sys` | Só `rusqlite` (bundled) |
| 3 | WAL inseguro em partilhas de rede | Catálogo obrigatoriamente em disco local; aviso na Fase 1 |
| 4 | LGPL exige ligação substituível em binários estáticos | LibRaw usada sob CDDL-1.0 |
| 5 | ExifTool é Perl (GPL/Artistic); o Perl do macOS está marcado como obsoleto | Leitura nativa em Rust no caminho crítico; ExifTool só como processo externo **opcional** (`-stay_open`), nunca ligado ao binário |
| 6 | `kamadak-exif` não lê EXIF dentro de CR3 (caixas CMT do ISOBMFF) nem de muitos RAW | Metadados RAW pela LibRaw; para CR3, extrair as caixas CMT1–CMT4 e passá-las ao leitor TIFF (Fase 2) |
| 7 | HEVC (HEIC) tem patentes; encoder x265 é GPL | macOS usa ImageIO; libheif/x265 só em builds Windows/Linux, documentado |
| 8 | WebView não partilha texturas com `wgpu` | Previews transferidas como bytes codificados; superfície nativa como plano B |
| 9 | IPC JSON do Tauri é lento para binários grandes | Protocolo URI próprio para imagens; `tauri::ipc::Response` para binários pontuais |
| 10 | Binários ORT para macOS Intel podem não existir | A validar na Fase 3; recurso: CPU ou build próprio |
| 11 | `ort` descarrega binários na compilação | Fixar e verificar no CI |
| 12 | YOLO (Ultralytics) é AGPL-3.0; pesos InsightFace não comerciais | Excluídos dos predefinidos |
| 13 | `tauri-driver` (E2E WebDriver) não suporta macOS | E2E em Linux no CI + testes de componentes; verificação manual no macOS |
| 14 | TypeScript 7 nativo ainda pouco suportado pelas ferramentas | TypeScript 5.9 fixado |
| 15 | Código C/C++ (LibRaw) pode terminar o processo com ficheiros malformados | Processo auxiliar para descodificação em massa (a validar na Fase 2) |
| 16 | A porta Rust `lensfun` (crate) é LGPL-3.0 e seria ligada estaticamente | Correção de lente com implementação própria dos modelos (ptlens/poly3, vinheta, TCA) usando apenas a **base de dados** Lensfun (CC-BY-SA, dados separados do binário), ou biblioteca C Lensfun em ligação dinâmica — decisão na Fase 4 |

## 20. Segurança da aplicação

Ver `SECURITY.md`. Resumo: CSP restritiva, capabilities mínimas do Tauri (só `core:default`
na Fase 0), comandos validam todos os argumentos, nenhum acesso genérico ao sistema de
ficheiros a partir do frontend (caminhos sempre resolvidos e verificados no núcleo).

## 21. Registo de decisões

| ID | Data | Decisão | Alternativas | Impacto |
|---|---|---|---|---|
| D-001 | 2026-11-08 | Tauri 2 + React + TS + Rust | Electron, SwiftUI, Flutter | Base de toda a aplicação |
| D-002 | 2026-11-08 | Catálogo SQLite por projeto + biblioteca global | Catálogo único global | Portabilidade e arquivo simples; pesquisa entre projetos exige abrir vários catálogos |
| D-003 | 2026-11-08 | Espaço de trabalho linear Rec.2020 f32 | ProPhoto linear, ACEScg | Define todas as operações de cor |
| D-004 | 2026-11-08 | LibRaw sob CDDL-1.0, compilada do código-fonte | rawler, rsraw, libraw-sys | Controlo da versão; exige toolchain C++ |
| D-005 | 2026-11-08 | Metadados nativos em Rust; ExifTool opcional externo | ExifTool obrigatório, exiv2 (GPL) | Sem dependência de Perl obrigatória |
| D-006 | 2026-11-08 | Codecs compilados com features; sem plugins dinâmicos na v1 | Plugins dinâmicos | Menos complexidade e problemas de assinatura |
| D-007 | 2026-11-08 | ONNX Runtime + backends Apple opcionais | Só Core ML, só ORT | Mantém o Windows; aproveita o Neural Engine |
| D-008 | 2026-11-08 | Previews via protocolo URI próprio | base64/JSON, servidor HTTP local | Desempenho; sem porta de rede aberta |
| D-009 | 2026-11-08 | pnpm com `minimumReleaseAge` de 7 dias | Sem restrição | Proteção contra pacotes comprometidos recém-publicados |
| D-010 | 2026-11-08 | Nome provisório “Momento Studio”, identificador `pt.momentostudio.desktop` | — | Mudar o identificador depois altera a pasta de dados da aplicação; continua por confirmar pelo utilizador |
| D-011 | 2026-11-08 | **Windows e macOS são plataformas-alvo desde já** (pedido do utilizador), não na Fase 9 | Windows só na Fase 9 | CI em Windows a partir da Fase 1; preferência por bibliotecas Rust puras; caminhos sempre `PathBuf`; URL de imagens por plataforma; atalhos Cmd/Ctrl |
| D-012 | 2026-11-08 | Persistência dos trabalhos em segundo plano no próprio domínio (`import_item.state`, `asset.thumb_state`) em vez de uma tabela genérica `job` | Tabela `job` genérica | Retoma após falha sem infraestrutura extra; a tabela `job` só será criada se surgir trabalho sem estado de domínio próprio |
| D-013 | 2026-11-08 | Miniaturas JPEG 640 px (q82); previews 2560 px a pedido; resolução total: original para JPEG/PNG, JPEG em cache para TIFF | Gerar sempre previews | Importação rápida; disco usado só quando necessário |
| D-014 | 2026-11-08 | JPEG descodificado com `jpeg-decoder` (redução DCT 1/2–1/8) para miniaturas | Descodificar sempre a resolução total | Miniaturas várias vezes mais rápidas, sem dependências C (libjpeg-turbo fica como opção) |
| D-015 | 2026-11-08 | A classificação aplica-se à variante principal; a interface expõe o `AssetId` | Classificação no asset | Preparado para cópias virtuais (Fase 4) sem migração de dados |
| D-016 | 2026-11-08 | LibRaw 0.22.3 como submódulo Git, compilada com `cc`, com uma **camada C própria** (`shim/`) que expõe só estruturas POD e apanha todas as exceções C++ | Crates `libraw-sys`/`rsraw`; bindgen sobre `libraw_data_t` | Independência do layout interno da LibRaw; sem libclang no build; exceções nunca atravessam a FFI |
| D-017 | 2026-11-08 | LibRaw no mesmo processo (sem processo auxiliar), com exceções C++ convertidas em erros e `catch_unwind` no registo de codecs | Processo auxiliar por ficheiro | Simplicidade e velocidade; risco residual de segfault em ficheiros extremamente malformados (a LibRaw é testada por OSS-Fuzz). A reavaliar se surgir algum caso real |
| D-018 | 2026-11-08 | Contrato `ImageCodec` alargado com `metadata()` e `SupportLevel` (completo/parcial com nota); registo devolve `MISSING_CODEC` quando não há descodificador | Lógica por formato no importador | O importador não conhece a LibRaw nem formatos; novos codecs não alteram o núcleo |
| D-019 | 2026-11-08 | HEIC/HEIF/AVIF com codecs do sistema: ImageIO (macOS) e WIC (Windows); nenhum codec HEVC próprio | libheif + libde265 + x265 | Sem patentes HEVC nem GPL; no Windows depende das extensões da Microsoft Store (estado “codec em falta” com indicação) |
| D-020 | 2026-11-08 | PSD/PSB com leitor próprio da imagem composta | Crate `psd` | O crate `psd` entrava em pânico com PSD válidos (ImageMagick) e não lê PSB |
| D-021 | 2026-11-08 | Preview de ecrã de RAW = preview embutida da câmara quando tem ≥ 1600 px, gerada **na mesma leitura** que a miniatura; revelação completa só a 100 % | Revelar sempre | Navegação imediata em milhares de RAW; uma só leitura de 40–60 MB por ficheiro |
| D-022 | 2026-11-08 | Cache com limites LRU: previews 10 GB, resolução total 5 GB, miniaturas sem limite | Sem limite | Projetos de 15 000 RAW não esgotam o disco |
| D-023 | 2026-11-08 | Política de 7 dias também para crates Rust (`scripts/pin-mature-crates.py`) | Só para npm | Mesma proteção da cadeia de fornecimento nos dois ecossistemas |
| D-024 | 2026-11-09 | Inferência com **tract-onnx 0.23.9** (Rust puro, CPU) em vez de ONNX Runtime (`ort`) | `ort` 2.0 (só versões RC), Core ML/DirectML | Mesmo código e resultados em macOS (Intel e Apple Silicon) e Windows, sem bibliotecas nativas a descarregar no build. Os modelos usados são pequenos (0,4–5 MB; 1,5–23 ms por inferência), por isso a CPU basta. Sem Neural Engine/GPU por agora; D-007 fica como evolução se forem precisos modelos maiores |
| D-025 | 2026-11-09 | Modelos **MediaPipe** (Apache-2.0): Face Landmarker (BlazeFace + malha 478 + blendshapes) e Image Embedder MobileNetV3, convertidos TFLite→ONNX por `scripts/prepare-models.py` com SHA-256 fixados; incluídos no instalador (≈ 11 MB), nunca no Git | YuNet + OpenCLIP; InsightFace (pesos não comerciais) | Uma só família de modelos, validada contra os resultados de referência publicados pela Google; os blendshapes dão diretamente “olho fechado” e “sorriso” |
| D-026 | 2026-11-09 | Análise numa imagem de trabalho de lado maior ≤ 2048 px (preview em cache quando existe), metade dos núcleos, uma instância dos modelos por thread; estado em `asset.analysis_state` (retoma após fecho) | Resolução total; serviço externo | Rostos de ~50 px numa fotografia de grupo de 24 MP continuam avaliáveis; análise retomável |
| D-027 | 2026-11-09 | Recomendações calculadas **a partir das métricas gravadas**, separadas da análise: mudar o modo ou o objetivo recalcula em milissegundos, sem reanalisar | Etiquetas calculadas durante a análise | 15 000 fotografias recalculadas em ≈ 32 ms (motor) |
| D-028 | 2026-11-09 | A IA **só recomenda**: nunca altera favoritas/rejeitadas nem apaga. “Aceitar sugestões” é uma ação explícita (por fotografia ou grupo), só em fotografias ainda sem decisão, só MELHOR→favorita e REJEITAR SUGERIDO→rejeitada, reversível | Aplicar automaticamente | Pedido do utilizador: decisões finais sempre humanas |
| D-029 | 2026-11-09 | Identidade facial com **SFace** (OpenCV Zoo, Apache-2.0, 128-D), alinhamento ArcFace por 5 pontos da malha MediaPipe; descarregado do LFS oficial ou de uma cópia no npm aceite **só** se o SHA-256 for idêntico ao objeto oficial | ArcFace/InsightFace (pesos não comerciais), FaceNet, dlib | Licença limpa; validado: tract = onnxruntime (cos 1,000000); mesma pessoa 0,96–0,98, pessoas diferentes < 0,08 |
| D-030 | 2026-11-09 | Pistas de cena/objetos com **EfficientNet-Lite2** do MediaPipe (ImageNet 1000, Apache-2.0) em vez de CLIP | CLIP/OpenCLIP | Não havia um CLIP com proveniência verificável acessível; o ImageNet não tem classes como “anel” ou “bolo de casamento”, por isso essas pistas são mais fracas e a aprendizagem com exemplos do utilizador compensa |
| D-031 | 2026-11-09 | Momentos por **regras explicáveis + linha temporal + exemplos do utilizador**, com confiança mínima 0,5 e margem 0,08; sub-momentos vencem o “pai” (beijo > cerimónia); sem confiança → “Por classificar” | Classificador treinado | Sem dados de treino de casamentos reais nesta fase; cada atribuição tem razões guardadas |
| D-032 | 2026-11-09 | Sincronização de câmaras guardada como correção por câmara (marca+modelo+série) aplicada **na leitura** (SQL); a hora EXIF original fica intacta | Reescrever datas no catálogo/ficheiros | Reversível; seleção e momentos usam a hora corrigida |
| D-033 | 2026-11-09 | Arrastar para momentos com eventos do rato (não HTML5 drag-and-drop) | HTML5 DnD | O WebView2 (Windows) bloqueia o DnD HTML5 quando a importação por arrastar de ficheiros está ativa |
| D-034 | 2026-11-09 | Motor de revelação em Rust (CPU, rayon), o **mesmo código** para pré-visualização, miniaturas, previews e exportação; espaço de trabalho Rec. 2020 linear (D-003), saída sRGB | WebGL na interface + Rust para exportar | Cor consistente entre ecrã e ficheiros; testável numericamente. GPU (wgpu) continua prevista para a Fase 9 |
| D-035 | 2026-11-09 | RAW editados a partir dos **dados lineares do sensor** (LibRaw 16 bits, gama 1, Rec. 2020, sem brilho automático; exposição base automática ≤ +2 EV), meia resolução em memória para editar | Editar a partir da preview embutida | Recuperação real de realces e balanço de brancos; 1 s para abrir um RAW de 36 MP |
| D-036 | 2026-11-09 | **Qualidade dinâmica**: durante o arrasto, render rápido (60 % do ecrã, nível da pirâmide, sem nitidez/ruído); 220 ms depois de parar, render final à resolução do ecrã com sobreamostragem; operações de grande raio numa guia de 384 px fixa | Render único | ~35 ms/frame a arrastar a exposição (4 núcleos); rápido e final dão o mesmo aspeto (diferença média < 2 níveis, testado) |
| D-037 | 2026-11-09 | Frames enviados à interface como **binário** (RGBA + histograma) pela IPC do Tauri e desenhados num canvas | JPEG por protocolo | Sem codificar/descodificar JPEG em cada frame |
| D-038 | 2026-11-09 | Receita por **variante** no catálogo (+ histórico completo e instantâneos) e cópia em **sidecar JSON na pasta do projeto**; nada é escrito junto dos originais | XMP ao lado dos originais | Os cartões/pastas do cliente nunca são tocados; XMP compatível fica para a exportação |
| D-039 | 2026-11-09 | Cópias virtuais = variantes; na interface, uma **chave** por fotografia (`AssetId` na principal, 2⁴⁰ + `VariantId` nas cópias) | Refazer a interface toda à volta de variantes | Classificação e edição por cópia; análise, pessoas e momentos continuam por ficheiro |
| D-040 | 2026-11-09 | **Edição automática por medição de cada fotografia** (histograma, chave da cena, rostos, cinzento das arestas + píxeis cinzentos, saturação, ruído de Immerkær nas zonas lisas, inclinação, aberração cromática, vinheta) que produz uma receita normal (`Settings`); correções **parciais** (até 0,4 EV de "ambiente" conservados; arrefecer menos do que aquecer) | Modelo treinado de edição; parâmetros fixos | Explicável, testável e sem dados de treino; o resultado é editável e desfazível como qualquer ajuste |
| D-041 | 2026-11-09 | Intensidades = fator das correções + acréscimo criativo; a intensidade só modera o **primeiro EV** de exposição (erros grandes corrigem-se sempre); "Personalizada" escolhe a força e os grupos | Uma só escala para tudo | "Muito suave" nunca deixa uma fotografia 2 EV subexposta |
| D-042 | 2026-11-09 | Ruído: o **medido** decide; o ISO × fator do sensor (tabela por marca/modelo) limita a redução quando o ISO é baixo (a "granulação" medida a ISO 100 é textura) e garante redução de cor nos RAW de ISO alto | Só o ISO; só a medição | Não se aplica redução forte a fotografias limpas (testado) |
| D-043 | 2026-11-09 | **AI Denoise com FFDNet** (KAIR, MIT), reconstruído em ONNX a partir dos pesos (sem PyTorch, sem executar pickle) e executado pelo tract por mosaicos de 256 px com sobreposição; o nível de ruído medido entra como mapa de σ; aplicado à **fonte** (antes dos outros ajustes) em memória, nunca escrito | Modelos maiores (NAFNet/Restormer: lentos em CPU, licenças/proveniência menos claras); denoise só tradicional | ≈ 3 s/MP em 4 núcleos (6 MP ≈ 17 s); no editor calcula em segundo plano (a pré-visualização usa a fonte sem ele até estar pronto); é treinado com ruído gaussiano em sRGB, não com RAW reais |
| D-044 | 2026-11-09 | Retoque básico como bloco `retouch` da receita: olhos vermelhos (círculos nas íris da malha MediaPipe, só píxeis muito vermelhos no domínio de visualização) e manchas do sensor (clonagem suave a partir de uma zona vizinha), aplicados na amostragem do original | Pincel de correção; edição de píxeis | Não destrutivo, sobrevive a qualquer mudança de recorte/geometria; zero falsos positivos em 33 rostos reais |
| D-045 | 2026-11-09 | Fila de edição automática própria (até 2 trabalhadores, cada fotografia usa todos os núcleos via rayon) com **pausa por condição** entre fotografias, cancelamento que não toca nas restantes, e acrescento à fila em curso | Reutilizar `WorkQueue` sem pausa | A pausa/cancelamento atuam entre fotografias (a que está em curso termina); a interface nunca bloqueia |
| D-046 | 2026-11-09 | Presets por **componentes** (13: exposição, tons, balanço, presença, HSL, curva, tonalização, P&B, nitidez, ruído, efeitos, objetiva, LUT) aplicados com **Amount** 0–150 % (interpolação entre a fotografia e o preset; acima de 100 % exagera a partir do neutro; curvas misturadas por amostragem); os internos só mudam o aspeto (sem exposição nem balanço) | Blocos inteiros; substituir a receita | Um preset nunca estraga a exposição/balanço já acertados; pode aplicar-se a meio |
| D-047 | 2026-11-09 | Presets do utilizador na **biblioteca global** (`library.db` v2), partilhados por todos os projetos; internos no código (não editáveis, duplicáveis); formato próprio `.momentopreset` (JSON com as LUT incluídas) | Ficheiros soltos; presets por projeto | Organizar (grupos, ordem) com transações; exportar/importar sem perder LUT |
| D-048 | 2026-11-09 | Novos blocos de receita: tonalização (sombras/realces), **P&B com mistura por cor** e **LUT `.cube`** (sRGB, trilinear, no fim do pipeline); LUT log/HDR recusadas | Implementar só o que o XMP pede | Necessários para looks de película e P&B; a LUT é reproduzida exatamente dentro do seu domínio |
| D-049 | 2026-11-09 | XMP: **só importação, aproximada, com relatório** (interpretado/ignorado); sem exportação XMP. Leitor tolerante sem dependência XML | Exportar XMP; prometer compatibilidade | Ver `docs/COMPATIBILIDADE-PRESETS.md` |
| D-050 | 2026-11-09 | Agrupamento de consistência a partir de características baratas: EV da cena (EXIF), dominante residual e chave da miniatura, flash (EXIF no 1.º MB do ficheiro), câmara, hora corrigida e momento; critério "combinado" = sequências contínuas com a mesma luz (exceções de 1–2 fotografias absorvidas) | Embeddings de cena; clustering global | ~10 ms por fotografia; explicável |
| D-051 | 2026-11-09 | **Sincronização inteligente**: copia o aspeto da referência e ajusta cada fotografia **medindo o resultado** (exposição pelos rostos ou pela chave; balanço para o branco ter a mesma cor; contraste; saturação; pele via HSL laranja/vermelho), em renders de 384 px, na fila de fundo | Copiar valores (o "Sincronizar" da Fase 5 continua disponível) | Testado: fotografia 1,3 EV mais escura com tungsténio fica com rosto, vestido e pele iguais à referência; copiar os valores não chegava |

