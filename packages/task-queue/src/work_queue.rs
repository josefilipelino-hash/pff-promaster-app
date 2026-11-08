use std::collections::{HashSet, VecDeque};
use std::hash::Hash;
use std::sync::{Condvar, Mutex, MutexGuard};

struct State<T> {
    urgent: VecDeque<T>,
    normal: VecDeque<T>,
    /// Itens à espera (verdade única; as filas podem conter entradas obsoletas).
    queued: HashSet<T>,
    closed: bool,
}

/// Fila de trabalho com duas prioridades.
///
/// - `push` acrescenta itens ao fim da fila normal (ignora os que já estão à espera);
/// - `prioritize` põe à frente os itens indicados que ainda estão à espera, por ordem;
///   a chamada seguinte substitui a lista urgente anterior (os itens voltam à ordem normal);
/// - `pop` bloqueia até haver trabalho ou a fila ser fechada.
pub struct WorkQueue<T: Copy + Eq + Hash> {
    state: Mutex<State<T>>,
    cv: Condvar,
}

impl<T: Copy + Eq + Hash> Default for WorkQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + Eq + Hash> WorkQueue<T> {
    pub fn new() -> Self {
        WorkQueue {
            state: Mutex::new(State {
                urgent: VecDeque::new(),
                normal: VecDeque::new(),
                queued: HashSet::new(),
                closed: false,
            }),
            cv: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Devolve quantos itens novos entraram na fila.
    pub fn push(&self, items: impl IntoIterator<Item = T>) -> usize {
        let mut s = self.lock();
        if s.closed {
            return 0;
        }
        let mut n = 0;
        for it in items {
            if s.queued.insert(it) {
                s.normal.push_back(it);
                n += 1;
            }
        }
        drop(s);
        self.cv.notify_all();
        n
    }

    pub fn prioritize(&self, items: impl IntoIterator<Item = T>) {
        let mut s = self.lock();
        // A lista urgente anterior volta para a frente da fila normal.
        let old: Vec<T> = s.urgent.drain(..).collect();
        for it in old.into_iter().rev() {
            s.normal.push_front(it);
        }
        let new: Vec<T> = items.into_iter().filter(|i| s.queued.contains(i)).collect();
        s.urgent.extend(new);
    }

    pub fn pop(&self) -> Option<T> {
        let mut s = self.lock();
        loop {
            while let Some(it) = s.urgent.pop_front().or_else(|| s.normal.pop_front()) {
                if s.queued.remove(&it) {
                    return Some(it);
                }
            }
            if s.closed {
                return None;
            }
            s = self.cv.wait(s).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn len(&self) -> usize {
        self.lock().queued.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fecha a fila: descarta o trabalho por fazer e liberta as threads em `pop`.
    pub fn close(&self) {
        let mut s = self.lock();
        s.closed = true;
        s.urgent.clear();
        s.normal.clear();
        s.queued.clear();
        drop(s);
        self.cv.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn fifo_without_duplicates() {
        let q = WorkQueue::new();
        assert_eq!(q.push([1, 2, 3, 2]), 3);
        assert_eq!(q.len(), 3);
        assert_eq!((q.pop(), q.pop(), q.pop()), (Some(1), Some(2), Some(3)));
        assert!(q.is_empty());
    }

    #[test]
    fn prioritized_items_jump_ahead_and_are_replaced() {
        let q = WorkQueue::new();
        q.push(1..=10);
        q.prioritize([8, 9, 42]); // 42 não está na fila: ignorado
        assert_eq!(q.pop(), Some(8));
        q.prioritize([5]); // 9 volta à frente da fila normal
        assert_eq!(q.pop(), Some(5));
        assert_eq!(q.pop(), Some(9));
        let rest: Vec<_> =
            std::iter::from_fn(|| (!q.is_empty()).then(|| q.pop().unwrap())).collect();
        assert_eq!(rest, [1, 2, 3, 4, 6, 7, 10]);
    }

    #[test]
    fn close_releases_blocked_workers() {
        let q = Arc::new(WorkQueue::<u32>::new());
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let q = q.clone();
                std::thread::spawn(move || q.pop())
            })
            .collect();
        std::thread::sleep(std::time::Duration::from_millis(50));
        q.close();
        for h in handles {
            assert_eq!(h.join().unwrap(), None);
        }
        assert_eq!(q.push([1]), 0, "fila fechada não aceita trabalho");
    }

    #[test]
    fn many_workers_process_each_item_once() {
        let q = Arc::new(WorkQueue::<u32>::new());
        q.push(0..10_000);
        let done = Arc::new(Mutex::new(Vec::new()));
        let hs: Vec<_> = (0..8)
            .map(|_| {
                let (q, done) = (q.clone(), done.clone());
                std::thread::spawn(move || {
                    while !q.is_empty() {
                        if let Some(v) = q.pop() {
                            done.lock().unwrap().push(v);
                        }
                        if q.is_empty() {
                            break;
                        }
                    }
                })
            })
            .collect();
        // Garantir que nenhuma thread fica bloqueada no último pop.
        while !q.is_empty() {
            std::thread::yield_now();
        }
        q.close();
        for h in hs {
            h.join().unwrap();
        }
        let mut v = done.lock().unwrap().clone();
        v.sort();
        assert_eq!(v, (0..10_000).collect::<Vec<_>>());
    }
}
