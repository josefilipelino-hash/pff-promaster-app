//! Presets internos. Mudam o **aspeto** (tons, cor, curva, tonalização, P&B, efeitos);
//! não tocam na exposição nem no balanço de brancos de cada fotografia (esses vêm da edição
//! automática ou da sincronização com uma referência). Os presets a cores repõem o P&B e a
//! tonalização, para que aplicar um a seguir a outro dê sempre o mesmo resultado.

use crate::{Component, Preset};
use momento_image_processing::develop::*;

/// Componentes de todos os presets internos.
const COMPONENTS: [Component; 7] = [
    Component::Tone,
    Component::Presence,
    Component::Hsl,
    Component::Curve,
    Component::Grading,
    Component::BlackWhite,
    Component::Effects,
];

fn light(contrast: f32, highlights: f32, shadows: f32, whites: f32, blacks: f32) -> Light {
    Light { exposure: 0.0, contrast, highlights, shadows, whites, blacks }
}

/// HSL a partir de listas (vermelho, laranja, amarelo, verde, água, azul, roxo, magenta).
fn hsl(hue: [f32; 8], saturation: [f32; 8], luminance: [f32; 8]) -> Hsl {
    Hsl { hue, saturation, luminance }
}

fn curve(lights: f32, darks: f32, rgb: &[[f32; 2]]) -> Curve {
    Curve { lights, darks, rgb: rgb.to_vec(), ..Default::default() }
}

fn grading(sh_hue: f32, sh_sat: f32, hl_hue: f32, hl_sat: f32, balance: f32) -> Grading {
    Grading {
        shadows_hue: sh_hue,
        shadows_sat: sh_sat,
        highlights_hue: hl_hue,
        highlights_sat: hl_sat,
        balance,
    }
}

fn effects(
    clarity: f32,
    texture: f32,
    dehaze: f32,
    vignette: f32,
    grain: f32,
    grain_size: f32,
) -> Effects {
    Effects { clarity, texture, dehaze, vignette, grain, grain_size, ..Default::default() }
}

fn color(saturation: f32, vibrance: f32) -> Color {
    Color { saturation, vibrance }
}

const Z: [f32; 8] = [0.0; 8];
/// Curva "mate": pretos levantados e brancos ligeiramente baixados.
const MATTE: [[f32; 2]; 4] = [[0.0, 0.06], [0.25, 0.27], [0.75, 0.76], [1.0, 0.97]];

struct Def {
    id: &'static str,
    name: &'static str,
    group: &'static str,
    description: &'static str,
    settings: Settings,
}

fn defs() -> Vec<Def> {
    let s = Settings::default;
    vec![
        Def {
            id: "natural-wedding",
            name: "Natural Wedding",
            group: "Cor",
            description: "Natural e luminoso: realces protegidos, pele limpa, verdes menos intensos.",
            settings: Settings {
                light: light(5.0, -15.0, 10.0, 5.0, -5.0),
                color: color(0.0, 8.0),
                hsl: hsl(Z, [0.0, -5.0, -5.0, -10.0, 0.0, -5.0, 0.0, 0.0], [0.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
                curve: curve(4.0, -2.0, &[]),
                ..s()
            },
        },
        Def {
            id: "clean-editorial",
            name: "Clean Editorial",
            group: "Cor",
            description: "Contraste limpo e cores contidas, estilo revista.",
            settings: Settings {
                light: light(15.0, -20.0, 5.0, 10.0, -10.0),
                color: color(-5.0, 5.0),
                hsl: hsl(Z, [0.0, -8.0, -15.0, -20.0, 0.0, 0.0, 0.0, 0.0], [0.0, 3.0, 0.0, 0.0, 0.0, -5.0, 0.0, 0.0]),
                curve: curve(5.0, -5.0, &[]),
                effects: effects(8.0, 0.0, 0.0, 0.0, 0.0, 25.0),
                ..s()
            },
        },
        Def {
            id: "fine-art",
            name: "Fine Art",
            group: "Cor",
            description: "Suave e mate, verdes apagados, tons pastel e grão fino.",
            settings: Settings {
                light: light(-10.0, -30.0, 15.0, -5.0, 10.0),
                color: color(-10.0, -5.0),
                hsl: hsl([0.0, 0.0, 0.0, -10.0, 0.0, 0.0, 0.0, 0.0], [0.0, 0.0, -10.0, -35.0, -10.0, -10.0, 0.0, 0.0], [0.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
                curve: curve(0.0, 0.0, &MATTE),
                grading: grading(200.0, 8.0, 40.0, 10.0, 0.0),
                effects: effects(-5.0, 0.0, 0.0, 0.0, 8.0, 20.0),
                ..s()
            },
        },
        Def {
            id: "romantic-soft",
            name: "Romantic Soft",
            group: "Cor",
            description: "Luz suave e quente, menos contraste e pele mais delicada.",
            settings: Settings {
                light: light(-15.0, -25.0, 20.0, 0.0, 15.0),
                color: color(-5.0, 5.0),
                hsl: hsl(Z, [0.0, -5.0, 0.0, -15.0, 0.0, 0.0, 0.0, 0.0], [0.0, 6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
                curve: curve(3.0, 0.0, &MATTE),
                grading: grading(330.0, 6.0, 30.0, 12.0, 10.0),
                effects: effects(-10.0, -5.0, 0.0, 0.0, 0.0, 25.0),
                ..s()
            },
        },
        Def {
            id: "film-warm",
            name: "Film Warm",
            group: "Película",
            description: "Película quente: sombras azul-petróleo, realces dourados, pretos levantados e grão.",
            settings: Settings {
                light: light(10.0, -10.0, 5.0, 0.0, 0.0),
                color: color(0.0, -5.0),
                hsl: hsl([0.0, -5.0, 0.0, -20.0, 0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, -25.0, 0.0, -10.0, 0.0, 0.0], Z),
                curve: curve(0.0, 0.0, &[[0.0, 0.05], [0.5, 0.52], [1.0, 0.96]]),
                grading: grading(190.0, 12.0, 38.0, 18.0, 10.0),
                effects: effects(0.0, 0.0, 0.0, 0.0, 18.0, 30.0),
                ..s()
            },
        },
        Def {
            id: "film-clean",
            name: "Film Clean",
            group: "Película",
            description: "Película limpa: cor natural, ligeiro desvanecimento e grão discreto.",
            settings: Settings {
                light: light(5.0, -10.0, 5.0, 0.0, 0.0),
                hsl: hsl([0.0, 0.0, 0.0, 0.0, 0.0, -5.0, 0.0, 0.0], [0.0, 0.0, 0.0, -15.0, 0.0, 0.0, 0.0, 0.0], Z),
                curve: curve(0.0, 0.0, &[[0.0, 0.03], [1.0, 0.98]]),
                grading: grading(210.0, 6.0, 45.0, 6.0, 0.0),
                effects: effects(0.0, 0.0, 0.0, 0.0, 8.0, 20.0),
                ..s()
            },
        },
        Def {
            id: "golden-hour",
            name: "Golden Hour",
            group: "Luz",
            description: "Realça a luz dourada do fim de tarde sem queimar o céu.",
            settings: Settings {
                light: light(5.0, -20.0, 10.0, 0.0, -5.0),
                color: color(0.0, 10.0),
                hsl: hsl(Z, [0.0, 5.0, 10.0, -10.0, 0.0, 0.0, 0.0, 0.0], Z),
                curve: curve(5.0, 0.0, &[]),
                grading: grading(15.0, 8.0, 35.0, 25.0, 15.0),
                ..s()
            },
        },
        Def {
            id: "ceremony-indoor",
            name: "Ceremony Indoor",
            group: "Luz",
            description: "Igrejas e interiores: abre as sombras, contém o amarelo do tungsténio, reduz ruído.",
            settings: Settings {
                light: light(0.0, -25.0, 25.0, 5.0, -5.0),
                color: color(0.0, 5.0),
                hsl: hsl(Z, [0.0, -10.0, -20.0, 0.0, 0.0, 0.0, 0.0, 0.0], Z),
                detail: Detail { noise_luminance: 15.0, noise_color: 25.0, ..Default::default() },
                ..s()
            },
        },
        Def {
            id: "reception-night",
            name: "Reception Night",
            group: "Luz",
            description: "Festa à noite: luzes coloridas contidas, sombras abertas, redução de ruído forte.",
            settings: Settings {
                light: light(10.0, -30.0, 15.0, 0.0, -10.0),
                color: color(0.0, 5.0),
                hsl: hsl(Z, [0.0, -15.0, -25.0, 0.0, 0.0, 0.0, -15.0, -15.0], Z),
                detail: Detail { noise_luminance: 25.0, noise_color: 35.0, noise_detail: 55.0, ..Default::default() },
                ..s()
            },
        },
        Def {
            id: "flash-clean",
            name: "Flash Clean",
            group: "Luz",
            description: "Flash direto: suaviza os reflexos na pele e escurece um pouco os cantos.",
            settings: Settings {
                light: light(8.0, -35.0, 15.0, -5.0, 0.0),
                hsl: hsl(Z, [-5.0, -8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], [0.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
                effects: effects(-5.0, 0.0, 0.0, -10.0, 0.0, 25.0),
                ..s()
            },
        },
        Def {
            id: "bw-classic",
            name: "B&W Classic",
            group: "Preto e branco",
            description: "Preto e branco clássico, pele luminosa e céus mais escuros.",
            settings: Settings {
                light: light(15.0, -10.0, 5.0, 10.0, -10.0),
                bw: BlackWhite { enabled: true, mix: [10.0, 15.0, 10.0, -10.0, -10.0, -15.0, 0.0, 0.0] },
                curve: curve(5.0, -5.0, &[]),
                effects: effects(0.0, 0.0, 0.0, 0.0, 5.0, 20.0),
                ..s()
            },
        },
        Def {
            id: "bw-documentary",
            name: "B&W Documentary",
            group: "Preto e branco",
            description: "Reportagem: contraste forte, clareza, grão marcado e vinheta.",
            settings: Settings {
                light: light(30.0, -15.0, 0.0, 5.0, -20.0),
                bw: BlackWhite { enabled: true, mix: [0.0, 5.0, 0.0, -10.0, -15.0, -25.0, 0.0, 0.0] },
                effects: Effects { grain_roughness: 60.0, ..effects(20.0, 0.0, 0.0, -12.0, 22.0, 30.0) },
                ..s()
            },
        },
        Def {
            id: "bw-fine-art",
            name: "B&W Fine Art",
            group: "Preto e branco",
            description: "Preto e branco suave e mate, com tom quente subtil.",
            settings: Settings {
                light: light(-5.0, -20.0, 10.0, 0.0, 5.0),
                bw: BlackWhite { enabled: true, mix: [5.0, 20.0, 10.0, 0.0, 0.0, -20.0, 0.0, 0.0] },
                curve: curve(0.0, 0.0, &[[0.0, 0.07], [0.5, 0.52], [1.0, 0.97]]),
                grading: grading(30.0, 10.0, 45.0, 6.0, 0.0),
                effects: effects(-5.0, 0.0, 0.0, 0.0, 10.0, 20.0),
                ..s()
            },
        },
    ]
}

/// Prefixo dos identificadores dos presets internos.
pub const BUILTIN_PREFIX: &str = "builtin:";

pub fn builtin() -> Vec<Preset> {
    defs()
        .into_iter()
        .map(|d| Preset {
            id: format!("{BUILTIN_PREFIX}{}", d.id),
            name: d.name.into(),
            group: d.group.into(),
            components: {
                let mut c = COMPONENTS.to_vec();
                // Interiores e festa: a redução de ruído faz parte do aspeto.
                if ["ceremony-indoor", "reception-night"].contains(&d.id) {
                    c.push(Component::NoiseReduction);
                }
                c
            },
            settings: d.settings,
            builtin: true,
            description: d.description.into(),
        })
        .collect()
}
