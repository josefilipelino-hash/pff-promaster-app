//! Importação **aproximada** de presets XMP do Lightroom Classic / Camera Raw.
//!
//! Os nomes dos controlos são parecidos, mas os motores não: a mesma "Exposição +0,5" ou
//! "Contraste +20" não produz o mesmo aspeto, porque as curvas, o espaço de trabalho e o
//! perfil da câmara (proprietário) são diferentes. Por isso:
//! - cada campo reconhecido é **interpretado** (nunca "idêntico") e listado no relatório;
//! - tudo o que o motor não sabe reproduzir é **ignorado** e listado com o motivo
//!   (máscaras, perfis de câmara, temperatura em Kelvin, perfis de objetiva, gradação de
//!   meios-tons, retoque);
//! - não se exporta para XMP: um ficheiro XMP criado aqui pareceria compatível sem o ser.
//!
//! O leitor não usa um parser XML completo: lê atributos `crs:Nome="valor"`, elementos
//! simples `<crs:Nome>valor</crs:Nome>` e as listas `rdf:li` das curvas de pontos.

use crate::{Component, Preset};
use momento_image_processing::develop::Settings;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Relatório de compatibilidade para a interface.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatReport {
    /// Campos interpretados (aproximação: o aspeto não será igual ao do Lightroom).
    pub interpreted: Vec<String>,
    /// Campos ignorados, com o motivo.
    pub ignored: Vec<(String, String)>,
}

/// Valores simples e listas de pontos encontrados no XMP.
type Scanned = (BTreeMap<String, String>, BTreeMap<String, Vec<[f32; 2]>>);

const HSL_NAMES: [&str; 8] =
    ["Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta"];

/// Valores `crs:*` encontrados (atributos e elementos simples) e listas de pontos.
fn scan(text: &str) -> Scanned {
    let mut vals = BTreeMap::new();
    let mut lists = BTreeMap::new();
    let mut rest = text;
    // Atributos crs:Nome="valor".
    while let Some(i) = rest.find("crs:") {
        rest = &rest[i + 4..];
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        let after = &rest[name.len()..];
        if let Some(v) = after.strip_prefix("=\"") {
            if let Some(end) = v.find('"') {
                vals.insert(name.clone(), v[..end].to_string());
            }
        } else if let Some(v) = after.strip_prefix('>') {
            // Elemento: valor simples ou lista rdf:li.
            if let Some(end) = v.find(&format!("</crs:{name}>")) {
                let inner = &v[..end];
                if inner.contains("<rdf:li") {
                    let mut pts = Vec::new();
                    for li in inner.split("<rdf:li").skip(1) {
                        let body = li.split_once('>').map(|x| x.1).unwrap_or("");
                        let body = body.split("</rdf:li>").next().unwrap_or("");
                        let nums: Vec<f32> =
                            body.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                        if nums.len() == 2 {
                            pts.push([nums[0] / 255.0, nums[1] / 255.0]);
                        }
                    }
                    lists.insert(name.clone(), pts);
                } else if !inner.contains('<') {
                    vals.insert(name.clone(), inner.trim().to_string());
                }
            }
        }
    }
    (vals, lists)
}

/// Converte um XMP num preset. `name` é usado se o XMP não tiver nome.
pub fn import(text: &str, fallback_name: &str) -> Result<(Preset, CompatReport), String> {
    if !text.contains("crs:") {
        return Err("não é um preset XMP do Lightroom/Camera Raw (sem campos crs:)".into());
    }
    let (vals, lists) = scan(text);
    let mut s = Settings::default();
    let mut comps: BTreeSet<Component> = BTreeSet::new();
    let mut rep = CompatReport::default();
    let num = |k: &str| vals.get(k).and_then(|v| v.trim_start_matches('+').parse::<f32>().ok());
    let mut used: BTreeSet<String> = BTreeSet::new();
    let take = |k: &str,
                comp: Component,
                set: &mut dyn FnMut(f32),
                rep: &mut CompatReport,
                used: &mut BTreeSet<String>,
                comps: &mut BTreeSet<Component>| {
        if let Some(v) = num(k) {
            set(v);
            comps.insert(comp);
            rep.interpreted.push(k.to_string());
            used.insert(k.to_string());
        }
    };
    macro_rules! map {
        ($k:expr, $c:expr, $field:expr) => {
            take($k, $c, &mut |v| $field = v, &mut rep, &mut used, &mut comps)
        };
        ($k:expr, $c:expr, $field:expr, $f:expr) => {
            take($k, $c, &mut |v| $field = $f(v), &mut rep, &mut used, &mut comps)
        };
    }
    use Component::*;
    map!("Exposure2012", Exposure, s.light.exposure);
    map!("Contrast2012", Tone, s.light.contrast);
    map!("Highlights2012", Tone, s.light.highlights);
    map!("Shadows2012", Tone, s.light.shadows);
    map!("Whites2012", Tone, s.light.whites);
    map!("Blacks2012", Tone, s.light.blacks);
    map!("IncrementalTemperature", WhiteBalance, s.wb.temperature, |v: f32| v.clamp(-100.0, 100.0));
    map!("IncrementalTint", WhiteBalance, s.wb.tint, |v: f32| v.clamp(-100.0, 100.0));
    map!("Vibrance", Presence, s.color.vibrance);
    map!("Saturation", Presence, s.color.saturation);
    map!("Clarity2012", Presence, s.effects.clarity);
    map!("Texture", Presence, s.effects.texture);
    map!("Dehaze", Presence, s.effects.dehaze);
    for (i, n) in HSL_NAMES.iter().enumerate() {
        map!(&format!("HueAdjustment{n}"), Hsl, s.hsl.hue[i]);
        map!(&format!("SaturationAdjustment{n}"), Hsl, s.hsl.saturation[i]);
        map!(&format!("LuminanceAdjustment{n}"), Hsl, s.hsl.luminance[i]);
        map!(&format!("GrayMixer{n}"), BlackWhite, s.bw.mix[i]);
    }
    map!("ParametricShadows", Curve, s.curve.shadows);
    map!("ParametricDarks", Curve, s.curve.darks);
    map!("ParametricLights", Curve, s.curve.lights);
    map!("ParametricHighlights", Curve, s.curve.highlights);
    map!("ParametricShadowSplit", Curve, s.curve.split_shadows);
    map!("ParametricMidtoneSplit", Curve, s.curve.split_midtones);
    map!("ParametricHighlightSplit", Curve, s.curve.split_highlights);
    map!("Sharpness", Sharpening, s.detail.sharpen_amount);
    map!("SharpenRadius", Sharpening, s.detail.sharpen_radius, |v: f32| v.clamp(0.5, 3.0));
    map!("SharpenDetail", Sharpening, s.detail.sharpen_detail);
    map!("SharpenEdgeMasking", Sharpening, s.detail.sharpen_masking);
    map!("LuminanceSmoothing", NoiseReduction, s.detail.noise_luminance);
    map!("LuminanceNoiseReductionDetail", NoiseReduction, s.detail.noise_detail);
    map!("ColorNoiseReduction", NoiseReduction, s.detail.noise_color);
    map!("PostCropVignetteAmount", Effects, s.effects.vignette);
    map!("PostCropVignetteMidpoint", Effects, s.effects.vignette_midpoint);
    map!("PostCropVignetteFeather", Effects, s.effects.vignette_feather);
    map!("PostCropVignetteRoundness", Effects, s.effects.vignette_roundness);
    map!("GrainAmount", Effects, s.effects.grain);
    map!("GrainSize", Effects, s.effects.grain_size);
    map!("GrainFrequency", Effects, s.effects.grain_roughness);
    map!("LensManualDistortionAmount", Lens, s.lens.distortion, |v: f32| -v);
    map!("VignetteAmount", Lens, s.lens.vignetting);
    map!("VignetteMidpoint", Lens, s.lens.vignetting_midpoint);
    // Tonalização: "Split toning" (antigo) ou "Color grading" (sombras/realces).
    for (prefix, hue_k, sat_k, bal_k) in [
        (
            "SplitToning",
            "SplitToningShadowHue",
            "SplitToningShadowSaturation",
            "SplitToningBalance",
        ),
        ("ColorGrade", "ColorGradeShadowHue", "ColorGradeShadowSat", "ColorGradeBlending"),
    ] {
        let _ = prefix;
        map!(hue_k, Grading, s.grading.shadows_hue);
        map!(sat_k, Grading, s.grading.shadows_sat);
        if bal_k == "SplitToningBalance" {
            map!(bal_k, Grading, s.grading.balance);
        }
    }
    map!("SplitToningHighlightHue", Grading, s.grading.highlights_hue);
    map!("SplitToningHighlightSaturation", Grading, s.grading.highlights_sat);
    map!("ColorGradeHighlightHue", Grading, s.grading.highlights_hue);
    map!("ColorGradeHighlightSat", Grading, s.grading.highlights_sat);
    if vals.get("ConvertToGrayscale").is_some_and(|v| v.eq_ignore_ascii_case("true")) {
        s.bw.enabled = true;
        comps.insert(BlackWhite);
        rep.interpreted.push("ConvertToGrayscale".into());
        used.insert("ConvertToGrayscale".into());
    } else if vals.contains_key("ConvertToGrayscale") {
        used.insert("ConvertToGrayscale".into());
        comps.insert(BlackWhite);
    }
    for (key, field) in [
        ("ToneCurvePV2012", 0usize),
        ("ToneCurvePV2012Red", 1),
        ("ToneCurvePV2012Green", 2),
        ("ToneCurvePV2012Blue", 3),
    ] {
        if let Some(pts) = lists.get(key) {
            let identity = pts.iter().all(|p| (p[0] - p[1]).abs() < 1e-3);
            if !identity && pts.len() >= 2 {
                let target = match field {
                    0 => &mut s.curve.rgb,
                    1 => &mut s.curve.red,
                    2 => &mut s.curve.green,
                    _ => &mut s.curve.blue,
                };
                *target = pts.clone();
                comps.insert(Curve);
                rep.interpreted.push(key.into());
            }
        }
    }
    // Ignorados com motivo (os mais comuns).
    let reasons: [(&str, &str); 14] = [
        (
            "Temperature",
            "temperatura em Kelvin: depende do RAW e da câmara; o motor usa balanço relativo",
        ),
        ("Tint", "matiz absoluta associada à temperatura em Kelvin"),
        ("CameraProfile", "perfis de câmara da Adobe são proprietários"),
        ("Look", "perfis criativos (Looks) da Adobe são proprietários"),
        ("MaskGroupBasedCorrections", "máscaras e ajustes locais não existem no motor"),
        ("GradientBasedCorrections", "filtros graduados (ajustes locais) não existem no motor"),
        (
            "CircularGradientBasedCorrections",
            "filtros radiais (ajustes locais) não existem no motor",
        ),
        ("PaintBasedCorrections", "pincel de ajuste (ajustes locais) não existe no motor"),
        ("LensProfileEnable", "perfis de objetiva (Lensfun/Adobe) não estão integrados"),
        (
            "AutoLateralCA",
            "aberração cromática automática por perfil (a edição automática mede-a por fotografia)",
        ),
        ("ColorGradeMidtoneHue", "gradação de meios-tons não existe no motor"),
        ("ColorGradeGlobalHue", "gradação global não existe no motor"),
        ("RetouchAreas", "retoque não é transportável entre fotografias"),
        ("CropTop", "recorte não faz parte de um preset"),
    ];
    for (k, why) in reasons {
        if vals.contains_key(k) || text.contains(&format!("crs:{k}")) {
            rep.ignored.push((k.into(), why.into()));
            used.insert(k.into());
        }
    }
    // Restantes campos crs: não reconhecidos (exceto metadados do preset).
    let meta = [
        "Version",
        "ProcessVersion",
        "PresetType",
        "Cluster",
        "UUID",
        "SupportsAmount",
        "SupportsAmount2",
        "SupportsColor",
        "SupportsMonochrome",
        "SupportsHighDynamicRange",
        "SupportsNormalDynamicRange",
        "SupportsSceneReferred",
        "SupportsOutputReferred",
        "RequiresRGBTables",
        "CameraModelRestriction",
        "Copyright",
        "ContactInfo",
        "Name",
        "ShortName",
        "SortName",
        "Group",
        "Description",
        "HasSettings",
        "AlreadyApplied",
        "WhiteBalance",
    ];
    for k in vals.keys().chain(lists.keys()) {
        if !used.contains(k) && !meta.contains(&k.as_str()) && !k.starts_with("ToneCurve") {
            rep.ignored.push((k.clone(), "sem equivalente no motor".into()));
        }
    }
    if comps.is_empty() {
        return Err("o XMP não tem ajustes que o motor consiga interpretar".into());
    }
    let name = xmp_name(text).unwrap_or_else(|| fallback_name.to_string());
    let preset = Preset {
        id: String::new(),
        name,
        group: "Importados".into(),
        components: comps.into_iter().collect(),
        settings: s,
        builtin: false,
        description: "Importado de XMP (interpretação aproximada)".into(),
    };
    Ok((preset, rep))
}

/// Nome do preset: `crs:Name` → `rdf:li` com o texto (formato do Lightroom).
fn xmp_name(text: &str) -> Option<String> {
    let i = text.find("<crs:Name>")?;
    let rest = &text[i..];
    let li = rest.find("<rdf:li")?;
    let body = rest[li..].split_once('>')?.1;
    let name = body.split("</rdf:li>").next()?.trim();
    (!name.is_empty() && !name.contains('<')).then(|| name.to_string())
}
