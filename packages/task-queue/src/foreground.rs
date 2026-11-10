//! Prioridade do primeiro plano: "se estou a ver a fotografia 3421, o processamento dessa
//! fotografia passa à frente do trabalho em segundo plano".
//!
//! O trabalho interativo (preview/1:1 da fotografia aberta, render do editor) segura um
//! [`Interactive`] enquanto corre. As threads de segundo plano (miniaturas, análise,
//! edição automática, exportação) chamam [`yield_to_foreground`] entre itens: enquanto
//! houver trabalho interativo, esperam, deixando todos os núcleos à fotografia vista.
//! Um limite de espera evita que o segundo plano pare de vez com o utilizador sempre a
//! navegar.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static LOCK: Mutex<()> = Mutex::new(());
static CHANGED: Condvar = Condvar::new();

/// Espera máxima de uma thread de segundo plano por cada item.
pub const MAX_WAIT: Duration = Duration::from_secs(3);

/// Marca trabalho interativo em curso (até ser largado).
#[must_use = "o trabalho interativo dura enquanto o guarda existir"]
pub struct Interactive(());

impl Interactive {
    pub fn begin() -> Self {
        ACTIVE.fetch_add(1, Ordering::SeqCst);
        Interactive(())
    }
}

impl Drop for Interactive {
    fn drop(&mut self) {
        if ACTIVE.fetch_sub(1, Ordering::SeqCst) == 1 {
            let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            CHANGED.notify_all();
        }
    }
}

/// Há trabalho interativo em curso?
pub fn busy() -> bool {
    ACTIVE.load(Ordering::SeqCst) > 0
}

/// Chamado pelas threads de segundo plano antes de cada item. Devolve o tempo esperado.
pub fn yield_to_foreground() -> Duration {
    if !busy() {
        return Duration::ZERO;
    }
    let t = Instant::now();
    let mut g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    while busy() {
        let left = MAX_WAIT.saturating_sub(t.elapsed());
        if left.is_zero() {
            break;
        }
        g = CHANGED.wait_timeout(g, left).unwrap_or_else(|e| e.into_inner()).0;
    }
    t.elapsed()
}

/// Portão LIFO para trabalho interativo pesado (gerar a preview da fotografia vista).
///
/// Ao navegar depressa com as setas, cada fotografia pede a sua preview; a que fica no
/// ecrã é a ÚLTIMA. Com `permits` em execução no máximo, quem espera entra pela ordem
/// inversa de chegada: o pedido mais recente passa à frente dos que ficaram para trás
/// (que continuam a ser feitos depois e ficam em cache).
pub struct LifoGate {
    permits: usize,
    state: Mutex<GateState>,
    cv: Condvar,
}

#[derive(Default)]
struct GateState {
    running: usize,
    next_ticket: u64,
    waiting: Vec<u64>,
}

pub struct GatePermit<'a>(&'a LifoGate);

impl LifoGate {
    pub fn new(permits: usize) -> Self {
        LifoGate {
            permits: permits.max(1),
            state: Mutex::new(GateState::default()),
            cv: Condvar::new(),
        }
    }

    pub fn acquire(&self) -> GatePermit<'_> {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let ticket = st.next_ticket;
        st.next_ticket += 1;
        st.waiting.push(ticket);
        while !(st.running < self.permits && st.waiting.last() == Some(&ticket)) {
            st = self.cv.wait(st).unwrap_or_else(|e| e.into_inner());
        }
        st.waiting.pop();
        st.running += 1;
        GatePermit(self)
    }
}

impl Drop for GatePermit<'_> {
    fn drop(&mut self) {
        let mut st = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        st.running -= 1;
        self.0.cv.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    #[test]
    fn lifo_gate_serves_the_newest_request_first() {
        let gate = Arc::new(LifoGate::new(1));
        let order = Arc::new(Mutex::new(Vec::new()));
        let first = gate.acquire();
        let mut hs = Vec::new();
        for i in 0..4 {
            let (g, o) = (gate.clone(), order.clone());
            hs.push(std::thread::spawn(move || {
                let _p = g.acquire();
                o.lock().unwrap().push(i);
            }));
            std::thread::sleep(Duration::from_millis(40));
        }
        drop(first);
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(*order.lock().unwrap(), vec![3, 2, 1, 0], "o mais recente primeiro");
    }

    #[test]
    fn background_waits_while_foreground_runs() {
        // Testes no mesmo processo partilham o estado global: um só teste usa-o.
        assert_eq!(yield_to_foreground(), Duration::ZERO, "sem trabalho interativo não espera");
        let g = Interactive::begin();
        assert!(busy());
        let done = Arc::new(AtomicBool::new(false));
        let d = done.clone();
        let h = std::thread::spawn(move || {
            let w = yield_to_foreground();
            d.store(true, Ordering::SeqCst);
            w
        });
        std::thread::sleep(Duration::from_millis(150));
        assert!(!done.load(Ordering::SeqCst), "o segundo plano espera");
        drop(g);
        let waited = h.join().unwrap();
        assert!(waited >= Duration::from_millis(140) && waited < MAX_WAIT, "{waited:?}");
        assert!(!busy());
        // Nunca fica parado para sempre.
        let _g = Interactive::begin();
        let t = Instant::now();
        yield_to_foreground();
        assert!(t.elapsed() >= MAX_WAIT && t.elapsed() < MAX_WAIT + Duration::from_secs(1));
    }
}
