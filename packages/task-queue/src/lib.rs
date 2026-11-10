//! # momento-task-queue
//!
//! - [`WorkQueue`]: fila em memória partilhada por várias threads de trabalho, com
//!   prioridade para itens urgentes (ex.: miniaturas visíveis no ecrã), sem duplicados
//!   e com encerramento limpo.
//! - [`foreground`]: o trabalho interativo (fotografia vista) passa à frente do segundo plano.
//! - Modelo de estados de tarefas ([`JobState`], [`Priority`]) para a futura tabela `job`.
//!
//! Persistência na Fase 1: cada trabalho guarda o seu estado no próprio domínio
//! (`import_item.state`, `asset.thumb_state`), o que permite retomar após falha sem uma
//! tabela genérica. Ver ARCHITECTURE.md §9.

pub mod foreground;
mod work_queue;
pub use work_queue::WorkQueue;

use serde::{Deserialize, Serialize};

/// Prioridade de uma tarefa. A ordem da enumeração é a ordem de execução.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    /// Pedido direto do utilizador (ex.: preview 1:1 da fotografia aberta).
    Interactive = 0,
    /// O que está visível no ecrã (thumbnails da grelha).
    Visible = 1,
    /// Trabalho normal em segundo plano (importação, análise).
    Normal = 2,
    /// Manutenção (pré-geração de previews, limpeza de cache).
    Background = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn is_terminal(self) -> bool {
        matches!(self, JobState::Completed | JobState::Failed | JobState::Cancelled)
    }

    /// Transições permitidas. `Running → Queued` acontece na recuperação após falha:
    /// tarefas que estavam a correr quando a aplicação terminou voltam à fila.
    pub fn can_transition_to(self, next: JobState) -> bool {
        use JobState::*;
        matches!(
            (self, next),
            (Queued, Running)
                | (Queued, Paused)
                | (Queued, Cancelled)
                | (Running, Completed)
                | (Running, Failed)
                | (Running, Paused)
                | (Running, Cancelled)
                | (Running, Queued)
                | (Paused, Queued)
                | (Paused, Cancelled)
                | (Failed, Queued)
        )
    }
}

/// Estado a atribuir no arranque a uma tarefa encontrada no catálogo.
pub fn recover_state(stored: JobState) -> JobState {
    match stored {
        JobState::Running => JobState::Queued,
        s => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use JobState::*;

    #[test]
    fn priorities_order_interactive_first() {
        let mut v =
            vec![Priority::Background, Priority::Interactive, Priority::Normal, Priority::Visible];
        v.sort();
        assert_eq!(
            v,
            [Priority::Interactive, Priority::Visible, Priority::Normal, Priority::Background]
        );
    }

    #[test]
    fn terminal_states_cannot_restart_except_failed_retry() {
        assert!(!Completed.can_transition_to(Queued));
        assert!(!Cancelled.can_transition_to(Queued));
        assert!(Failed.can_transition_to(Queued));
        for s in [Completed, Cancelled] {
            for n in [Queued, Running, Paused, Completed, Failed, Cancelled] {
                assert!(!s.can_transition_to(n));
            }
        }
    }

    #[test]
    fn running_jobs_are_requeued_after_crash() {
        assert_eq!(recover_state(Running), Queued);
        assert_eq!(recover_state(Paused), Paused);
        assert!(Running.can_transition_to(recover_state(Running)));
    }

    #[test]
    fn serializes_for_ui() {
        assert_eq!(serde_json::to_string(&Priority::Visible).unwrap(), "\"visible\"");
        assert_eq!(serde_json::to_string(&Paused).unwrap(), "\"paused\"");
    }
}
