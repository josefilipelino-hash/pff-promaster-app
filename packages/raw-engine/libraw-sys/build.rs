//! Compila a LibRaw (submódulo `LibRaw/`, versão fixada) e a camada `shim/` com o `cc`,
//! de forma idêntica em macOS (clang), Windows (MSVC) e Linux (gcc/clang).
//!
//! `patches/LibRaw/` guarda ficheiros da LibRaw corrigidos por nós (ex.: suporte da Canon EOS
//! C50 retroportado da LibRaw atual). A LibRaw é copiada para a pasta de compilação e esses
//! ficheiros substituem os originais — o submódulo fica intacto.

use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).expect("pasta da LibRaw") {
        let p = e.expect("entrada").path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "cpp") {
            let name = p.file_stem().unwrap().to_string_lossy();
            // `*_ph.cpp` são marcadores vazios fora da lista oficial de objetos (Makefile.dist).
            if !name.ends_with("_ph") {
                out.push(p);
            }
        }
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("criar pasta");
    for e in std::fs::read_dir(from).expect("ler pasta") {
        let p = e.expect("entrada").path();
        let dest = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_tree(&p, &dest);
        } else {
            std::fs::copy(&p, &dest).expect("copiar ficheiro");
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let original = root.join("LibRaw");
    if !original.join("libraw/libraw.h").exists() {
        panic!("Código-fonte da LibRaw em falta. Execute: git submodule update --init --recursive");
    }
    let libraw = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("LibRaw");
    let _ = std::fs::remove_dir_all(&libraw);
    for dir in ["src", "internal", "libraw"] {
        copy_tree(&original.join(dir), &libraw.join(dir));
    }
    copy_tree(&root.join("patches/LibRaw"), &libraw);
    let mut sources = Vec::new();
    collect(&libraw.join("src"), &mut sources);
    sources.sort();

    let mut b = cc::Build::new();
    b.cpp(true)
        .files(&sources)
        .file(root.join("shim/momento_libraw.cpp"))
        .include(&libraw)
        .include(root.join("shim"))
        .define("LIBRAW_NODLL", None) // biblioteca estática (no Windows evita dllimport)
        .define("USE_ZLIB", None) // DNG com compressão deflate (ex.: DNG de vírgula flutuante)
        .define("USE_X3FTOOLS", None) // Sigma X3F (desativado por omissão na LibRaw)
        // M_PI, M_SQRT1_2…: no Windows (MSVC e MinGW) só são definidos com esta macro.
        .define("_USE_MATH_DEFINES", None)
        .warnings(false)
        .opt_level(3);
    if let Ok(inc) = std::env::var("DEP_Z_INCLUDE") {
        b.include(inc);
    }
    let target = std::env::var("TARGET").unwrap();
    if target.contains("msvc") {
        b.flag("/EHsc").flag("/std:c++14").define("_CRT_SECURE_NO_WARNINGS", None);
    } else {
        // gnu++14 (e não c++14): o modo ISO estrito esconde as constantes matemáticas POSIX.
        b.flag_if_supported("-std=gnu++14").flag_if_supported("-w");
    }
    b.compile("momento_libraw");

    println!("cargo:rerun-if-changed=shim");
    println!("cargo:rerun-if-changed=LibRaw/src");
    println!("cargo:rerun-if-changed=LibRaw/internal");
    println!("cargo:rerun-if-changed=LibRaw/libraw");
    println!("cargo:rerun-if-changed=patches");
    println!("cargo:rerun-if-changed=build.rs");
}
