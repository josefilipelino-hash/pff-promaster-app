# Formatos de exportação

> **Estado na Fase 9 (v0.9.0):** motor de exportação implementado. A tabela diz o que
> existe **e foi testado** (no Linux); macOS e Windows ainda não foram executados.

A exportação parte **sempre do original** (RAW em resolução total do sensor, ou o ficheiro
original) com a receita de edição — nunca de uma preview. A imagem final do motor de
revelação (Rec. 2020 linear, vírgula flutuante) é redimensionada em luz linear (Lanczos 3),
recebe a nitidez de saída e a marca de água, é convertida para o espaço de cor de saída e
só então quantizada (8 bits com ruído anti-bandas; 10/12/16 bits sem).

## Formatos

| Formato | Profundidades | Compressão | Cor | Metadados | Biblioteca | Verificação |
|---|---|---|---|---|---|---|
| JPEG | 8 | qualidade 1–100 (4:4:4 a partir de 90) | ICC (qualquer, incl. personalizado) | EXIF + XMP | jpeg-encoder | Pillow; PSNR ≥ 38 dB a q92 |
| JPEG XL | 8, 16 | **só sem perdas** | sRGB, P3, Adobe RGB, ProPhoto, Rec. 709/2020 (codificação de cor no cabeçalho, sem ICC) | Exif + XMP (caixas) | zune-jpegxl + cabeçalho próprio | jxl-oxide e libjxl; exato |
| JPEG 2000 (JP2) | 8, 12, 16 | sem perdas (5/3) ou com perdas (9/7) | ICC (caixa `colr`) | — (o openjp2 não escreve caixas de metadados) | openjp2 (Rust) | OpenJPEG; exato sem perdas |
| PNG | 8, 16 | rápida / máxima | ICC (`iCCP`) | eXIf + XMP (iTXt) | png | Pillow/imagecodecs; exato |
| TIFF | 8, 16, 32 float | nenhuma, LZW, ZIP (preditor horizontal nos inteiros) | ICC (etiqueta 34675) | Artist, Copyright, Make, Model, DateTime (ASCII) + XMP | tiff | tifffile; exato |
| BigTIFF | 8, 16, 32 float | idem | idem | idem | tiff | tifffile; exato |
| WebP | 8 | com perdas (qualidade) / sem perdas | ICC (VP8X) | EXIF + XMP | libwebp (webp) | Pillow; exato sem perdas |
| AVIF | 8, 10, 12 | qualidade 1–100 (4:4:4) | **só** sRGB, Display P3, Rec. 709, Rec. 2020 (CICP/nclx; o contentor não leva ICC) | EXIF | rav1e + avif-serialize | libavif (imagecodecs/Pillow) |
| OpenEXR | 16 float, 32 float | nenhuma, ZIP, PIZ | **linear**, com as cromaticidades do espaço | owner, data | exr | OpenEXR (Python); exato em 32 bits |
| PSD | 8, 16 | nenhuma, RLE | ICC (recurso 1039) | XMP (recurso 1060), resolução | escrita própria (imagem composta, sem camadas) | psd-tools e o nosso leitor |
| PDF | 8 (JPEG embutido) | qualidade | ICCBased | Autor, XMP | escrita própria | pypdf; página = tamanho de impressão pelo PPI |
| HEIF / HEIC | — | — | — | — | **indisponível** | — |
| DNG | — | — | — | — | **indisponível** | — |

**HEIF/HEIC:** codificar exige um codificador HEVC (x265 é GPL e há patentes). Não está
incluído. O AVIF usa o mesmo contentor HEIF com AV1. No macOS o ImageIO do sistema codifica
HEIC; usá-lo fica para quando for possível testar num Mac.

**DNG:** um DNG útil para arquivo guarda os dados do sensor por revelar, com as matrizes de
cor da câmara; ainda não está implementado (a receita "Arquivo DNG" aparece desativada com
a explicação).

**Não exportamos** para formatos RAW proprietários (CR3, NEF, ARW, RAF, …).

## Espaços de cor

sRGB, Display P3, Adobe RGB (1998) compatível, ProPhoto RGB, Rec. 709, Rec. 2020 e **perfil
ICC personalizado / do laboratório** (só RGB; intento percetual ou colorimétrico relativo
com compensação do ponto negro). Os perfis dos espaços normalizados são gerados pela
LittleCMS (ICC v2, matriz + curvas) e embutidos; a conversão por matriz foi verificada
contra a LittleCMS com esse mesmo perfil. Fora da gama, os valores são cortados por
componente (como no ecrã). Formatos sem ICC (JPEG XL, AVIF, EXR) sinalizam a cor de forma
nativa e só aceitam espaços que conseguem descrever.

## Opções por receita

Formato · profundidade · qualidade · compressão · espaço de cor / perfil ICC + intento ·
dimensões (original, lado longo, lado curto, largura, altura, caber em, preencher com
recorte ao centro, megapíxeis; px, cm ou polegadas) · não ampliar · PPI/DPI · nitidez de
saída (ecrã, papel mate, papel brilhante; baixa/normal/alta) · metadados (tudo, só
copyright e contacto, nada; remover GPS) · copyright, autor, contacto · marca de água
(texto com a fonte Inter incluída ou outra TTF/OTF, ou imagem PNG/JPEG; tamanho, opacidade,
9 posições, margem) · nome (modelo com campos `{nome}` `{seq}` `{data}` `{hora}`
`{projeto}` `{cliente}` `{receita}` `{momento}` `{largura}` `{altura}`, prefixo, sufixo,
início e dígitos da sequência) · subpastas · conflitos (acrescentar -2…, substituir — nunca
os originais —, ignorar).

## Receitas incluídas

| Receita | Ficheiro | Dimensões | Nitidez | Metadados | Pasta |
|---|---|---|---|---|---|
| Alta Resolução | JPEG q95 sRGB | original, 300 PPI | — | tudo, sem GPS | Entrega/AltaResolucao |
| Galeria | JPEG q85 | lado longo 2560 px | ecrã | tudo, sem GPS | Galeria |
| Website | JPEG q80 | lado longo 2048 px | ecrã | só copyright | Website |
| Email | JPEG q75 | lado longo 1200 px | ecrã (baixa) | só copyright | Email |
| Instagram 4:5 | JPEG q90 | 1080×1350 exatos (recorte ao centro) | ecrã | só copyright | Instagram |
| Instagram Square | JPEG q90 | 1080×1080 exatos | ecrã | só copyright | Instagram |
| Impressão | JPEG q100 sRGB | original, 300 PPI | papel brilhante | tudo, sem GPS | Impressao |
| Album | JPEG q95 | lado longo 4000 px | papel mate (baixa) | tudo, sem GPS | Album |
| Arquivo TIFF | TIFF 16 bits ZIP, Adobe RGB | original | — | tudo, com GPS | Arquivo |
| Arquivo DNG | — | — | — | — | indisponível |

Plano **"Exportar casamento"** (uma operação): Alta Resolução + Galeria + Instagram 4:5 +
Impressão + Arquivo TIFF → `Entrega/AltaResolucao`, `Galeria`, `Instagram`, `Impressao`,
`Arquivo`. Receitas e planos do utilizador ficam na biblioteca global (todos os projetos).

## Fila

Cada fotografia é revelada **uma vez** e dela saem todas as receitas. A fila mostra
progresso, ficheiros e fotografias restantes, bytes escritos e tamanho total estimado
(corrigido com os ficheiros reais à medida que avançam), espaço livre no destino, tempo
restante e erros por ficheiro; pausa, retoma e cancelamento entre ficheiros. Sem espaço,
pausa sozinha. Escrita atómica (ficheiro temporário + mudança de nome): nunca ficam
ficheiros a meio.
