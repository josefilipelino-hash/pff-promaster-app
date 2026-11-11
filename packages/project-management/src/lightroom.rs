//! Classificações para o Lightroom: a seleção assistida passa a estrelas que o Lightroom lê.
//!
//! Um catálogo `.lrcat` é uma base de dados privada da Adobe, sem documentação e diferente
//! entre versões — gerá-lo arriscaria um catálogo que o Lightroom não abre ou corrompe. O
//! caminho suportado pela Adobe são os **ficheiros XMP**: para cada RAW, um `<nome>.xmp` ao
//! lado, que o Lightroom lê ao importar (ou em *Metadados → Ler metadados dos ficheiros*).
//!
//! - Melhor → 5★, Alternativa → 3★, Rever → 2★ (pedido do utilizador);
//! - os RAW **nunca** são alterados: só se cria/atualiza o `.xmp` ao lado;
//! - DNG, JPEG, TIFF, HEIC, PSD: o Lightroom só lê a classificação de dentro do ficheiro,
//!   que nunca se altera — ficam no relatório como não escritos;
//! - um `.xmp` que já exista (Lightroom, Capture One, câmara) só tem a classificação
//!   alterada, e é guardada uma cópia antes (`.xmp.pff-original`, só da primeira vez).

use crate::ProjectSession;
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_image_codecs::{format_from_id, FileFormat, FormatFamily};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Estrelas por etiqueta da seleção assistida (pedido do utilizador).
pub fn stars_for_label(label: &str) -> Option<u8> {
    match label {
        "best" => Some(5),
        "alternative" => Some(3),
        "review" => Some(2),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LightroomOptions {
    /// As estrelas dadas pelo utilizador (≥1) prevalecem sobre a sugestão.
    pub keep_user_stars: bool,
    /// Aplicar também as mesmas estrelas no catálogo do PFF ProMaster (só às fotografias
    /// ainda sem estrelas).
    pub apply_in_app: bool,
}

impl Default for LightroomOptions {
    fn default() -> Self {
        LightroomOptions { keep_user_stars: true, apply_in_app: true }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightroomReport {
    /// `.xmp` criados de novo.
    pub created: u64,
    /// `.xmp` existentes com a classificação atualizada (cópia guardada).
    pub updated: u64,
    /// Por estrelas escritas: [2★, 3★, 5★, outras].
    pub by_stars: [u64; 4],
    /// Fotografias com etiqueta mas que não são RAW (o Lightroom não lê `.xmp` ao lado).
    pub not_raw: Vec<String>,
    /// Originais indisponíveis (disco desligado/em falta).
    pub unavailable: Vec<String>,
    /// Falhas de escrita (pasta só de leitura, permissões…).
    pub failed: Vec<(String, String)>,
    /// Estrelas aplicadas no catálogo da aplicação.
    pub stars_in_app: u64,
}

/// Formatos cujo `.xmp` ao lado o Lightroom lê (RAW proprietários; o DNG guarda o XMP dentro).
fn sidecar_supported(format: FileFormat, file_name: &str) -> bool {
    let dng = Path::new(file_name)
        .extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("dng"));
    format.family() == FormatFamily::Raw && !dng
}

/// Caminho do `.xmp` do Lightroom: mesmo nome, extensão `.xmp`.
pub fn sidecar_path(original: &Path) -> PathBuf {
    original.with_extension("xmp")
}

/// XMP mínimo com a classificação, como o Lightroom o escreve.
pub fn new_sidecar(rating: u8) -> String {
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"PFF ProMaster\">\n \
<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  \
<rdf:Description rdf:about=\"\"\n    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n    \
xmp:Rating=\"{rating}\"/>\n \
</rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>\n"
    )
}

/// Muda só a classificação de um XMP existente (atributo `xmp:Rating="n"` ou elemento
/// `<xmp:Rating>n</xmp:Rating>`); sem ela, acrescenta o atributo ao primeiro
/// `rdf:Description` (e o espaço de nomes `xmp`, se faltar). `None` se o texto não parecer
/// um XMP (não se arrisca estragar o ficheiro).
pub fn set_rating(xmp: &str, rating: u8) -> Option<String> {
    let r = rating.to_string();
    if let Some(i) = xmp.find("xmp:Rating=\"") {
        let start = i + "xmp:Rating=\"".len();
        let end = start + xmp[start..].find('"')?;
        return Some(format!("{}{}{}", &xmp[..start], r, &xmp[end..]));
    }
    if let Some(i) = xmp.find("<xmp:Rating>") {
        let start = i + "<xmp:Rating>".len();
        let end = start + xmp[start..].find("</xmp:Rating>")?;
        return Some(format!("{}{}{}", &xmp[..start], r, &xmp[end..]));
    }
    let d = xmp.find("<rdf:Description")?;
    let tag_end = d + xmp[d..].find('>')?;
    // Antes de "/>" ou ">".
    let insert_at = if xmp[..tag_end].ends_with('/') { tag_end - 1 } else { tag_end };
    let ns = if xmp[d..tag_end].contains("xmlns:xmp=")
        || xmp.contains("xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"")
    {
        String::new()
    } else {
        "\n    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"".to_string()
    };
    Some(format!("{}{ns}\n    xmp:Rating=\"{r}\"{}", &xmp[..insert_at], &xmp[insert_at..]))
}

/// Escrita atómica: temporário na mesma pasta + rename.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("xmp.pff-tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Escreve/atualiza o `.xmp` de um RAW. Devolve `true` se o ficheiro já existia.
pub fn write_sidecar(original: &Path, rating: u8) -> CoreResult<bool> {
    let side = sidecar_path(original);
    match std::fs::read_to_string(&side) {
        Ok(old) => {
            let new = set_rating(&old, rating).ok_or_else(|| {
                CoreError::Other(
                    "o .xmp existente não é um XMP reconhecível: não foi alterado".into(),
                )
            })?;
            // Cópia só de um .xmp de outro programa (Lightroom, Capture One, câmara), uma vez.
            let ours = old.contains("x:xmptk=\"PFF ProMaster\"");
            let backup = side.with_extension("xmp.pff-original");
            if !ours && !backup.exists() {
                std::fs::copy(&side, &backup)?;
            }
            write_atomic(&side, &new)?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            write_atomic(&side, &new_sidecar(rating))?;
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
}

impl ProjectSession {
    /// Classificações da seleção assistida para o Lightroom (ver o módulo).
    pub fn export_lightroom_ratings(&self, o: &LightroomOptions) -> CoreResult<LightroomReport> {
        let recs = self.list_recommendations()?;
        // Versões principais (as cópias virtuais não têm ficheiro próprio).
        let photos: HashMap<AssetId, (u8, String, String)> = self
            .list_photos()?
            .into_iter()
            .filter(|p| p.virtual_copy.is_none())
            .map(|p| (p.asset, (p.stars, p.file_name, p.format)))
            .collect();
        let mut rep = LightroomReport::default();
        let mut app_stars: HashMap<u8, Vec<AssetId>> = HashMap::new();
        for r in recs {
            let Some(suggested) = stars_for_label(&r.label) else { continue };
            let Some((user_stars, name, format)) = photos.get(&r.asset) else { continue };
            let rating = if o.keep_user_stars && *user_stars > 0 { *user_stars } else { suggested };
            if o.apply_in_app && *user_stars == 0 {
                app_stars.entry(suggested).or_default().push(r.asset);
            }
            let raw_sidecar = format_from_id(format).is_some_and(|f| sidecar_supported(f, name));
            if !raw_sidecar {
                rep.not_raw.push(name.clone());
                continue;
            }
            let Some(orig) = self.catalog().original(r.asset)? else { continue };
            let path = Path::new(&orig.path);
            if !path.is_file() {
                rep.unavailable.push(name.clone());
                continue;
            }
            match write_sidecar(path, rating) {
                Ok(existed) => {
                    if existed {
                        rep.updated += 1;
                    } else {
                        rep.created += 1;
                    }
                    let i = match rating {
                        2 => 0,
                        3 => 1,
                        5 => 2,
                        _ => 3,
                    };
                    rep.by_stars[i] += 1;
                }
                Err(e) => rep.failed.push((name.clone(), e.to_string())),
            }
        }
        for (stars, ids) in app_stars {
            rep.stars_in_app += self.set_stars(&ids, stars)? as u64;
        }
        let _ = self.catalog().log_event(
            "classificacoes_lightroom",
            &format!(
                "{} .xmp criados, {} atualizados, {} não RAW, {} indisponíveis, {} falhas",
                rep.created,
                rep.updated,
                rep.not_raw.len(),
                rep.unavailable.len(),
                rep.failed.len()
            ),
        );
        tracing::info!(
            criados = rep.created,
            atualizados = rep.updated,
            "classificações para o Lightroom"
        );
        Ok(rep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_map_to_the_requested_stars() {
        assert_eq!(stars_for_label("best"), Some(5));
        assert_eq!(stars_for_label("alternative"), Some(3));
        assert_eq!(stars_for_label("review"), Some(2));
        assert_eq!(stars_for_label("reject"), None);
        assert_eq!(stars_for_label("duplicate"), None);
    }

    #[test]
    fn existing_sidecars_only_change_the_rating() {
        // Atributo (forma do Lightroom).
        let lr = "<x:xmpmeta><rdf:RDF><rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmlns:crs=\"http://ns.adobe.com/camera-raw-settings/1.0/\" xmp:Rating=\"1\" crs:Exposure2012=\"+0.50\"/></rdf:RDF></x:xmpmeta>";
        let out = set_rating(lr, 5).unwrap();
        assert!(
            out.contains("xmp:Rating=\"5\"") && out.contains("crs:Exposure2012=\"+0.50\""),
            "{out}"
        );
        // Elemento.
        let el = "<rdf:Description rdf:about=\"\"><xmp:Rating>0</xmp:Rating><dc:x>y</dc:x></rdf:Description>";
        assert_eq!(set_rating(el, 3).unwrap(), el.replace(">0<", ">3<"));
        // Sem classificação nem espaço de nomes xmp: acrescenta os dois, mantém o resto.
        let none = "<x:xmpmeta><rdf:RDF><rdf:Description rdf:about=\"\" xmlns:crs=\"c\" crs:WhiteBalance=\"As Shot\"/></rdf:RDF></x:xmpmeta>";
        let out = set_rating(none, 2).unwrap();
        assert!(
            out.contains("xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"")
                && out.contains("xmp:Rating=\"2\"/>"),
            "{out}"
        );
        assert!(out.contains("crs:WhiteBalance=\"As Shot\""));
        assert!(set_rating("isto não é xmp", 5).is_none());
    }

    #[test]
    fn sidecars_are_written_next_to_the_raw_without_touching_it() {
        let d = tempfile::tempdir().unwrap();
        let raw = d.path().join("IMG_0001.CR3");
        std::fs::write(&raw, b"dados do sensor").unwrap();
        assert!(!write_sidecar(&raw, 5).unwrap());
        let side = d.path().join("IMG_0001.xmp");
        let text = std::fs::read_to_string(&side).unwrap();
        assert!(text.contains("xmp:Rating=\"5\"") && text.starts_with("<?xpacket"));
        assert_eq!(std::fs::read(&raw).unwrap(), b"dados do sensor", "RAW intacto");
        // Um .xmp nosso: atualizado sem cópia.
        assert!(write_sidecar(&raw, 3).unwrap());
        assert!(std::fs::read_to_string(&side).unwrap().contains("xmp:Rating=\"3\""));
        let backup = d.path().join("IMG_0001.xmp.pff-original");
        assert!(!backup.exists());
        // Um .xmp do Lightroom: cópia guardada (uma vez) e só a classificação muda.
        let lr = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"Adobe XMP Core\"><rdf:RDF><rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmp:Rating=\"1\"/></rdf:RDF></x:xmpmeta>";
        std::fs::write(&side, lr).unwrap();
        write_sidecar(&raw, 5).unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), lr);
        write_sidecar(&raw, 2).unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), lr, "cópia só da primeira vez");
        assert!(std::fs::read_to_string(&side).unwrap().contains("xmp:Rating=\"2\""));
        // XMP irreconhecível: recusado, ficheiro intacto.
        std::fs::write(&side, "lixo").unwrap();
        assert!(write_sidecar(&raw, 5).is_err());
        assert_eq!(std::fs::read_to_string(&side).unwrap(), "lixo");
    }
}
