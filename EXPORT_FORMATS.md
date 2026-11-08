# Formatos de exportação

> **Estado na Fase 0: nenhuma exportação implementada.** A exportação chega na Fase 6;
> antes disso, a Fase 1 não exporta ficheiros.

## Formatos planeados

| Formato | Profundidades | Compressão | Biblioteca prevista | Licença | Notas |
|---|---|---|---|---|---|
| JPEG | 8 | com perdas | jpeg-encoder / mozjpeg | MIT/Apache · BSD/IJG | entrega, web |
| JPEG XL | 8, 10, 12, 16, 32f | com/sem perdas | libjxl | BSD-3 | |
| JPEG 2000 | 8, 16 | com/sem perdas | OpenJPEG | BSD-2 | |
| PNG | 8, 16 | sem perdas | png | MIT/Apache | |
| WebP | 8 | com/sem perdas | libwebp | BSD-3 | |
| AVIF | 8, 10, 12 | com/sem perdas | ravif (rav1e) | BSD-2 | codificação lenta |
| HEIF / HEIC | 8, 10 | com perdas | ImageIO (macOS) | sistema | Windows: a avaliar (x265 é GPL) |
| TIFF / BigTIFF | 8, 16, 32f | sem compressão, LZW, Deflate | tiff | MIT | impressão, arquivo |
| PSD | 8, 16 | RLE | escrita própria (imagem composta) | — | camadas: mais tarde |
| OpenEXR | 16f, 32f | ZIP, PIZ | exr | BSD-3 | |
| DNG / DNG linear | 16 | sem perdas | escrita própria segundo a especificação DNG | — | preservação; nunca CR3/NEF/ARW |
| PDF | — | — | pdf-writer / krilla | MIT/Apache | álbuns, provas |
| GIF | 8 (paleta) | LZW | gif | MIT/Apache | sequências simples |
| MP4 | — | H.264/HEVC | AVFoundation (macOS) / FFmpeg externo | sistema / LGPL | slideshows |
| ZIP | — | Deflate | zip | MIT | pacote de entrega |
| Galeria HTML | — | — | gallery (minijinja) | Apache-2.0 | ver Fase 7 |

**Não exportamos** para formatos RAW proprietários (CR3, NEF, ARW, …): são formatos de
câmara. Para preservar informação RAW usa-se DNG.

## Espaços de cor

sRGB, Display P3, Adobe RGB (compatível), ProPhoto RGB, Rec.709, Rec.2020, ACEScg (só em
EXR/TIFF float), perfil ICC personalizado, perfil do laboratório. Conversão via
LittleCMS 2 com rendering intent e compensação do ponto negro configuráveis.

## Opções por receita

Qualidade · compressão · dimensionamento (largura, altura, lado maior/menor, limite de
megapíxeis, limite de tamanho de ficheiro por pesquisa binária da qualidade) · DPI/PPI ·
nome (tokens: `{cliente}`, `{data}`, `{seq:4}`, `{nome_original}`, `{momento}`, …) ·
prefixo/sufixo · subpastas · perfil · metadados (manter tudo / copyright e contacto /
nenhum; remover GPS) · marca de água (texto ou SVG/PNG) · nitidez de saída (ecrã, papel
mate, papel brilhante) · fundo para formatos sem alfa · conflitos (renomear, substituir
com confirmação, ignorar) · exportação paralela.

## Presets de exportação planeados

Entrega em alta resolução · Galeria online · Impressão profissional · Álbum · Redes
sociais · Instagram vertical (1080×1350) · Instagram quadrado (1080×1080) · Facebook ·
Website · Email · Pré-visualização · Arquivo TIFF · Arquivo DNG · Entrega sem metadados
privados · Slideshow · Pacote completo do cliente.

Dimensões específicas de redes sociais serão confirmadas no momento da implementação
(as plataformas alteram-nas com frequência).
