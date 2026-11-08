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

**Estado na Fase 1 (v0.1.0): JPEG, PNG e TIFF são importados e lidos.** Todos os outros
formatos são detetados pelo conteúdo e registados no relatório de importação como
“não suportados”, sem interromper a importação.

> ⚠️ **Testado c/ ficheiro real** continua “—” para todos os formatos: os testes desta fase
> usam imagens **sintéticas** geradas por código (JPEG/PNG/TIFF com EXIF escrito segundo a
> especificação), não ficheiros de câmaras reais. A validação com ficheiros reais de câmara
> (perfis ICC, EXIF de fabricante, CMYK, TIFF comprimidos) depende das amostras pedidas.

## Formatos comuns

| Formato | Extensões | Deteção | Leitura (adaptador · fase) | Testado c/ ficheiro real |
|---|---|---|---|---|
| JPEG | .jpg .jpeg | ✅ | ✅ jpeg-decoder (redução DCT para miniaturas) · F1 | — (sintético ✅) |
| PNG / APNG | .png .apng | ✅ (APNG como PNG) | ✅ image-rs (8/16 bits, transparência sobre branco; APNG: 1.º fotograma) · F1 | — (sintético ✅) |
| TIFF | .tif .tiff | ✅ | ✅ image-rs/tiff (8/16 bits) · F1 | — (sintético ✅) |
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

## Metadados lidos na importação (Fase 1)

EXIF de JPEG, TIFF e PNG (bloco `eXIf`) com `kamadak-exif`: orientação (1–8, aplicada
automaticamente nas miniaturas e no visualizador), data/hora original (+ subsegundos e fuso
quando existem; recurso a DateTimeDigitized/DateTime), marca, modelo, n.º de série, objetiva,
ISO, velocidade, abertura, distância focal e equivalente 35 mm. EXIF ausente ou corrompido
nunca impede a importação. IPTC/XMP: Fase 2.

## Limites conhecidos da leitura atual

- Perfis ICC embutidos são **preservados** nas miniaturas/previews (o WebView faz a gestão de
  cor), mas ainda não são convertidos pelo núcleo (LittleCMS na Fase 2).
- JPEG CMYK: conversão simplificada (sem perfil) só para miniaturas.
- 16 bits por canal é convertido para 8 bits nas previews (a edição em `f32` chega na Fase 4).
- Imagens acima de 200 megapíxeis são recusadas por segurança.

## Comportamento perante ficheiros inválidos (implementado na Fase 1)

- Formato desconhecido ou ainda sem adaptador → registado como “não suportado”, importação continua. ✅
- Cabeçalho inválido → registado como “erro”, importação continua. ✅
- Cabeçalho válido mas dados truncados → a fotografia entra no catálogo e a miniatura fica
  marcada como falhada, com a mensagem de erro visível no painel de informação. ✅
- Falha de descodificação RAW → processo auxiliar termina sem afetar a aplicação (F2).
- Formato não editável diretamente → conversão interna para TIFF 16 bits ou DNG linear,
  guardada como derivado (o original fica intacto).
