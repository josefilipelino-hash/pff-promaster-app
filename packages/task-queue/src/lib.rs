//! # momento-task-queue
//!
//! Na Fase 0 este crate define apenas o **modelo** da fila (prioridades e máquina de
//! estados), que será persistido na tabela `job` do catálogo. O executor (tokio para
//! I/O + rayon para CPU) é implementado na Fase 1. Ver ARCHITECTURE.md §9.

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
