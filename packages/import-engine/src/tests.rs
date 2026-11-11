use super::*;
use momento_image_codecs::default_registry;
use momento_test_fixtures::*;
use std::sync::atomic::AtomicUsize;

struct Env {
    _dir: tempfile::TempDir,
    root: PathBuf,
    catalog: PathBuf,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let catalog = root.join("proj.momento/catalog.db");
    std::fs::create_dir_all(catalog.parent().unwrap()).unwrap();
    Catalog::open(&catalog).unwrap();
    Env { _dir: dir, root, catalog }
}

fn opts(mode: ImportMode, recursive: bool, e: &Env) -> ImportOptions {
    ImportOptions {
        mode,
        recursive,
        originals_dir: Some(e.root.join("originais")),
        project_dir: Some(e.root.join("proj.momento")),
        threads: 4,
        verify: true,
        backup_dir: None,
    }
}

fn make_source(e: &Env, n: u32) -> PathBuf {
    let src = e.root.join("cartao");
    for i in 0..n {
        let o = [1u16, 6, 8, 3][(i % 4) as usize];
        let sub = if i % 2 == 0 { "DCIM/100CANON" } else { "DCIM/101CANON/extra" };
        write(
            &src.join(sub).join(format!("IMG_{i:04}.jpg")),
            &jpeg_bytes(&test_image(120, 80, i, o), Some(&spec_for(i, o)), 80),
        )
        .unwrap();
    }
    write(
        &src.join("DCIM/100CANON/PNG_1.png"),
        &png_bytes(&test_image(60, 40, 99, 1), Some(&spec_for(1, 1))),
    )
    .unwrap();
    write(&src.join("DCIM/100CANON/TIFF_1.tif"), &tiff_bytes(&test_image(60, 40, 98, 1))).unwrap();
    // duplicado exato com outro nome
    std::fs::copy(src.join("DCIM/100CANON/IMG_0000.jpg"), src.join("DCIM/100CANON/COPIA.jpg"))
        .unwrap();
    write(&src.join("DCIM/100CANON/notas.txt"), b"texto").unwrap();
    write(&src.join("DCIM/100CANON/falso.jpg"), b"nao e jpeg").unwrap();
    write(&src.join("DCIM/100CANON/.oculto.jpg"), &jpeg_bytes(&test_image(10, 10, 1, 1), None, 80))
        .unwrap();
    src
}

fn run(e: &Env, paths: &[PathBuf], o: &ImportOptions) -> (SessionCounts, Vec<AssetId>) {
    let mut cat = Catalog::open(&e.catalog).unwrap();
    let (session, _) = create_session(&mut cat, paths, o).unwrap();
    drop(cat);
    let added = Mutex::new(Vec::new());
    let counts = run_session(
        &e.catalog,
        session,
        o,
        &default_registry(),
        &ImportControl::default(),
        &|ev| {
            if let ImportEvent::AssetsAdded(ids) = ev {
                added.lock().unwrap().extend(ids);
            }
        },
    )
    .unwrap();
    (counts, added.into_inner().unwrap())
}

#[test]
fn imports_folder_recursively_with_duplicates_and_unsupported() {
    let e = env();
    let src = make_source(&e, 20);
    let (c, added) = run(&e, std::slice::from_ref(&src), &opts(ImportMode::Reference, true, &e));
    assert_eq!(c.imported, 22, "20 JPEG + 1 PNG + 1 TIFF");
    assert_eq!(c.duplicate, 1, "COPIA.jpg");
    assert_eq!(c.unsupported, 2, "notas.txt e falso.jpg");
    assert_eq!(c.total, 25, "ficheiro oculto ignorado");
    assert_eq!(added.len(), 22);

    let cat = Catalog::open(&e.catalog).unwrap();
    let rows = cat.list_photos().unwrap();
    assert_eq!(rows.len(), 22);
    let d = cat
        .photo_details(rows.iter().find(|r| r.file_name == "IMG_0001.jpg").unwrap().id)
        .unwrap()
        .unwrap();
    assert_eq!(d.orientation, 6);
    assert_eq!((d.width, d.height), (80, 120), "dimensões gravadas antes da orientação");
    assert_eq!(d.camera_model.as_deref(), Some("NIKON Z 6_2"));
    assert_eq!(d.iso, Some(400));
    assert!(d.capture_time.is_some());
    // Comparar caminhos (não texto): no Windows o separador é "\\".
    assert_eq!(
        std::path::Path::new(&d.path),
        src.join("DCIM").join("101CANON").join("extra").join("IMG_0001.jpg")
    );

    // Reimportar a mesma pasta: tudo “já importado”, nada novo.
    let (c2, added2) = run(&e, &[src], &opts(ImportMode::Reference, true, &e));
    assert_eq!(c2.imported, 0);
    // 22 + COPIA.jpg (o mesmo conteúdo de IMG_0000, que já está no catálogo): decide-se
    // pelo conteúdo (Fase 15).
    assert_eq!(c2.already_imported, 23);
    assert!(added2.is_empty());
}

#[test]
fn non_recursive_only_takes_direct_files() {
    let e = env();
    let src = make_source(&e, 6);
    let (c, _) = run(&e, &[src.join("DCIM/100CANON")], &opts(ImportMode::Reference, false, &e));
    // 3 JPEG pares + PNG + TIFF (+ COPIA duplicado, notas, falso)
    assert_eq!(c.imported, 5);
}

#[test]
fn individual_files_are_imported() {
    let e = env();
    let src = make_source(&e, 4);
    let files = vec![
        src.join("DCIM/100CANON/IMG_0000.jpg"),
        src.join("DCIM/100CANON/PNG_1.png"),
        src.join("DCIM/100CANON/notas.txt"),
    ];
    let (c, _) = run(&e, &files, &opts(ImportMode::Reference, false, &e));
    assert_eq!((c.imported, c.unsupported), (2, 1));
}

#[test]
fn copy_mode_copies_verifies_and_keeps_originals_intact() {
    let e = env();
    let src = make_source(&e, 8);
    let original = src.join("DCIM/100CANON/IMG_0000.jpg");
    let before = blake3::hash(&std::fs::read(&original).unwrap());
    let (c, _) = run(&e, std::slice::from_ref(&src), &opts(ImportMode::Copy, true, &e));
    assert_eq!(c.imported, 10);
    assert_eq!(blake3::hash(&std::fs::read(&original).unwrap()), before, "original intacto");
    assert!(original.exists());

    let cat = Catalog::open(&e.catalog).unwrap();
    for r in cat.list_photos().unwrap() {
        let d = cat.photo_details(r.id).unwrap().unwrap();
        assert!(d.path.starts_with(&*e.root.join("originais").to_string_lossy()), "{}", d.path);
        assert!(d.source_path.is_some());
        assert!(Path::new(&d.path).exists());
    }
    // Organização por câmara; dos dois ficheiros idênticos só um é copiado.
    let a = e.root.join("originais/EOS_R5/IMG_0000.jpg").exists();
    let b = e.root.join("originais/EOS_R5/COPIA.jpg").exists();
    assert!(a ^ b, "exatamente uma cópia do conteúdo duplicado");
    assert!(e.root.join("originais/NIKON_Z_6_2/IMG_0001.jpg").exists());
}

#[test]
fn originals_are_never_modified_in_reference_mode() {
    let e = env();
    let src = make_source(&e, 10);
    let files = collect_files(std::slice::from_ref(&src), true, None);
    let before: Vec<_> = files
        .iter()
        .map(|f| (std::fs::read(f).unwrap(), std::fs::metadata(f).unwrap().modified().unwrap()))
        .collect();
    run(&e, &[src], &opts(ImportMode::Reference, true, &e));
    for (f, (bytes, mtime)) in files.iter().zip(before) {
        assert_eq!(std::fs::read(f).unwrap(), bytes);
        assert_eq!(std::fs::metadata(f).unwrap().modified().unwrap(), mtime);
    }
}

#[test]
fn interrupted_import_resumes_pending_items() {
    let e = env();
    let src = make_source(&e, 150);
    let o = ImportOptions { threads: 1, ..opts(ImportMode::Reference, true, &e) };
    let mut cat = Catalog::open(&e.catalog).unwrap();
    let (session, total) = create_session(&mut cat, &[src], &o).unwrap();
    drop(cat);
    // Suspende depois do primeiro lote gravado (simula fecho da aplicação).
    let control = ImportControl::default();
    let seen = AtomicUsize::new(0);
    run_session(&e.catalog, session, &o, &default_registry(), &control, &|ev| {
        if let ImportEvent::Progress(p) = ev {
            if p.processed > 0 && seen.fetch_add(1, Ordering::SeqCst) == 0 {
                control.suspend();
            }
        }
    })
    .unwrap();
    let cat = Catalog::open(&e.catalog).unwrap();
    let counts = cat.import_session_counts(session).unwrap();
    assert!(counts.pending > 0 && counts.pending < total as u64, "{counts:?}");
    // Ao reabrir, a sessão aparece como por terminar e é retomada.
    assert_eq!(cat.unfinished_import_session().unwrap().unwrap().id, session);
    let c = run_session(
        &e.catalog,
        session,
        &o,
        &default_registry(),
        &ImportControl::default(),
        &|_| {},
    )
    .unwrap();
    assert_eq!(c.pending, 0);
    assert_eq!(c.imported, 152);
    assert_eq!(Catalog::open(&e.catalog).unwrap().photo_count().unwrap(), 152);
    assert!(Catalog::open(&e.catalog).unwrap().unfinished_import_session().unwrap().is_none());
}

#[test]
fn cancelled_import_is_not_resumed() {
    let e = env();
    let src = make_source(&e, 100);
    let o = ImportOptions { threads: 1, ..opts(ImportMode::Reference, true, &e) };
    let mut cat = Catalog::open(&e.catalog).unwrap();
    let (session, _) = create_session(&mut cat, &[src], &o).unwrap();
    drop(cat);
    let control = ImportControl::default();
    control.cancel();
    let c = run_session(&e.catalog, session, &o, &default_registry(), &control, &|_| {}).unwrap();
    assert_eq!(c.imported, 0);
    assert!(Catalog::open(&e.catalog).unwrap().unfinished_import_session().unwrap().is_none());
}

#[test]
fn project_folder_is_never_imported() {
    let e = env();
    write(
        &e.root.join("proj.momento/cache/thumbs/00/1.jpg"),
        &jpeg_bytes(&test_image(10, 10, 1, 1), None, 80),
    )
    .unwrap();
    write(
        &e.root.join("outro.momento/cache/x.jpg"),
        &jpeg_bytes(&test_image(10, 10, 2, 1), None, 80),
    )
    .unwrap();
    write(&e.root.join("foto.jpg"), &jpeg_bytes(&test_image(10, 10, 3, 1), None, 80)).unwrap();
    let files =
        collect_files(std::slice::from_ref(&e.root), true, Some(&e.root.join("proj.momento")));
    assert_eq!(files, vec![e.root.join("foto.jpg")]);
}

/// Fase 2: estados de importação com ficheiros de vários formatos.
#[test]
fn states_for_new_formats_missing_codec_and_corrupt() {
    let e = env();
    let src = e.root.join("mistura");
    let img = test_image(200, 150, 3, 1);
    let enc = |f: image::ImageFormat| {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img.clone()).write_to(&mut out, f).unwrap();
        out.into_inner()
    };
    write(&src.join("a.webp"), &enc(image::ImageFormat::WebP)).unwrap();
    write(&src.join("b.bmp"), &enc(image::ImageFormat::Bmp)).unwrap();
    write(&src.join("c.gif"), &enc(image::ImageFormat::Gif)).unwrap();
    write(&src.join("d.tga"), &enc(image::ImageFormat::Tga)).unwrap();
    // HEIC: cabeçalho ISOBMFF válido, mas sem descodificador nesta plataforma (Linux).
    let mut heic = vec![0, 0, 0, 24];
    heic.extend_from_slice(b"ftypheic\0\0\0\0mif1heic");
    heic.resize(4096, 0);
    write(&src.join("e.heic"), &heic).unwrap();
    // RW2 truncado: assinatura Panasonic e mais nada.
    let mut rw2 = b"IIU\0\x08\0\0\0".to_vec();
    rw2.resize(2048, 0);
    write(&src.join("f.rw2"), &rw2).unwrap();
    // PSD com cabeçalho válido e dados truncados.
    let mut psd = b"8BPS\0\x01\0\0\0\0\0\0\0\x03\0\0\0\x10\0\0\0\x10\0\x08\0\x03".to_vec();
    psd.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    write(&src.join("g.psd"), &psd).unwrap();
    write(
        &src.join("h.svg"),
        b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    )
    .unwrap();

    let mut reg = momento_image_codecs::default_registry();
    reg.register(std::sync::Arc::new(momento_raw_engine::RawCodec::new(
        momento_image_codecs::default_registry(),
    )));
    let o = opts(ImportMode::Reference, true, &e);
    let mut cat = Catalog::open(&e.catalog).unwrap();
    let (session, _) = create_session(&mut cat, std::slice::from_ref(&src), &o).unwrap();
    drop(cat);
    let c = run_session(&e.catalog, session, &o, &reg, &ImportControl::default(), &|_| {}).unwrap();
    assert_eq!(c.imported, 4, "{c:?}");
    assert_eq!(c.missing_codec, 1, "{c:?}");
    assert_eq!(c.corrupt, 2, "RW2 e PSD truncados: {c:?}");
    assert_eq!(c.unsupported, 1, "SVG não é fotografia: {c:?}");

    let cat = Catalog::open(&e.catalog).unwrap();
    let issues = cat.import_session_issues(session, 10).unwrap();
    let msg = |name: &str| {
        issues.iter().find(|i| i.source_path.ends_with(name)).unwrap().message.clone().unwrap()
    };
    assert!(msg("e.heic").contains("sem descodificador"));
    assert!(msg("f.rw2").contains("incompleto ou danificado"));
    assert!(msg("h.svg").contains("não é uma fotografia"));
    let gif = cat.list_photos().unwrap().into_iter().find(|p| p.file_name == "c.gif").unwrap();
    assert!(gif.partial, "GIF: só o primeiro fotograma");
    let d = cat.photo_details(gif.id).unwrap().unwrap();
    assert_eq!(d.codec.as_deref(), Some("raster"));
    assert_eq!(d.bit_depth, Some(8));
}

/// Fase 15 (crítico): um segundo cartão montado no mesmo caminho, com os mesmos nomes
/// (a câmara recomeça em IMG_0001), tem fotografias DIFERENTES — têm de entrar.
/// Antes eram ignoradas como "já importadas" só pelo caminho.
#[test]
fn second_card_at_same_path_with_same_names_is_imported() {
    let e = env();
    let card = e.root.join("cartao");
    let shoot = |seed: u32| {
        for i in 0..4 {
            write(
                &card.join(format!("DCIM/100CANON/IMG_{i:04}.jpg")),
                &jpeg_bytes(&test_image(120, 80, seed + i, 1), Some(&spec_for(i, 1)), 80),
            )
            .unwrap();
        }
    };
    shoot(0);
    let o = opts(ImportMode::Copy, true, &e);
    let (c1, _) = run(&e, std::slice::from_ref(&card), &o);
    assert_eq!(c1.imported, 4);
    // Cartão formatado / segundo fotógrafo: mesmos caminhos, outro conteúdo.
    std::fs::remove_dir_all(&card).unwrap();
    shoot(100);
    let (c2, _) = run(&e, std::slice::from_ref(&card), &o);
    assert_eq!((c2.imported, c2.already_imported), (4, 0), "{c2:?}");
    // O mesmo cartão outra vez: agora sim, já importadas (pelo conteúdo).
    let (c3, _) = run(&e, std::slice::from_ref(&card), &o);
    assert_eq!((c3.imported, c3.already_imported), (0, 4), "{c3:?}");
    let copies = walkdir::WalkDir::new(e.root.join("originais"))
        .into_iter()
        .flatten()
        .filter(|x| x.file_type().is_file())
        .count();
    assert_eq!(copies, 8, "as 8 fotografias diferentes têm cópia própria");
}

/// Fase 15: caminho inexistente e nomes de câmara perigosos ("..") não se perdem nem
/// escapam da pasta dos originais.
#[test]
fn missing_paths_are_reported_and_camera_names_cannot_escape() {
    let e = env();
    let src = e.root.join("fonte");
    let spec = ExifSpec { model: Some("..".into()), make: Some("..".into()), ..spec_for(1, 1) };
    write(&src.join("IMG_1.jpg"), &jpeg_bytes(&test_image(120, 80, 1, 1), Some(&spec), 80))
        .unwrap();
    let o = opts(ImportMode::Copy, true, &e);
    let (c, _) = run(&e, &[src.clone(), e.root.join("nao-existe")], &o);
    assert_eq!((c.imported, c.error), (1, 1), "{c:?}");
    let copied: Vec<PathBuf> = walkdir::WalkDir::new(&e.root)
        .into_iter()
        .flatten()
        .filter(|x| x.file_type().is_file() && x.file_name() == "IMG_1.jpg")
        .map(|x| x.into_path())
        .collect();
    assert!(copied.iter().any(|p| p.starts_with(e.root.join("originais"))), "{copied:?}");
    assert_eq!(copied.len(), 2, "original + cópia dentro de originais: {copied:?}");
}

/// Fase 15 (encontrado na aplicação real): no modo "manter no local", o MESMO caminho com
/// OUTRO conteúdo (cartão reutilizado) não é "já importada" — é um erro explicado.
#[test]
fn reference_mode_same_path_new_content_is_reported() {
    let e = env();
    let card = e.root.join("cartao");
    let shoot = |seed: u32| {
        for i in 0..3 {
            write(
                &card.join(format!("IMG_{i:04}.jpg")),
                &jpeg_bytes(&test_image(120, 80, seed + i, 1), Some(&spec_for(i, 1)), 80),
            )
            .unwrap();
        }
    };
    shoot(0);
    let o = opts(ImportMode::Reference, true, &e);
    let (c1, _) = run(&e, std::slice::from_ref(&card), &o);
    assert_eq!(c1.imported, 3);
    shoot(500);
    let (c2, _) = run(&e, std::slice::from_ref(&card), &o);
    assert_eq!((c2.imported, c2.already_imported, c2.error), (0, 0, 3), "{c2:?}");
}
