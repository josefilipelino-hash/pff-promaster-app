# Formatos suportados (leitura)

> **Compatibilidade abrangente com formatos profissionais, RAW e formatos modernos, com
> sistema extensível de codecs.**
>
> Este documento só marca um formato como **validado** quando foi aberto com sucesso a
> partir de um **ficheiro real** (de câmara, ou produzido por um codificador independente)
> pelo caminho completo da aplicação: deteção → metadados → miniatura → preview →
> resolução total. Estado em 2026-11-08 (Fase 2, v0.2.0).

## Legenda

| Símbolo | Significado |
|---|---|
| ✅ **Validado** | Ficheiro(s) real(is) importado(s) e mostrado(s) corretamente nesta fase |
| ⚠️ **Parcial** | Importado e mostrado, com uma limitação identificada (explicada ao utilizador na aplicação) |
| 🔶 **Implementado, não validado** | Código existe e compila para a plataforma, mas não foi executado com ficheiros reais (sem macOS/Windows/amostras nesta sessão) |
| 🔍 **Só deteção** | Reconhecido pelo conteúdo; aparece como “codec em falta” ou “não suportado” |
| ❌ | Não suportado |

## Estados apresentados ao utilizador

| Estado | Quando | Na aplicação |
|---|---|---|
| **Importada** | Lida com sucesso | Grelha normal |
| **Suporte parcial** | Importada com limitação (ex.: só a preview embutida de um RAW, HDR sem mapeamento de tons, CMYK sem perfil) | Selo “Parcial” na miniatura + explicação no painel de informação |
| **Corrompida** | Formato reconhecido, dados danificados ou incompletos | Relatório de importação (vermelho), com o motivo; se só falhar a miniatura, aviso no painel |
| **Codec em falta** | Formato reconhecido, sem descodificador nesta plataforma | Relatório de importação (amarelo), com indicação do que instalar quando aplicável |
| **Não suportada** | Formato desconhecido ou que não é uma fotografia (SVG, PDF, texto…) | Relatório de importação |

Um ficheiro problemático **nunca** interrompe uma importação: erros dos descodificadores e
pânicos de bibliotecas são isolados por ficheiro (testado com ficheiros reais malformados e
com 5 000 entradas aleatórias).

## RAW de câmara — LibRaw 0.22.3

Compilada do código-fonte (submódulo `packages/raw-engine/libraw-sys/LibRaw`, licença
CDDL-1.0), com zlib (DNG deflate) e Sigma X3F ativados. A lista interna de câmaras da
LibRaw 0.22.3, nesta compilação, tem **1 274 modelos** (`LibRaw::cameraCount()`, verificado por teste); a tabela abaixo distingue o que foi validado aqui.

Fluxo: **preview embutida primeiro** (miniatura e preview de ecrã, ≈ 10–140 ms), revelação
completa **só ao fazer zoom a 100 %** (≈ 0,3–9 s), com orientação, metadados (câmara,
objetiva, exposição, data), profundidade de bits e cor guardados no catálogo.

| Fabricante | Extensão | Estado | Ficheiros reais testados |
|---|---|---|---|
| Canon | CR3 | ✅ | EOS R3, EOS R8 |
| Canon | CR2 | ✅ | EOS 7D, EOS 40D (sRAW) |
| Canon | CRW | ✅ | EOS 10D |
| Nikon | NEF | ✅ | D810, D70 (preview já rodada pela câmara — corrigido), D3S (NASA) |
| Nikon | NEF High Efficiency (Z8/Z9) | ⚠️ esperado parcial | sem amostra; a LibRaw marca este descodificador como não suportado → só preview embutida |
| Nikon | NRW | 🔶 | sem amostra (mesmo descodificador TIFF da LibRaw) |
| Sony | ARW | ✅ | ILCE-7M4 (A7 IV), NEX-5N |
| Sony | SRF, SR2 | 🔶 | sem amostra |
| Fujifilm | RAF | ✅ | X-E1 (X-Trans), FinePix S5500 (Bayer) |
| Panasonic | RW2 | ✅ | DMC-GF7 · ficheiro truncado → **corrompido** ✅ |
| Panasonic | RAW | 🔶 | sem amostra |
| Leica | RWL | ✅ | D-LUX (Typ 109) |
| Leica | DNG | ✅ | T (Typ 701) |
| Olympus / OM System | ORF | ✅ | OM-1, E-M10 |
| Pentax / Ricoh | PEF | ✅ | K-1 Mark II |
| Pentax | DNG | ✅ | Q-S1 |
| Samsung | SRW | ✅ | NX1 |
| Sigma | X3F | ✅ | SD9, DP2 Quattro |
| Kodak | DCR | ✅ | DCS Pro SLR/n (preview já rodada — corrigido) |
| Kodak | KDC | 🔶 | sem amostra |
| Hasselblad | 3FR, FFF | 🔶 | sem amostra |
| Phase One | IIQ | 🔶 | sem amostra |
| Mamiya / Leaf | MEF, MOS | 🔶 | sem amostra |
| Minolta | MRW | 🔶 | sem amostra |
| Epson | ERF | 🔶 | sem amostra |
| DJI | DNG | ✅ | Phantom 4 (FC330) |
| Smartphones | DNG | ✅ | LG Nexus 5X |
| Apple | ProRAW (DNG, compressão JPEG sem perdas) | 🔶 | sem amostra; usa o mesmo descodificador DNG validado |
| Apple | ProRAW com JPEG XL (DNG 1.7) | ⚠️ esperado parcial | a LibRaw sem DNG SDK não descodifica dados JPEG XL do sensor → só preview embutida (descodificada com jxl-oxide) |
| Adobe | DNG | ✅ | 5 DNG de 4 fabricantes |
| Adobe | DNG com compressão com perdas (“lossy DNG”) | 🔶 | sem amostra; compilado sem libjpeg → a validar |
| GoPro | GPR (VC5) | ⚠️ esperado parcial | a LibRaw marca como não suportado sem GPR SDK |

## Formatos de imagem

| Formato | Extensões | Estado | Descodificador | Validado com |
|---|---|---|---|---|
| JPEG | .jpg .jpeg | ✅ | jpeg-decoder (redução DCT) | ficheiros de câmara (Fase 1) e ImageMagick |
| JPEG CMYK | .jpg | ⚠️ | jpeg-decoder | ImageMagick — conversão simplificada, sem perfil |
| PNG (8/16 bits, alfa) | .png | ✅ | image | ImageMagick |
| TIFF 8/16 bits | .tif .tiff | ✅ | image/tiff | ImageMagick |
| BigTIFF | .tif .btf | ✅ | image/tiff | ImageMagick (TIFF64), DNG BigTIFF |
| WebP (com/sem perdas, alfa, animado) | .webp | ✅ (animado: 1.º fotograma) | image-webp | libwebp (ImageMagick), ficheiros reais |
| JPEG XL | .jxl | ✅ | jxl-oxide | libjxl (imagecodecs) |
| JPEG 2000 | .jp2 .j2k .jpx | ✅ | jpeg2k + openjp2 (Rust) | fotografia real `balloon.jp2`, OpenJPEG (ImageMagick) |
| PSD (imagem composta) | .psd | ✅ RGB/cinzentos · ⚠️ CMYK | leitor próprio | ImageMagick, ficheiros reais |
| PSB | .psb | 🔶 (ficheiro real malformado → corrompido ✅) | leitor próprio | teste sintético RLE/RAW |
| BMP | .bmp | ✅ | image | ImageMagick |
| GIF | .gif | ⚠️ 1.º fotograma | image | ImageMagick |
| OpenEXR | .exr | ⚠️ sem mapeamento de tons | image/exr | OpenEXR 3.5 |
| Radiance HDR | .hdr | ⚠️ sem mapeamento de tons | image | ImageMagick |
| ICO | .ico | ✅ | image | ImageMagick |
| PPM/PGM/PBM/PAM | .ppm .pnm .pam | ✅ | image | ImageMagick |
| TGA | .tga | ✅ (detetado pela extensão) | image | ImageMagick |
| **HEIC / HEIF** | .heic .heif .hif | 🔶 macOS (ImageIO) · 🔶 Windows (WIC + extensões) · 🔍 Linux | sistema | 8 ficheiros reais (iPhone, Sony A7 IV, 3 malformados) → **codec em falta** no Linux ✅ |
| **AVIF** | .avif | 🔶 macOS 13+ (ImageIO) · 🔶 Windows (WIC + AV1 Extension) · 🔍 Linux | sistema | 3 ficheiros reais → codec em falta no Linux ✅ |
| PCX | .pcx | 🔍 | — | — |
| SVG, PDF | .svg .pdf | 🔍 não são importados como fotografias | — | — |

### HEIC/HEIF/AVIF por plataforma

- **macOS**: adaptador ImageIO (codecs do sistema, sem custos de patentes HEVC). Compila
  para `aarch64-apple-darwin`; **validar no Mac** (próxima verificação manual).
- **Windows**: adaptador WIC. HEIC exige “HEIF Image Extensions” + “HEVC Video Extensions”
  (Microsoft Store); AVIF exige “AV1 Video Extension”. Sem elas, o estado é “codec em
  falta” com a indicação da extensão a instalar. Compila para Windows; **validar no PC**.
- **Linux** (só desenvolvimento): sem adaptador → “codec em falta”.

## Metadados extraídos

| Origem | Campos |
|---|---|
| LibRaw (RAW) | câmara, objetiva, ISO, velocidade, abertura, focal (+35 mm), data/hora, orientação, bits, cor do sensor, dimensões, preview embutida, descodificador |
| EXIF (JPEG, TIFF, PNG, WebP, HEIF) | os mesmos campos + n.º de série, subsegundos e fuso horário |
| ICC | descrição do perfil de cor embutido (ex.: “Adobe RGB (1998)”, “Display P3”) |
| PSD | modo de cor, profundidade |

IPTC/XMP: ainda não (Fase 5/6).

## Como validar mais formatos

```bash
MOMENTO_SAMPLES_DIR=/pasta/com/ficheiros cargo test -p momento-project-management --release --test real_formats -- --nocapture
```

Gera `import_report.md` com o resultado por ficheiro. Os ficheiros nunca entram no Git.
Quando houver amostras reais de NRW, SRF/SR2, 3FR/FFF, IIQ, KDC, MRW, MEF, ProRAW e lossy
DNG, este documento é atualizado.
