use super::*;
use image::{Rgb, RgbImage};
use momento_image_processing::develop::{render_rgb, Quality, Source};

fn scene() -> Source {
    let img = RgbImage::from_fn(300, 200, |x, y| {
        if y < 60 {
            Rgb([(x * 255 / 299) as u8; 3])
        } else if y < 130 {
            [Rgb([200, 60, 50]), Rgb([215, 165, 135]), Rgb([60, 160, 70]), Rgb([60, 100, 200])]
                [(x / 75) as usize]
        } else {
            Rgb([30 + (x / 3) as u8, 40, 50])
        }
    });
    Source::from_srgb(&img, 300, 200)
}

#[test]
fn amount_blends_only_the_chosen_components() {
    let mut base = Settings::default();
    base.light.exposure = 0.7;
    base.wb.temperature = 12.0;
    base.light.contrast = 10.0;
    base.geometry.straighten = 2.0;
    let mut p = Settings::default();
    p.light.contrast = 30.0;
    p.light.exposure = -1.0;
    p.color.vibrance = 20.0;
    p.bw.enabled = true;
    p.curve.rgb = vec![[0.0, 0.1], [1.0, 1.0]];
    let comps = [Component::Tone, Component::Presence, Component::BlackWhite, Component::Curve];
    let full = apply(&base, &p, &comps, 100.0);
    assert_eq!(full.light.contrast, 30.0);
    assert_eq!(full.color.vibrance, 20.0);
    assert!(full.bw.enabled);
    assert_eq!(full.curve.rgb, p.curve.rgb);
    // Não incluídos: ficam os da fotografia.
    assert_eq!(
        (full.light.exposure, full.wb.temperature, full.geometry.straighten),
        (0.7, 12.0, 2.0)
    );
    // 0 % = nada muda; 50 % = a meio; 150 % = exagera a partir do neutro.
    assert_eq!(apply(&base, &p, &comps, 0.0), base);
    let half = apply(&base, &p, &comps, 50.0);
    assert_eq!((half.light.contrast, half.color.vibrance), (20.0, 10.0));
    assert!(half.bw.enabled, "booleanos a partir de 50 %");
    let curve_mid = half.curve.rgb.first().unwrap()[1];
    assert!((curve_mid - 0.05).abs() < 0.01, "curva a meio: {:?}", half.curve.rgb);
    assert!(!apply(&base, &p, &comps, 40.0).bw.enabled);
    let more = apply(&base, &p, &comps, 150.0);
    assert_eq!((more.light.contrast, more.color.vibrance), (45.0, 30.0));
    // Capturar só os componentes escolhidos.
    let c = capture(&full, &[Component::Tone]);
    assert_eq!(c.light.contrast, 30.0);
    assert_eq!(c.light.exposure, 0.0);
    assert!(!c.bw.enabled);
}

#[test]
fn lut_amount_scales() {
    let mut p = Settings::default();
    p.lut.id = "abc".into();
    p.lut.name = "Teste".into();
    p.lut.amount = 80.0;
    let s = apply(&Settings::default(), &p, &[Component::Lut], 50.0);
    assert_eq!((s.lut.id.as_str(), s.lut.amount), ("abc", 40.0));
}

#[test]
fn builtin_presets_are_complete_and_distinct() {
    let b = builtin::builtin();
    let names: Vec<&str> = b.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Natural Wedding",
            "Clean Editorial",
            "Fine Art",
            "Romantic Soft",
            "Film Warm",
            "Film Clean",
            "Golden Hour",
            "Ceremony Indoor",
            "Reception Night",
            "Flash Clean",
            "B&W Classic",
            "B&W Documentary",
            "B&W Fine Art"
        ]
    );
    let ids: std::collections::HashSet<_> = b.iter().map(|p| &p.id).collect();
    assert_eq!(ids.len(), 13);
    let src = scene();
    let mut photo = Settings::default();
    photo.light.exposure = 0.4;
    photo.wb.temperature = 8.0;
    let mut renders = Vec::new();
    for p in &b {
        assert!(p.builtin && !p.description.is_empty());
        assert!(
            !p.components.contains(&Component::Exposure)
                && !p.components.contains(&Component::WhiteBalance)
        );
        let s = apply(&photo, &p.settings, &p.components, 100.0);
        assert_eq!(
            (s.light.exposure, s.wb.temperature),
            (0.4, 8.0),
            "{} mantém exposição e balanço",
            p.name
        );
        let img = render_rgb(&src, &s, 150, Quality::Final);
        // P&B (eventualmente com um tom subtil) vs. cor.
        let spread = img
            .pixels()
            .map(|q| q[0].max(q[1]).max(q[2]) - q[0].min(q[1]).min(q[2]))
            .max()
            .unwrap();
        if p.settings.bw.enabled {
            let limit = if p.settings.grading.shadows_sat > 0.0 { 25 } else { 2 };
            assert!(spread <= limit, "{}: {spread}", p.name);
        } else {
            assert!(spread > 60, "{}: {spread}", p.name);
        }
        renders.push(img.into_raw());
    }
    for i in 0..renders.len() {
        for j in i + 1..renders.len() {
            assert_ne!(renders[i], renders[j], "{} = {}", b[i].name, b[j].name);
        }
    }
    // Um preset a cores depois de um P&B volta às cores.
    let bw = b.iter().find(|p| p.name == "B&W Classic").unwrap();
    let nat = b.iter().find(|p| p.name == "Natural Wedding").unwrap();
    let s = apply(
        &apply(&photo, &bw.settings, &bw.components, 100.0),
        &nat.settings,
        &nat.components,
        100.0,
    );
    assert!(!s.bw.enabled);
    assert_eq!(s.light.contrast, nat.settings.light.contrast);
}

#[test]
fn file_roundtrip_with_lut() {
    let mut p = builtin::builtin()[4].clone();
    p.id = "u1".into();
    p.settings.lut.id = "lutid".into();
    p.components.push(Component::Lut);
    let cube =
        "LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n".to_string();
    let text = file::export(&[p.clone()], [("lutid".to_string(), cube)].into());
    let f = file::import(&text).unwrap();
    assert_eq!(f.presets[0].settings, p.settings);
    assert!(!f.presets[0].builtin);
    assert!(f.luts.contains_key("lutid"));
    assert!(file::import("{}").is_err());
    assert!(file::import(&text.replace("\"version\": 1", "\"version\": 9"))
        .unwrap_err()
        .contains("recente"));
}

const XMP: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
   crs:PresetType="Normal" crs:Version="15.0" crs:ProcessVersion="11.0"
   crs:Exposure2012="+0.35" crs:Contrast2012="+18" crs:Highlights2012="-40" crs:Shadows2012="+25"
   crs:Temperature="5600" crs:Tint="+8" crs:Vibrance="+12" crs:Clarity2012="+5"
   crs:SaturationAdjustmentGreen="-30" crs:HueAdjustmentOrange="-4"
   crs:SplitToningShadowHue="210" crs:SplitToningShadowSaturation="10"
   crs:SplitToningHighlightHue="40" crs:SplitToningHighlightSaturation="15"
   crs:GrainAmount="20" crs:ColorGradeMidtoneHue="30" crs:CameraProfile="Adobe Standard"
   crs:HasSettings="True">
   <crs:Name><rdf:Alt><rdf:li xml:lang="x-default">Casamento Quente</rdf:li></rdf:Alt></crs:Name>
   <crs:ToneCurvePV2012><rdf:Seq><rdf:li>0, 20</rdf:li><rdf:li>128, 132</rdf:li><rdf:li>255, 245</rdf:li></rdf:Seq></crs:ToneCurvePV2012>
   <crs:MaskGroupBasedCorrections><rdf:Seq><rdf:li>...</rdf:li></rdf:Seq></crs:MaskGroupBasedCorrections>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

#[test]
fn xmp_import_is_approximate_and_reports_what_is_ignored() {
    let (p, rep) = xmp::import(XMP, "sem nome").unwrap();
    assert_eq!(p.name, "Casamento Quente");
    let s = &p.settings;
    assert_eq!(
        (s.light.exposure, s.light.contrast, s.light.highlights, s.light.shadows),
        (0.35, 18.0, -40.0, 25.0)
    );
    assert_eq!(
        (s.color.vibrance, s.effects.clarity, s.hsl.saturation[3], s.hsl.hue[1]),
        (12.0, 5.0, -30.0, -4.0)
    );
    assert_eq!((s.grading.shadows_hue, s.grading.highlights_sat), (210.0, 15.0));
    assert_eq!(s.effects.grain, 20.0);
    assert_eq!(s.curve.rgb.len(), 3);
    assert!((s.curve.rgb[0][1] - 20.0 / 255.0).abs() < 1e-5);
    // Kelvin absoluto, perfil, máscaras e meios-tons: ignorados com motivo.
    let ignored: Vec<&str> = rep.ignored.iter().map(|x| x.0.as_str()).collect();
    for k in [
        "Temperature",
        "Tint",
        "CameraProfile",
        "MaskGroupBasedCorrections",
        "ColorGradeMidtoneHue",
    ] {
        assert!(ignored.contains(&k), "{k} devia ser ignorado: {ignored:?}");
    }
    assert!(!p.components.contains(&Component::WhiteBalance), "sem balanço relativo no XMP");
    assert!(
        p.components.contains(&Component::Exposure) && p.components.contains(&Component::Grading)
    );
    assert!(rep.interpreted.contains(&"Contrast2012".to_string()));
    assert!(xmp::import("<x/>", "x").is_err());
}

/// Fase 15: tonalização laranja (30°) a 50 % ou 150 % continua laranja — a matiz não é
/// interpolada a partir da neutra (220°), só a intensidade escala.
#[test]
fn grading_hue_keeps_the_preset_colour_at_any_amount() {
    let base = Settings::default();
    let mut p = Settings::default();
    p.grading.shadows_hue = 30.0;
    p.grading.shadows_sat = 40.0;
    for amount in [50.0, 100.0, 150.0] {
        let s = apply(&base, &p, &[Component::Grading], amount);
        assert_eq!(s.grading.shadows_hue, 30.0, "a {amount}%");
    }
    assert!(apply(&base, &p, &[Component::Grading], 50.0).grading.shadows_sat < 40.0);
}
