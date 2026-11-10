//! Benchmarks da Fase 12 pelo caminho real da aplicação (sessão, catálogo, filas).
//!
//! ```text
//! perf_bench import  <fotos> <trabalho>      importação + miniaturas (tempo, débito, RAM)
//! perf_bench open    <projeto>               abrir + consultas que a interface faz ao abrir
//! perf_bench thumbs  <projeto> <n>           servir miniaturas já em cache (como no scroll)
//! perf_bench preview <projeto> <n>           preview a frio (sem cache) e a quente
//! perf_bench priority <projeto> <n>          preview da fotografia vista com/sem trabalho em fundo
//! perf_bench raw     <projeto>               RAW: preview e revelação completa
//! perf_bench ai      <projeto> <segundos>    análise de IA (fotografias/s, s/fotografia)
//! perf_bench export  <projeto> <n> <destino> exportações por minuto (JPEG alta resolução)
//! ```
//! Resultados numa linha `RESULTADO chave=valor` por medição (para o PERFORMANCE.md).

use momento_core_types::AssetId;
use momento_database::assets::PhotoRow;
use momento_database::import::ImportMode;
use momento_project_management::export::{ExportRequest, JobState};
use momento_project_management::paths::CacheLevel;
use momento_project_management::*;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Ev {
    thumbs_done: AtomicU64,
}
impl SessionEvents for Ev {
    fn import_progress(&self, _: &momento_import_engine::ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, b: &thumbnails::ThumbBatch) {
        self.thumbs_done.store(b.done, Ordering::Relaxed);
    }
}

fn status(key: &str) -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        .unwrap_or(0)
        / 1024
}

fn mem(tag: &str) {
    println!(
        "RESULTADO {tag}_rss_mb={} {tag}_pico_mb={} {tag}_anon_mb={} {tag}_ficheiros_mb={}",
        status("VmRSS:"),
        status("VmHWM:"),
        status("RssAnon:"),
        status("RssFile:")
    );
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    if v.is_empty() {
        return 0.0;
    }
    v[((v.len() - 1) as f64 * p).round() as usize]
}

fn report(tag: &str, v: &mut [f64]) {
    let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
    println!(
        "RESULTADO {tag}_n={} {tag}_media_ms={mean:.2} {tag}_p50_ms={:.2} {tag}_p95_ms={:.2} {tag}_max_ms={:.2}",
        v.len(),
        pct(v, 0.5),
        pct(v, 0.95),
        pct(v, 1.0)
    );
}

fn open(root: &str) -> (Arc<ProjectSession>, Arc<Ev>) {
    let ev = Arc::new(Ev::default());
    let s = ProjectSession::open(Path::new(root), ev.clone()).expect("abrir projeto");
    (s, ev)
}

/// Espalha `n` chaves pelo projeto (de forma determinística).
fn spread(rows: &[PhotoRow], n: usize, filter: impl Fn(&PhotoRow) -> bool) -> Vec<i64> {
    let pool: Vec<&PhotoRow> = rows.iter().filter(|r| filter(r)).collect();
    let step = (pool.len() / n.max(1)).max(1);
    pool.iter().step_by(step).take(n).map(|r| r.id.0).collect()
}

fn drop_cache(s: &Arc<ProjectSession>, key: i64, level: CacheLevel) {
    if let Ok(p) = s.serve_image(key, level) {
        let _ = std::fs::remove_file(p.path);
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t0 = Instant::now();
    match a.get(1).map(String::as_str) {
        Some("import") => {
            let paths = create_project(&NewProject {
                name: "Casamento 15k".into(),
                client_name: "Teste".into(),
                event_type: "wedding".into(),
                event_date: Some("2026-06-15".into()),
                location: String::new(),
                originals_dir: None,
                work_dir: a[3].clone(),
            })
            .unwrap();
            let (s, ev) = open(paths.root.to_str().unwrap());
            let t = Instant::now();
            s.start_import(&ImportRequest {
                paths: vec![a[2].clone()],
                mode: ImportMode::Reference,
                recursive: true,
            })
            .unwrap();
            s.wait_import();
            let imp = t.elapsed();
            let n = s.list_photos().unwrap().len();
            println!(
                "RESULTADO importacao_s={:.1} fotografias={n} importacao_por_s={:.0}",
                imp.as_secs_f64(),
                n as f64 / imp.as_secs_f64()
            );
            mem("depois_importacao");
            let t = Instant::now();
            while s.thumbnails_remaining() > 0 {
                std::thread::sleep(Duration::from_millis(100));
            }
            let th = t.elapsed() + imp;
            println!(
                "RESULTADO miniaturas_total_s={:.1} miniaturas_por_s={:.0} (desde o inicio da importacao; feitas={})",
                th.as_secs_f64(),
                n as f64 / th.as_secs_f64(),
                ev.thumbs_done.load(Ordering::Relaxed)
            );
            mem("depois_miniaturas");
            println!("PROJETO {}", paths.root.display());
            s.close();
        }
        Some("open") => {
            let (s, _) = open(&a[2]);
            println!("RESULTADO abrir_projeto_ms={:.1}", ms(t0.elapsed()));
            mem("aberto");
            let run = |name: &str, f: &dyn Fn() -> usize| {
                let mut v = Vec::new();
                let mut size = 0;
                for _ in 0..5 {
                    let t = Instant::now();
                    size = f();
                    v.push(ms(t.elapsed()));
                }
                println!(
                    "RESULTADO consulta_{name}_ms={:.1} (melhor de 5: {:.1}) bytes_json={size}",
                    v[0],
                    v.iter().cloned().fold(f64::MAX, f64::min)
                );
            };
            let json = |v: &dyn erased::Ser| v.len_json();
            run("list_photos", &|| json(&s.list_photos().unwrap()));
            run("asset_moments", &|| json(&s.asset_moments().unwrap()));
            run("list_moments", &|| json(&s.list_moments().unwrap()));
            run("analysis_status", &|| json(&s.analysis_status().unwrap()));
            run("list_recommendations", &|| json(&s.list_recommendations().unwrap()));
            run("list_groups", &|| json(&s.list_groups().unwrap()));
            run("list_persons", &|| json(&s.list_persons().unwrap()));
            run("photo_persons", &|| json(&s.photo_persons().unwrap()));
            let ids: Vec<AssetId> =
                s.list_photos().unwrap().iter().take(200).map(|r| r.asset).collect();
            let t = Instant::now();
            s.set_stars(&ids, 3).unwrap();
            println!("RESULTADO estrelas_200_ms={:.1}", ms(t.elapsed()));
            let t = Instant::now();
            for id in ids.iter().take(50) {
                s.set_flag(&[*id], 1).unwrap();
            }
            println!("RESULTADO flag_individual_media_ms={:.2}", ms(t.elapsed()) / 50.0);
            s.set_stars(&ids, 0).unwrap();
            for id in ids.iter().take(50) {
                s.set_flag(&[*id], 0).unwrap();
            }
            mem("depois_consultas");
            s.close();
        }
        Some("register") => {
            // Regista o projeto na lista de recentes de uma biblioteca (aplicação real).
            let (s, _) = open(&a[2]);
            let lib = momento_database::library::Library::open(Path::new(&a[3])).unwrap();
            lib.touch(&s.recent_entry().unwrap()).unwrap();
            s.close();
        }
        Some("thumbs") => {
            let (s, _) = open(&a[2]);
            let rows = s.list_photos().unwrap();
            let n: usize = a[3].parse().unwrap();
            let keys = spread(&rows, n, |_| true);
            // Como o scroll: vários pedidos em paralelo (o WebView abre ~6–8 ligações).
            let par = 8;
            let lat = Arc::new(std::sync::Mutex::new(Vec::new()));
            let t = Instant::now();
            std::thread::scope(|sc| {
                for c in keys.chunks(keys.len().div_ceil(par)) {
                    let (s, lat) = (s.clone(), lat.clone());
                    sc.spawn(move || {
                        for k in c {
                            let t = Instant::now();
                            let p = s.serve_image(*k, CacheLevel::Thumb).unwrap();
                            let _ = std::fs::read(&p.path).unwrap();
                            lat.lock().unwrap().push(ms(t.elapsed()));
                        }
                    });
                }
            });
            let total = t.elapsed();
            println!(
                "RESULTADO miniaturas_servidas_por_s={:.0}",
                keys.len() as f64 / total.as_secs_f64()
            );
            report("miniatura_servir", &mut lat.lock().unwrap());
            s.close();
        }
        Some("thumbgen") => {
            // Regenera `n` miniaturas pela fila real (RAM e débito do serviço de miniaturas).
            let (s, ev) = open(&a[2]);
            let rows = s.list_photos().unwrap();
            let n: usize = a[3].parse().unwrap();
            let ids: Vec<AssetId> = rows.iter().take(n).map(|r| r.asset).collect();
            mem("antes");
            let before = ev.thumbs_done.load(Ordering::Relaxed);
            let t = Instant::now();
            s.enqueue_thumbnails(ids);
            std::thread::sleep(Duration::from_millis(50));
            while s.thumbnails_remaining() > 0 {
                std::thread::sleep(Duration::from_millis(20));
            }
            let el = t.elapsed().as_secs_f64();
            println!("RESULTADO thumbgen_n={n} thumbgen_por_s={:.0} thumbgen_ms_por_foto_parede={:.2} feitas={}", n as f64 / el, el * 1000.0 / n as f64, ev.thumbs_done.load(Ordering::Relaxed) - before);
            mem("thumbgen");
            if a.get(4).map(String::as_str) == Some("2x") {
                // Segunda passagem: se a memória fosse uma fuga, cresceria outra vez.
                let ids: Vec<AssetId> = rows.iter().take(n).map(|r| r.asset).collect();
                s.enqueue_thumbnails(ids);
                std::thread::sleep(Duration::from_millis(50));
                while s.thumbnails_remaining() > 0 {
                    std::thread::sleep(Duration::from_millis(20));
                }
                std::thread::sleep(Duration::from_millis(500));
                mem("thumbgen_2a_passagem");
            }
            s.close();
        }
        Some("preview") => {
            let (s, _) = open(&a[2]);
            let rows = s.list_photos().unwrap();
            let n: usize = a[3].parse().unwrap();
            let keys = spread(&rows, n, |r| !r.format.contains("raw"));
            for k in &keys {
                drop_cache(&s, *k, CacheLevel::Preview);
            }
            let (mut cold, mut warm) = (Vec::new(), Vec::new());
            for k in &keys {
                let t = Instant::now();
                s.serve_image(*k, CacheLevel::Preview).unwrap();
                cold.push(ms(t.elapsed()));
                let t = Instant::now();
                s.serve_image(*k, CacheLevel::Preview).unwrap();
                warm.push(ms(t.elapsed()));
            }
            report("preview_frio", &mut cold);
            report("preview_quente", &mut warm);
            mem("previews");
            s.close();
        }
        Some("priority") => {
            // "Estou a ver a fotografia 3421": a preview dela não pode esperar pelo trabalho em fundo.
            let (s, _) = open(&a[2]);
            let rows = s.list_photos().unwrap();
            let n: usize = a[3].parse().unwrap();
            let keys = spread(&rows, n * 2, |r| !r.format.contains("raw"));
            let (idle_keys, busy_keys) = keys.split_at(n);
            for k in &keys {
                drop_cache(&s, *k, CacheLevel::Preview);
            }
            let mut idle = Vec::new();
            for k in idle_keys {
                let t = Instant::now();
                s.serve_image(*k, CacheLevel::Preview).unwrap();
                idle.push(ms(t.elapsed()));
            }
            report("vista_sem_carga", &mut idle);
            // Carga em fundo: análise de IA de todo o projeto + miniaturas a regenerar.
            let pending = s.start_analysis(true).unwrap();
            let all: Vec<AssetId> = rows.iter().map(|r| r.asset).collect();
            s.enqueue_thumbnails(all);
            std::thread::sleep(Duration::from_secs(3));
            let mut busy = Vec::new();
            for k in busy_keys {
                let t = Instant::now();
                s.serve_image(*k, CacheLevel::Preview).unwrap();
                busy.push(ms(t.elapsed()));
                std::thread::sleep(Duration::from_millis(300));
            }
            report("vista_com_carga", &mut busy);
            println!("RESULTADO carga_analise_pendente={pending}");
            s.cancel_analysis();
            s.wait_analysis();
            s.close();
        }
        Some("nav") => {
            // Navegação rápida com as setas: um pedido de preview a cada 40 ms (como o
            // WebView, cada um na sua thread). Importa a latência da ÚLTIMA (a que fica no ecrã).
            let (s, _) = open(&a[2]);
            let rows = s.list_photos().unwrap();
            let n: usize = a[3].parse().unwrap();
            let keys = spread(&rows, n, |r| !r.format.contains("raw"));
            for k in &keys {
                drop_cache(&s, *k, CacheLevel::Preview);
            }
            let lat = Arc::new(std::sync::Mutex::new(vec![0.0; keys.len()]));
            let t = Instant::now();
            std::thread::scope(|sc| {
                for (i, k) in keys.iter().enumerate() {
                    let (s, lat) = (s.clone(), lat.clone());
                    sc.spawn(move || {
                        let t = Instant::now();
                        s.serve_image(*k, CacheLevel::Preview).unwrap();
                        lat.lock().unwrap()[i] = ms(t.elapsed());
                    });
                    std::thread::sleep(Duration::from_millis(40));
                }
            });
            let v = lat.lock().unwrap().clone();
            println!(
                "RESULTADO nav_ultima_ms={:.0} nav_todas_s={:.1} nav_n={}",
                v.last().unwrap(),
                t.elapsed().as_secs_f64(),
                v.len()
            );
            report("nav", &mut v.clone());
            s.close();
        }
        Some("raw") => {
            let (s, _) = open(&a[2]);
            let rows = s.list_photos().unwrap();
            let raws: Vec<&PhotoRow> = rows.iter().filter(|r| r.format.contains("raw")).collect();
            let mut seen = std::collections::HashSet::new();
            let (mut prev, mut full) = (Vec::new(), Vec::new());
            for r in raws {
                if !seen
                    .insert(r.format.clone() + &r.file_name[r.file_name.len().saturating_sub(4)..])
                    && seen.len() > 12
                {
                    continue;
                }
                let k = r.id.0;
                drop_cache(&s, k, CacheLevel::Preview);
                drop_cache(&s, k, CacheLevel::Full);
                let t = Instant::now();
                if s.serve_image(k, CacheLevel::Preview).is_err() {
                    continue;
                }
                let p = ms(t.elapsed());
                let t = Instant::now();
                let f = s.serve_image(k, CacheLevel::Full).map(|_| ms(t.elapsed()));
                println!(
                    "RAW {} preview_ms={p:.0} completo_ms={:.0}",
                    r.file_name,
                    f.as_ref().copied().unwrap_or(-1.0)
                );
                prev.push(p);
                if let Ok(f) = f {
                    full.push(f);
                }
                mem("raw");
            }
            report("raw_preview", &mut prev);
            report("raw_completo", &mut full);
            s.close();
        }
        Some("ai") => {
            let (s, _) = open(&a[2]);
            let secs: u64 = a[3].parse().unwrap();
            // Reanálise completa: o mesmo trabalho em todas as medições.
            s.start_analysis(true).unwrap();
            let before = 0;
            let t = Instant::now();
            let mut peak = 0;
            while t.elapsed() < Duration::from_secs(secs) && s.analysis_running() {
                std::thread::sleep(Duration::from_millis(500));
                peak = peak.max(status("VmRSS:"));
            }
            let done = s.analysis_status().unwrap().counts.analyzed - before;
            let el = t.elapsed().as_secs_f64();
            println!(
                "RESULTADO ia_fotografias={done} ia_por_s={:.2} ia_s_por_fotografia_parede={:.3} ia_threads={} ia_rss_pico_mb={peak}",
                done as f64 / el,
                el / done.max(1) as f64,
                (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) / 2).max(1)
            );
            s.cancel_analysis();
            s.wait_analysis();
            s.close();
        }
        Some("export") => {
            let (s, _) = open(&a[2]);
            let rows = s.list_photos().unwrap();
            let n: usize = a[3].parse().unwrap();
            let keys = spread(&rows, n, |r| !r.format.contains("raw"));
            let recipe = momento_export_engine::presets::builtin().into_iter().next().unwrap();
            println!("receita: {}", recipe.name);
            let t = Instant::now();
            let id = s
                .start_export(ExportRequest {
                    keys,
                    recipes: vec![recipe],
                    destination: a[4].clone(),
                    name: "bench".into(),
                    ..Default::default()
                })
                .unwrap();
            let mut peak = 0;
            loop {
                match s.job_state(id) {
                    Some(JobState::Done) => break,
                    Some(JobState::Failed) => panic!("falhou"),
                    _ => {}
                }
                peak = peak.max(status("VmRSS:"));
                std::thread::sleep(Duration::from_millis(100));
            }
            let el = t.elapsed().as_secs_f64();
            println!("RESULTADO exportacoes_por_min={:.0} exportacao_s={el:.1} n={n} export_rss_pico_mb={peak}", n as f64 * 60.0 / el);
            s.close();
        }
        _ => eprintln!("uso: ver o cabeçalho do ficheiro"),
    }
}

/// Tamanho do JSON enviado à interface (custo de IPC).
mod erased {
    pub trait Ser {
        fn len_json(&self) -> usize;
    }
    impl<T: serde::Serialize> Ser for T {
        fn len_json(&self) -> usize {
            serde_json::to_vec(self).map(|v| v.len()).unwrap_or(0)
        }
    }
}
