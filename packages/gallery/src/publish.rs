//! Publicação — **arquitetura** para os fornecedores futuros (S3, Cloudflare R2,
//! Backblaze B2, SFTP). Nesta fase nenhum serviço é contactado: a galeria é uma pasta
//! pronta a publicar.
//!
//! O que já existe e está testado: o **plano de envio** (cada ficheiro com caminho,
//! tamanho, SHA-256, tipo MIME e `Cache-Control`), a **comparação incremental** com o que
//! já foi publicado (enviar só o que mudou, apagar o que saiu) e a validação das
//! configurações. Credenciais nunca vão na configuração: só o **nome** de uma entrada no
//! porta-chaves do sistema (`credential_ref`).

use momento_core_types::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    S3,
    R2,
    B2,
    Sftp,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 4] =
        [ProviderKind::S3, ProviderKind::R2, ProviderKind::B2, ProviderKind::Sftp];
}

/// Configuração de um destino. Sem segredos: `credential_ref` aponta para o porta-chaves.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderConfig {
    pub kind: Option<ProviderKind>,
    /// S3/R2/B2: endpoint (ex.: `https://<conta>.r2.cloudflarestorage.com`).
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    /// Prefixo/pasta remota.
    pub prefix: String,
    /// SFTP.
    pub host: String,
    pub port: u16,
    pub user: String,
    /// Endereço público onde a galeria fica visível.
    pub public_url: String,
    /// Nome da entrada no porta-chaves (Keychain/Credential Manager), nunca a chave.
    pub credential_ref: String,
}

impl ProviderConfig {
    /// Erros de configuração (chaves i18n).
    pub fn validate(&self) -> Vec<&'static str> {
        let mut e = Vec::new();
        let Some(kind) = self.kind else {
            return vec!["gallery.publish.noKind"];
        };
        match kind {
            ProviderKind::S3 | ProviderKind::R2 | ProviderKind::B2 => {
                if self.bucket.trim().is_empty() {
                    e.push("gallery.publish.bucket");
                }
                if kind != ProviderKind::S3 && !self.endpoint.starts_with("https://") {
                    e.push("gallery.publish.endpoint");
                }
            }
            ProviderKind::Sftp => {
                if self.host.trim().is_empty() || self.user.trim().is_empty() {
                    e.push("gallery.publish.host");
                }
            }
        }
        if self.prefix.contains("..") {
            e.push("gallery.publish.prefix");
        }
        if self.credential_ref.trim().is_empty() {
            e.push("gallery.publish.credential");
        }
        e
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadItem {
    /// Caminho relativo com "/".
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub content_type: String,
    pub cache_control: String,
}

pub fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        "zip" => "application/zip",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Imagens, fontes e ZIP têm nomes aleatórios que nunca mudam: cache longa; HTML e dados
/// mudam a cada geração: sem cache.
pub fn cache_control(path: &str) -> &'static str {
    if path.ends_with(".html") || path.starts_with("data/") || path.starts_with("assets/gallery.") {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    }
}

/// Plano de envio de uma pasta de galeria (ficheiros ocultos excluídos).
pub fn plan(root: &Path) -> CoreResult<Vec<UploadItem>> {
    let mut out = Vec::new();
    for e in walkdir::WalkDir::new(root).sort_by_file_name().into_iter().flatten() {
        if !e.file_type().is_file() {
            continue;
        }
        let rel = e
            .path()
            .strip_prefix(root)
            .map_err(|_| CoreError::Other("caminho".into()))?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        if rel.iter().any(|c| c.starts_with('.') || c.ends_with(".tmp")) {
            continue;
        }
        let path = rel.join("/");
        let mut h = sha2::Sha256::new();
        let mut f = std::fs::File::open(e.path())?;
        let mut buf = vec![0u8; 1 << 16];
        let mut bytes = 0u64;
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
            bytes += n as u64;
        }
        out.push(UploadItem {
            content_type: content_type(&path).into(),
            cache_control: cache_control(&path).into(),
            sha256: h.finalize().iter().map(|b| format!("{b:02x}")).collect(),
            path,
            bytes,
        });
    }
    Ok(out)
}

/// Diferenças entre o plano local e o que está publicado: (enviar, apagar).
pub fn diff(local: &[UploadItem], remote: &[UploadItem]) -> (Vec<UploadItem>, Vec<String>) {
    let r: HashMap<&str, &str> =
        remote.iter().map(|i| (i.path.as_str(), i.sha256.as_str())).collect();
    let l: HashMap<&str, ()> = local.iter().map(|i| (i.path.as_str(), ())).collect();
    let send = local
        .iter()
        .filter(|i| r.get(i.path.as_str()) != Some(&i.sha256.as_str()))
        .cloned()
        .collect();
    let delete = remote
        .iter()
        .filter(|i| !l.contains_key(i.path.as_str()))
        .map(|i| i.path.clone())
        .collect();
    (send, delete)
}

/// Um destino de publicação.
pub trait Provider {
    fn kind(&self) -> ProviderKind;
    /// Lista do que já está publicado (para o envio incremental).
    fn remote_state(&self) -> CoreResult<Vec<UploadItem>>;
    fn upload(
        &self,
        root: &Path,
        items: &[UploadItem],
        progress: &dyn Fn(u64, u64),
    ) -> CoreResult<()>;
    fn delete(&self, paths: &[String]) -> CoreResult<()>;
}

/// Nenhum fornecedor está implementado nesta fase (a interface mostra-os como futuros).
pub fn provider_for(cfg: &ProviderConfig) -> CoreResult<Box<dyn Provider>> {
    Err(CoreError::Other(format!(
        "publicação em {:?} ainda não disponível: exporte a pasta e publique-a manualmente",
        cfg.kind
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_diff_and_validation() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("img/t")).unwrap();
        std::fs::create_dir_all(d.path().join("data")).unwrap();
        std::fs::write(d.path().join("index.html"), "<html>").unwrap();
        std::fs::write(d.path().join("img/t/abc.jpg"), b"jpeg").unwrap();
        std::fs::write(d.path().join("data/gallery.js"), "x").unwrap();
        std::fs::write(d.path().join(".momento-gallery"), "m").unwrap();
        let p = plan(d.path()).unwrap();
        let paths: Vec<&str> = p.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(
            paths,
            ["data/gallery.js", "img/t/abc.jpg", "index.html"],
            "sem ficheiros ocultos"
        );
        let jpg = &p[1];
        assert_eq!(jpg.content_type, "image/jpeg");
        assert!(jpg.cache_control.contains("immutable"));
        assert_eq!(p[2].cache_control, "no-cache");
        assert_eq!(jpg.sha256.len(), 64);
        // Incremental: só o que mudou; apaga o que saiu.
        let mut remote = p.clone();
        remote[2].sha256 = "0".repeat(64);
        remote.push(UploadItem { path: "img/t/old.jpg".into(), ..p[1].clone() });
        let (send, delete) = diff(&p, &remote);
        assert_eq!(send.iter().map(|i| i.path.as_str()).collect::<Vec<_>>(), ["index.html"]);
        assert_eq!(delete, ["img/t/old.jpg"]);
        // Configurações: sem segredos, só referências.
        let r2 = ProviderConfig {
            kind: Some(ProviderKind::R2),
            endpoint: "https://conta.r2.cloudflarestorage.com".into(),
            bucket: "galerias".into(),
            credential_ref: "momento-r2".into(),
            ..Default::default()
        };
        assert!(r2.validate().is_empty());
        let sftp = ProviderConfig { kind: Some(ProviderKind::Sftp), ..Default::default() };
        assert_eq!(sftp.validate(), ["gallery.publish.host", "gallery.publish.credential"]);
        assert!(provider_for(&r2).is_err(), "nenhum serviço contactado nesta fase");
    }
}
