//! Segurança do trabalho (Fase 13): uso exclusivo do projeto, deteção de sessões
//! interrompidas, validação e recuperação do catálogo, cópias de segurança com versões,
//! exportar/importar o catálogo e limpeza de ficheiros temporários.
//!
//! Princípios: nunca apagar o que não se criou; um catálogo danificado é guardado de lado
//! (nunca apagado); cada restauro guarda primeiro o estado atual.

use crate::paths::ProjectPaths;
use momento_core_types::{CoreError, CoreResult};
use momento_database::safety::Validation;
use momento_database::{now_unix, Catalog};
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const LOCK_FILE: &str = "session.lock";

/// Uso exclusivo do projeto enquanto está aberto. O bloqueio do sistema operativo é
/// libertado mesmo que o processo termine de forma inesperada; o conteúdo do ficheiro
/// ("running") é o que denuncia uma sessão que não fechou bem.
pub struct SessionLock {
    file: Option<File>,
    path: PathBuf,
}

impl SessionLock {
    /// Devolve o bloqueio e se a sessão anterior terminou sem fechar o projeto.
    pub fn acquire(paths: &ProjectPaths) -> CoreResult<(Self, bool)> {
        let path = paths.root.join(LOCK_FILE);
        let mut file =
            OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Err(CoreError::ProjectInUse),
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
        let mut prev = String::new();
        let _ = file.read_to_string(&mut prev);
        let unclean = prev.starts_with("running");
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        write!(file, "running {}", now_unix())?;
        file.sync_all()?;
        Ok((SessionLock { file: Some(file), path }, unclean))
    }

    /// Fecho limpo: a próxima abertura não vê uma interrupção.
    pub fn release(mut self) {
        self.release_inner();
    }

    fn release_inner(&mut self) {
        if let Some(f) = self.file.take() {
            drop(f);
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        // `Drop` sem `release` (ex.: pânico): o bloqueio sai, o ficheiro fica "running"
        // → a próxima abertura valida o catálogo por completo.
        self.file.take();
    }
}

/// Uma versão (cópia de segurança) do catálogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub file: String,
    /// Segundos desde 1970.
    pub created: i64,
    pub bytes: u64,
    /// "auto", "manual", "antes-do-restauro", "danificado".
    pub kind: String,
}

fn parse_backup_name(name: &str) -> Option<(i64, String)> {
    let stem = name.strip_suffix(".db")?;
    let rest = stem.strip_prefix("catalog-")?;
    // catalog-<ts>.db | catalog-<tipo>-<ts>.db
    match rest.rsplit_once('-') {
        Some((kind, ts)) => Some((ts.parse().ok()?, kind.to_string())),
        None => Some((rest.parse().ok()?, "auto".to_string())),
    }
}

/// Versões do catálogo, da mais recente para a mais antiga.
pub fn list_backups(paths: &ProjectPaths) -> Vec<BackupInfo> {
    let Ok(rd) = std::fs::read_dir(paths.backups()) else { return Vec::new() };
    let mut v: Vec<BackupInfo> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let (created, kind) = parse_backup_name(&name)?;
            Some(BackupInfo {
                file: e.path().to_string_lossy().into_owned(),
                created,
                bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                kind,
            })
        })
        .collect();
    v.sort_by(|a, b| b.created.cmp(&a.created).then(a.file.cmp(&b.file)));
    v
}

/// Retenção das cópias automáticas: as 10 mais recentes + a última de cada um dos 14 dias
/// anteriores. As manuais, as de antes de um restauro e as danificadas nunca são apagadas
/// automaticamente.
pub fn prune_backups(paths: &ProjectPaths) {
    let auto: Vec<BackupInfo> =
        list_backups(paths).into_iter().filter(|b| b.kind == "auto").collect();
    let now = now_unix();
    let mut days_kept = std::collections::HashSet::new();
    for (i, b) in auto.iter().enumerate() {
        let day = b.created / 86_400;
        let keep = i < 10 || (now - b.created < 14 * 86_400 && days_kept.insert(day));
        if i < 10 {
            days_kept.insert(day);
        }
        if !keep {
            let _ = std::fs::remove_file(&b.file);
        }
    }
}

/// Cópia de segurança do catálogo aberto, **validada** depois de escrita.
pub fn create_backup(
    catalog: &Catalog,
    paths: &ProjectPaths,
    kind: &str,
) -> CoreResult<BackupInfo> {
    let dir = paths.backups();
    std::fs::create_dir_all(&dir)?;
    let now = now_unix();
    let name = if kind == "auto" {
        format!("catalog-{now}.db")
    } else {
        format!("catalog-{kind}-{now}.db")
    };
    let dest = dir.join(name);
    let tmp = dir.join(format!(".backup-{now}-{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    catalog.backup_to(&tmp)?;
    let v = Catalog::open_existing(&tmp).and_then(|c| c.validate(true));
    match v {
        Ok(v) if v.ok() => {}
        Ok(v) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(CoreError::Corrupt(format!(
                "a cópia de segurança não passou a validação: {:?}",
                v.integrity
            )));
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    }
    std::fs::rename(&tmp, &dest)?;
    let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    if kind == "auto" {
        prune_backups(paths);
    }
    Ok(BackupInfo {
        file: dest.to_string_lossy().into_owned(),
        created: now,
        bytes,
        kind: kind.into(),
    })
}

/// Valida um ficheiro de catálogo sem o alterar (cópia, versão suportada, integridade).
pub fn check_catalog_file(file: &Path) -> CoreResult<Validation> {
    // Um catálogo copiado com o projeto aberto pode ter alterações só no `-wal` ao lado:
    // copiar apenas o ficheiro principal perdia-as em silêncio (Fase 15).
    let wal = PathBuf::from(format!("{}-wal", file.to_string_lossy()));
    if std::fs::metadata(&wal).is_ok_and(|m| m.len() > 0) {
        return Err(CoreError::Other(
            "este catálogo tem alterações por consolidar (ficheiro -wal ao lado): abra o projeto original e feche-o, ou use \"Exportar catálogo\"".into(),
        ));
    }
    // Numa cópia temporária: abrir um catálogo antigo aplicaria migrações ao original.
    let dir = tempdir_near(file)?;
    let tmp = dir.join("verificar.db");
    std::fs::copy(file, &tmp)?;
    let res = (|| {
        let c = Catalog::open(&tmp)?;
        if c.project()?.is_none() {
            return Err(CoreError::Corrupt("não é um catálogo do Momento (sem projeto)".into()));
        }
        c.validate(true)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    res
}

fn tempdir_near(file: &Path) -> CoreResult<PathBuf> {
    let base = std::env::temp_dir().join(format!(
        "momento-verificar-{}-{}",
        std::process::id(),
        file.file_name().and_then(|n| n.to_str()).unwrap_or("x").len() as u64 ^ now_unix() as u64
    ));
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

fn sidecars(db: &Path) -> [PathBuf; 2] {
    let s = db.to_string_lossy();
    [PathBuf::from(format!("{s}-wal")), PathBuf::from(format!("{s}-shm"))]
}

/// Guarda o catálogo atual em `backups/` (com o WAL) sem o apagar.
fn set_aside(paths: &ProjectPaths, kind: &str) -> CoreResult<Option<PathBuf>> {
    let cat = paths.catalog();
    if !cat.exists() {
        return Ok(None);
    }
    std::fs::create_dir_all(paths.backups())?;
    let dest = paths.backups().join(format!("catalog-{kind}-{}.db", now_unix()));
    std::fs::rename(&cat, &dest)?;
    for (from, to) in sidecars(&cat).iter().zip(sidecars(&dest).iter()) {
        if from.exists() {
            std::fs::rename(from, to)?;
        }
    }
    Ok(Some(dest))
}

/// Substitui o catálogo do projeto (fechado) por `file`, depois de o validar. O atual fica
/// em `backups/catalog-antes-do-restauro-*.db`. Serve para restaurar uma versão e para
/// importar um catálogo exportado.
pub fn replace_catalog(paths: &ProjectPaths, file: &Path) -> CoreResult<Option<PathBuf>> {
    let v = check_catalog_file(file)?;
    if !v.ok() {
        return Err(CoreError::Corrupt(format!(
            "o catálogo escolhido está danificado: {:?}",
            v.integrity
        )));
    }
    let tmp = paths.root.join(".catalogo-novo.tmp");
    copy_synced(file, &tmp)?;
    let aside = set_aside(paths, "antes-do-restauro")?;
    std::fs::rename(&tmp, paths.catalog())?;
    // A cache é indexada pelo id interno das fotografias: com outro catálogo, os mesmos ids
    // podem ser outras fotografias. É regenerável — apaga-se (ao abrir, as miniaturas em
    // falta voltam a ser geradas).
    if let Err(e) = crate::cache::clear_all(paths) {
        tracing::warn!(erro = %e, "não foi possível limpar a cache depois do restauro");
    }
    Ok(aside)
}

/// Exporta o catálogo (instantâneo consistente, validado) para `dest`.
pub fn export_catalog(catalog: &Catalog, dest: &Path) -> CoreResult<()> {
    if dest.exists() {
        return Err(CoreError::Other("o ficheiro de destino já existe".into()));
    }
    let tmp = dest.with_extension("tmp-export");
    catalog.backup_to(&tmp)?;
    let ok = Catalog::open_existing(&tmp).and_then(|c| c.validate(true)).map(|v| v.ok());
    if !matches!(ok, Ok(true)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(CoreError::Corrupt("a exportação do catálogo não passou a validação".into()));
    }
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// O que aconteceu ao abrir (mostrado ao utilizador e registado no catálogo).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenReport {
    /// A sessão anterior não fechou o projeto (falha, corte de energia, processo morto).
    pub previous_unclean: bool,
    /// Validação feita (rápida ou completa) e o resultado.
    pub validated_full: bool,
    pub validation_ms: u64,
    /// O catálogo estava danificado e foi restaurado desta versão (segundos desde 1970).
    pub restored_from: Option<i64>,
    /// Onde ficou o catálogo danificado (nunca é apagado).
    pub damaged_kept: Option<String>,
    /// Ficheiros temporários de escritas interrompidas removidos.
    pub temp_files_removed: u64,
}

/// Erro de abertura que indica dano no ficheiro (e não versão, espaço ou permissões).
fn is_corruption(e: &CoreError) -> bool {
    match e {
        CoreError::Corrupt(_) => true,
        CoreError::Database(m) => {
            let m = m.to_lowercase();
            m.contains("malformed") || m.contains("not a database") || m.contains("corrupt")
        }
        _ => false,
    }
}

/// Copia e sincroniza com o disco (um corte de energia não deixa uma cópia a meio).
fn copy_synced(from: &Path, to: &Path) -> CoreResult<()> {
    std::fs::copy(from, to)?;
    // Com escrita: no Windows, `sync_all` num ficheiro aberto só para leitura dá "Access is
    // denied" (restaurar versões e substituir o catálogo falhavam).
    std::fs::OpenOptions::new().write(true).open(to)?.sync_all()?;
    Ok(())
}

/// Validação na abertura e recuperação automática a partir da versão válida mais recente.
pub fn validate_or_recover(
    paths: &ProjectPaths,
    full: bool,
    report: &mut OpenReport,
) -> CoreResult<()> {
    let t = std::time::Instant::now();
    let check = Catalog::open(&paths.catalog()).and_then(|c| c.validate(full));
    report.validated_full = full;
    report.validation_ms = t.elapsed().as_millis() as u64;
    let problem = match check {
        Ok(v) if v.ok() => return Ok(()),
        Ok(v) => format!("{:?} (chaves estrangeiras: {})", v.integrity, v.foreign_keys),
        // Só dano real justifica trocar o catálogo. Versão mais recente da aplicação, disco
        // cheio, permissões, ficheiro ocupado: o erro é mostrado tal como é e NADA é
        // trocado — antes, um destes erros punha de lado o catálogo atual e restaurava uma
        // versão antiga, perdendo o trabalho recente (Fase 15).
        Err(e) if is_corruption(&e) => e.to_string(),
        Err(e) => return Err(e),
    };
    tracing::error!(problema = %problem, "catálogo danificado; a procurar uma versão válida");
    for b in list_backups(paths).into_iter().filter(|b| b.kind == "auto" || b.kind == "manual") {
        if check_catalog_file(Path::new(&b.file)).is_ok_and(|v| v.ok()) {
            // Cópia completa e sincronizada ANTES de mexer no atual; troca por `rename`.
            let tmp = paths.root.join(".catalogo-recuperado.tmp");
            copy_synced(Path::new(&b.file), &tmp)?;
            let damaged = set_aside(paths, "danificado")?;
            std::fs::rename(&tmp, paths.catalog())?;
            if let Err(e) = crate::cache::clear_all(paths) {
                tracing::warn!(erro = %e, "não foi possível limpar a cache depois do restauro");
            }
            report.restored_from = Some(b.created);
            report.damaged_kept = damaged.map(|p| p.to_string_lossy().into_owned());
            tracing::warn!(versao = b.created, "catálogo restaurado de uma cópia de segurança");
            return Ok(());
        }
    }
    Err(CoreError::Corrupt(format!(
        "o catálogo está danificado e não há nenhuma cópia de segurança válida ({problem})"
    )))
}

/// Remove ficheiros temporários de escritas atómicas interrompidas (`.<nome>.<pid>-<n>.tmp`)
/// numa pasta e subpastas. Só ficheiros com este padrão exato, criados pela aplicação.
pub fn remove_temp_files(dir: &Path, max_depth: usize) -> u64 {
    fn is_ours(name: &str) -> bool {
        let Some(inner) = name.strip_prefix('.').and_then(|n| n.strip_suffix(".tmp")) else {
            return false;
        };
        let Some((_, tail)) = inner.rsplit_once('.') else { return false };
        let Some((pid, n)) = tail.split_once('-') else { return false };
        !pid.is_empty()
            && pid.bytes().all(|b| b.is_ascii_digit())
            && !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_digit())
    }
    let mut removed = 0;
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let name = e.file_name().to_string_lossy().into_owned();
        if ft.is_dir() && max_depth > 0 {
            removed += remove_temp_files(&e.path(), max_depth - 1);
        } else if ft.is_file() && is_ours(&name) && std::fs::remove_file(e.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// "Eliminar projeto" (privacidade): move a pasta `.momento` inteira (catálogo, cache,
/// rostos, edições, versões) para o Lixo do sistema — recuperável até o Lixo ser esvaziado.
/// O projeto tem de estar fechado. **Nunca toca nos originais**: recusa se a pasta dos
/// originais, algum original ou alguma cópia de segurança da importação estiver dentro da
/// pasta do projeto.
pub fn delete_project(root: &Path) -> CoreResult<()> {
    let paths = ProjectPaths::new(root);
    if !paths.catalog().is_file() {
        return Err(CoreError::Other("a pasta escolhida não é um projeto".into()));
    }
    let (lock, _) = SessionLock::acquire(&paths)?;
    let root_c = root.canonicalize()?;
    let inside = |p: &str| {
        Path::new(p).canonicalize().map(|c| c.starts_with(&root_c)).unwrap_or(false)
            || Path::new(p).starts_with(root)
    };
    {
        let cat = Catalog::open_existing(&paths.catalog())?;
        if let Some(dir) = cat.project()?.and_then(|p| p.originals_dir) {
            if inside(&dir) {
                return Err(CoreError::Other(
                    "a pasta dos originais está dentro do projeto: elimine-o manualmente depois de mover os originais".into(),
                ));
            }
        }
        let originals = cat.originals()?;
        if let Some(o) = originals
            .iter()
            .find(|o| inside(&o.path) || o.backup_path.as_deref().is_some_and(inside))
        {
            return Err(CoreError::Other(format!(
                "há originais dentro da pasta do projeto ({}): nada foi eliminado",
                o.file_name
            )));
        }
    }
    lock.release();
    trash::delete(root).map_err(|e| {
        CoreError::Other(format!("não foi possível mover o projeto para o Lixo: {e}"))
    })?;
    tracing::info!("projeto movido para o Lixo");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_names_and_temp_pattern() {
        assert_eq!(
            parse_backup_name("catalog-1700000000.db"),
            Some((1_700_000_000, "auto".into()))
        );
        assert_eq!(
            parse_backup_name("catalog-manual-1700000000.db"),
            Some((1_700_000_000, "manual".into()))
        );
        assert_eq!(
            parse_backup_name("catalog-antes-do-restauro-17.db"),
            Some((17, "antes-do-restauro".into()))
        );
        assert_eq!(parse_backup_name("notas.txt"), None);
        let d = tempfile::tempdir().unwrap();
        let sub = d.path().join("a");
        std::fs::create_dir_all(&sub).unwrap();
        for n in [".IMG_1.jpg.123-4.tmp", "IMG_1.jpg", ".oculto.tmp", "x.123-4.tmp", ".f.12-.tmp"] {
            std::fs::write(sub.join(n), b"x").unwrap();
        }
        assert_eq!(remove_temp_files(d.path(), 2), 1, "só o temporário da escrita atómica");
        assert!(sub.join("IMG_1.jpg").exists() && sub.join(".oculto.tmp").exists());
    }

    #[test]
    fn lock_detects_in_use_and_unclean_sessions() {
        let d = tempfile::tempdir().unwrap();
        let paths = ProjectPaths::from_user_path(d.path());
        let (lock, unclean) = SessionLock::acquire(&paths).unwrap();
        assert!(!unclean);
        // Outro processo/janela: o bloqueio do sistema impede a segunda abertura.
        assert!(matches!(SessionLock::acquire(&paths), Err(CoreError::ProjectInUse)));
        // Fecho sem `release` (como uma falha): a próxima abertura sabe.
        drop(lock);
        let (lock, unclean) = SessionLock::acquire(&paths).unwrap();
        assert!(unclean, "sessão anterior interrompida");
        lock.release();
        let (_lock, unclean) = SessionLock::acquire(&paths).unwrap();
        assert!(!unclean, "depois de um fecho limpo");
    }
}
