//! Crash logs (Fase 13): `logs/crash-<data>.txt`, para diagnóstico.
//!
//! Nunca levam fotografias, miniaturas, metadados nem dados de clientes: os caminhos e os
//! emails são removidos (`momento_core_types::redact`). Ficam só no computador — nada é
//! enviado. Guardam-se os 20 mais recentes.

use momento_core_types::redact::redact;
use std::path::{Path, PathBuf};

const KEEP: usize = 20;

fn header(kind: &str) -> String {
    format!(
        "PFF ProMaster — {kind}\nversão: {}\nsistema: {} {}\nquando (UTC, segundos desde 1970): {}\n",
        momento_core_types::CORE_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
        momento_database::now_unix()
    )
}

fn write(dir: &Path, body: &str) -> Option<PathBuf> {
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join(format!("crash-{}.txt", momento_database::now_unix()));
    std::fs::write(&path, redact(body)).ok()?;
    // Retenção.
    let mut all: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt"))
        })
        .collect();
    all.sort();
    while all.len() > KEEP {
        let _ = std::fs::remove_file(all.remove(0));
    }
    Some(path)
}

/// Últimas linhas do log da aplicação (já anonimizadas na escrita do relatório). Os
/// ficheiros chamam-se `momento.<AAAA-MM-DD>.log` (rotação diária, ver `lib.rs`); o
/// formato antigo `momento.log.<data>` também é aceite.
fn log_tail(dir: &Path, lines: usize) -> String {
    let mut logs: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("momento.") && n.contains(".log"))
                })
                .collect()
        })
        .unwrap_or_default();
    logs.sort();
    let Some(last) = logs.last() else { return String::new() };
    let text = std::fs::read_to_string(last).unwrap_or_default();
    let v: Vec<&str> = text.lines().collect();
    v[v.len().saturating_sub(lines)..].join("\n")
}

/// Escritor dos registos (`logs/momento.<data>.log`): cada linha passa por `redact` antes de
/// chegar ao disco — caminhos (que têm nomes de clientes) e emails nunca ficam nos registos.
pub struct Redacting<W: std::io::Write>(pub W);

impl<W: std::io::Write> std::io::Write for Redacting<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write_all(redact(&String::from_utf8_lossy(buf)).as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Pânico em Rust: mensagem, local e pilha de chamadas (só nomes de funções).
pub fn install(dir: PathBuf) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let bt = std::backtrace::Backtrace::force_capture();
        let body = format!(
            "{}thread: {}\n{}\n\npilha:\n{bt}\n",
            header("falha interna (pânico)"),
            thread.name().unwrap_or("?"),
            info
        );
        if let Some(p) = write(&dir, &body) {
            tracing::error!(relatorio = %p.file_name().and_then(|n| n.to_str()).unwrap_or(""), "falha interna registada");
        }
        default(info);
    }));
}

/// A sessão anterior não fechou o projeto (falha nativa, energia, processo morto): relatório
/// com o que se sabe e o fim do log anterior.
pub fn report_unclean(
    dir: &Path,
    report: &momento_project_management::safety::OpenReport,
) -> Option<PathBuf> {
    let body = format!(
        "{}O projeto não foi fechado na sessão anterior (falha, corte de energia ou processo terminado).\n\
         validação completa do catálogo: {} ms; restaurado de uma cópia: {}; temporários removidos: {}\n\n\
         fim do log anterior:\n{}\n",
        header("encerramento inesperado"),
        report.validation_ms,
        report.restored_from.map(|t| t.to_string()).unwrap_or_else(|| "não".into()),
        report.temp_files_removed,
        log_tail(dir, 60)
    );
    write(dir, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_lines_never_carry_paths_or_emails() {
        use std::io::Write;
        let mut out = Vec::new();
        Redacting(&mut out)
            .write_all(
                "INFO projeto aberto projeto=/Users/ana/Casamentos/2026-06-20_Inês_e_João.momento cliente=ines@example.pt\n"
                    .as_bytes(),
            )
            .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("projeto aberto") && text.contains("<caminho>"), "{text}");
        assert!(
            !text.contains("Inês") && !text.contains("/Users") && !text.contains("ines@"),
            "{text}"
        );
    }

    #[test]
    fn reports_are_written_without_personal_paths_and_rotated() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("momento.2026-11-09.log"), "INFO dia anterior\n").unwrap();
        std::fs::write(
            d.path().join("momento.2026-11-10.log"),
            "INFO projeto aberto projeto=/Users/ana/Casamentos/Silva.momento\nERROR original não encontrado: /Volumes/Disco/Silva/IMG_1.CR3\n",
        )
        .unwrap();
        let r = momento_project_management::safety::OpenReport {
            previous_unclean: true,
            ..Default::default()
        };
        let p = report_unclean(d.path(), &r).unwrap();
        let text = std::fs::read_to_string(p).unwrap();
        assert!(text.contains("encerramento inesperado") && text.contains("<caminho>"), "{text}");
        assert!(!text.contains("dia anterior"), "deve usar o log mais recente: {text}");
        assert!(!text.contains("Silva") && !text.contains("/Users"), "{text}");
        for i in 0..25 {
            std::fs::write(d.path().join(format!("crash-{:04}.txt", i)), "x").unwrap();
        }
        write(d.path(), "y");
        let n = std::fs::read_dir(d.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
            .count();
        assert_eq!(n, KEEP);
    }
}
