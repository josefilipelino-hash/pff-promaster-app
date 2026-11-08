# Formatos suportados (leitura)

> **Compatibilidade abrangente com formatos profissionais, RAW e formatos modernos, com
> sistema extensível de codecs.**
>
> Esta tabela distingue rigorosamente três níveis. Nenhum formato é anunciado como
> suportado sem teste com um ficheiro real.

Legenda das colunas:
- **Deteção**: o formato real é identificado pelo conteúdo (assinatura). ✅ = implementado
  e testado com cabeçalhos sintéticos.
- **Leitura**: descodificação de imagem. 📅 = planeado para a fase indicada.
- **Testado c/ ficheiro real**: só passa a ✅ com um ficheiro real verificado.

**Estado na Fase 0: só existe deteção. Nenhum formato é ainda lido.**

## Formatos comuns

| Formato | Extensões | Deteção | Leitura (adaptador · fase) | Testado c/ ficheiro real |
|---|---|---|---|---|
| JPEG | .jpg .jpeg | ✅ | image-rs/zune-jpeg · 📅 F1 | — |
| PNG / APNG | .png .apng | ✅ (APNG como PNG) | image-rs · 📅 F1 (APNG: 1.º fotograma) | — |
| TIFF | .tif .tiff | ✅ | tiff · 📅 F1 | — |
| BigTIFF | .tif .tiff .btf | ✅ | tiff · 📅 F2 | — |
| BMP | .bmp | ✅ | image-rs · 📅 F2 | — |
| GIF | .gif | ✅ | image-rs · 📅 F2 | — |
| WebP | .webp | ✅ | image-rs · 📅 F2 | — |
| AVIF | .avif | ✅ | ImageIO (macOS) / libheif · 📅 F2 | — |
| HEIF / HEIC | .heif .heic .hif | ✅ | ImageIO (macOS) / libheif · 📅 F2 | — |
| JPEG XL | .jxl | ✅ | jxl-oxide · 📅 F2 | — |
| JPEG 2000 | .jp2 .j2k .jpx | ✅ | ImageIO (macOS) / OpenJPEG · 📅 F2 | — |
| PSD / PSB | .psd .psb | ✅ | psd (imagem composta) · 📅 F2; PSB a validar | — |
| OpenEXR | .exr | ✅ | exr · 📅 F2 | — |
| Radiance HDR (RGBE) | .hdr .pic | ✅ | image-rs · 📅 F2 | — |
| ICO | .ico | ✅ | image-rs · 📅 F2 | — |
| PPM/PGM/PBM | .ppm .pgm .pbm .pnm | ✅ | image-rs · 📅 F2 | — |
| PAM | .pam | ✅ | image-rs · 📅 F2 | — |
| TGA | .tga | ⚠️ só por extensão (sem assinatura) | image-rs · 📅 F2 | — |
| PCX | .pcx | ✅ | a avaliar (sem adaptador maduro identificado) | — |
| SVG | .svg | ✅ | resvg · 📅 F6 (marcas de água, templates) | — |
| PDF | .pdf | ✅ | pdfium · 📅 F8 (grafismos de álbum) | — |

## Formatos RAW

Leitura de todos via **LibRaw** (📅 Fase 2), com preview embutida na importação.

| Fabricante | Extensões | Deteção |
|---|---|---|
| Canon | CR3 | ✅ assinatura (ISOBMFF `crx `) |
| Canon | CR2 | ✅ assinatura (TIFF + `CR`) |
| Canon | CRW | ✅ assinatura (`HEAPCCDR`) |
| Fujifilm | RAF | ✅ assinatura |
| Olympus / OM System | ORF | ✅ assinatura |
| Panasonic | RW2 | ✅ assinatura |
| Minolta | MRW | ✅ assinatura |
| Sigma | X3F | ✅ assinatura (suporte LibRaw a X3F é limitado; a validar) |
| Nikon | NEF, NRW | ✅ TIFF + extensão (confirmação pelas tags na F2) |
| Sony | ARW, SRF, SR2 | ✅ TIFF + extensão |
| Adobe / Leica / Pentax / Ricoh / DJI / Apple ProRAW | DNG | ✅ TIFF + extensão (confirmação por `DNGVersion` na F2) |
| Leica | RWL | ✅ TIFF + extensão |
| Pentax | PEF | ✅ TIFF + extensão |
| Hasselblad | 3FR, FFF | ✅ TIFF + extensão |
| Phase One | IIQ | ✅ TIFF + extensão (a validar com ficheiro real) |
| Samsung | SRW | ✅ TIFF + extensão |
| Epson | ERF | ✅ TIFF + extensão |
| Kodak | DCR, KDC | ✅ TIFF + extensão |
| Leaf / Mamiya | MOS, MEF | ✅ TIFF + extensão |
| Panasonic (antigos) | RAW | ✅ TIFF + extensão |

Nenhum RAW foi ainda testado com ficheiro real.

## Comportamento perante ficheiros inválidos (a partir da Fase 1)

- Formato desconhecido → registado como “não suportado”, importação continua.
- Cabeçalho válido mas dados corrompidos → registado como “corrompido”, importação continua.
- Falha de descodificação RAW → processo auxiliar termina sem afetar a aplicação (F2).
- Formato não editável diretamente → conversão interna para TIFF 16 bits ou DNG linear,
  guardada como derivado (o original fica intacto).
