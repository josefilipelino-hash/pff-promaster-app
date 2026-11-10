# Compatibilidade de presets: XMP (Lightroom/Camera Raw) e LUT `.cube`

Resumo honesto do que o motor do PFF ProMaster consegue — e não consegue — reproduzir.

## LUT `.cube` — reproduzidas corretamente (com limites)

| Caso | Estado |
|---|---|
| LUT 3D (2–65³) e 1D com domínio 0–1, feitas para entrada/saída **sRGB** (LUT criativas, "looks") | ✅ aplicadas no fim do pipeline, em sRGB codificado, com interpolação trilinear — o mesmo método dos editores de vídeo. Testado: LUT identidade, troca de canais, inversão, 50 % de intensidade |
| LUT com `DOMAIN_MIN/MAX` ≠ 0–1 ou `LUT_*_INPUT_RANGE` (log/HDR: S-Log3, V-Log, LogC, PQ) | ❌ recusadas com explicação: o motor não produz imagens log, por isso o aspeto não seria o pretendido |
| Cores fora de sRGB | cortadas antes da LUT (igual a aplicar a LUT a um JPEG sRGB) |
| Intensidade | 0–100 % (mistura com a imagem sem LUT), e escala com o Amount do preset |

As LUT importadas ficam na pasta de dados da aplicação (`luts/<id>.cube`) e são copiadas para
`<projeto>/luts/` quando aplicadas, para o projeto abrir noutro computador. Os ficheiros
`.momentopreset` exportados incluem o texto das LUT que usam.

## XMP — importação **aproximada**

Os controlos têm nomes parecidos, mas os motores são diferentes (curvas, espaço de trabalho,
perfis de câmara proprietários da Adobe). **Nenhum campo é reproduzido "igual"**: cada valor é
interpretado pelo nosso motor e o relatório de importação mostra o que foi interpretado e o
que foi ignorado, com o motivo. Não exportamos XMP: um ficheiro criado aqui pareceria
compatível com o Lightroom sem o ser.

| Campos XMP (`crs:`) | Tratamento |
|---|---|
| `Exposure2012`, `Contrast2012`, `Highlights2012`, `Shadows2012`, `Whites2012`, `Blacks2012` | interpretados (mesma escala nominal) |
| `IncrementalTemperature`, `IncrementalTint` (JPEG) | interpretados como balanço relativo |
| `Temperature`, `Tint` (Kelvin, RAW) | **ignorados**: dependem da câmara; o motor usa balanço relativo |
| `Vibrance`, `Saturation`, `Clarity2012`, `Texture`, `Dehaze` | interpretados |
| `HueAdjustment*`, `SaturationAdjustment*`, `LuminanceAdjustment*` (8 cores) | interpretados (HSL) |
| `Parametric*` e `ToneCurvePV2012` (+ Red/Green/Blue) | interpretados (curva paramétrica e de pontos) |
| `ConvertToGrayscale`, `GrayMixer*` | interpretados (preto e branco com mistura) |
| `SplitToning*`, `ColorGradeShadow*`, `ColorGradeHighlight*` | interpretados (tonalização de sombras/realces) |
| `ColorGradeMidtone*`, `ColorGradeGlobal*` | **ignorados**: o motor não tem gradação de meios-tons/global |
| `Sharpness`, `SharpenRadius/Detail/EdgeMasking`, `LuminanceSmoothing`, `ColorNoiseReduction` | interpretados |
| `PostCropVignette*`, `Grain*` | interpretados |
| `LensManualDistortionAmount`, `VignetteAmount` | interpretados; `LensProfileEnable`, `AutoLateralCA` **ignorados** (sem perfis) |
| `CameraProfile`, `Look` | **ignorados**: perfis proprietários |
| Máscaras, filtros graduados/radiais, pincel, retoque, recorte | **ignorados**: não existem no motor ou não pertencem a um preset |
| `.lrtemplate` (Lightroom antigo) | não suportado — exportar como XMP |

Recomendação: depois de importar um XMP, rever numa fotografia e ajustar antes de usar em lote.
