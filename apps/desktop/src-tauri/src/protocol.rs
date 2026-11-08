//! Protocolo `momento://` para servir imagens à interface sem base64 nem JSON.
//!
//! URL: `momento://localhost/<token>/<nível>/<id>` (no Windows:
//! `http://momento.localhost/…`). Só serve ficheiros identificados por ID de fotografia
//! do projeto aberto — nunca caminhos arbitrários.

use crate::state::AppState;
use momento_core_types::AssetId;
use momento_project_management::paths::CacheLevel;
use momento_project_management::previews;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

fn respond(status: StatusCode, mime: &str, body: Vec<u8>, cache: bool) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(
            header::CACHE_CONTROL,
            if cache { "private, max-age=31536000, immutable" } else { "no-store" },
        )
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn not_found(msg: &str) -> Response<Vec<u8>> {
    respond(StatusCode::NOT_FOUND, "text/plain; charset=utf-8", msg.as_bytes().to_vec(), false)
}

/// Interpreta `/<token>/<nível>/<id>`.
pub fn parse(path: &str) -> Option<(u64, CacheLevel, AssetId)> {
    let mut it = path.trim_start_matches('/').split('/');
    let token = it.next()?.parse().ok()?;
    let level = match it.next()? {
        "thumb" => CacheLevel::Thumb,
        "preview" => CacheLevel::Preview,
        "full" => CacheLevel::Full,
        _ => return None,
    };
    let id = AssetId(it.next()?.parse().ok()?);
    if it.next().is_some() {
        return None;
    }
    Some((token, level, id))
}

pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let path = request.uri().path().to_string();
    // Trabalho fora da thread da interface: a geração de uma preview pode demorar.
    std::thread::spawn(move || {
        let Some((token, level, id)) = parse(&path) else {
            return responder.respond(not_found("pedido inválido"));
        };
        let state = app.state::<AppState>();
        if state.token() != Some(token) {
            return responder.respond(not_found("projeto não está aberto"));
        }
        let Ok(session) = state.session() else {
            return responder.respond(not_found("projeto não está aberto"));
        };
        let res = previews::ensure(&session.paths, session.registry(), id, level)
            .and_then(|s| Ok((std::fs::read(&s.path)?, s.mime)));
        responder.respond(match res {
            Ok((bytes, mime)) => respond(StatusCode::OK, mime, bytes, true),
            Err(e) => not_found(&e.to_string()),
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_valid_paths() {
        assert_eq!(parse("/3/thumb/42"), Some((3, CacheLevel::Thumb, AssetId(42))));
        assert_eq!(parse("/3/full/1"), Some((3, CacheLevel::Full, AssetId(1))));
        assert_eq!(parse("/3/thumb/42/x"), None);
        assert_eq!(parse("/3/../../etc/passwd"), None);
        assert_eq!(parse("/x/thumb/1"), None);
        assert_eq!(parse("/1/original/1"), None);
    }
}
