pub mod task;

use crate::queue::task::{DownloadTask, Priority, TaskState};

/// In-memory очередь с приоритетами.
/// Оркестратор владеет Queue и управляет ей через &mut.
pub struct Queue {
    tasks: Vec<DownloadTask>,
}

impl Queue {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn push(&mut self, task: DownloadTask) {
        self.tasks.push(task);
    }

    pub fn all(&self) -> &[DownloadTask] {
        &self.tasks
    }

    pub fn get(&self, id: &str) -> Option<&DownloadTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut DownloadTask> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    /// Следующий задача для запуска (Queued/Waiting с наивысшим приоритетом)
    pub fn next_waiting(&self) -> Option<&DownloadTask> {
        self.tasks
            .iter()
            .filter(|t| t.state == TaskState::Waiting)
            .max_by(|a, b| {
                a.priority
                    .cmp(&b.priority)
                    .then(a.created_at.cmp(&b.created_at))
            })
    }

    pub fn remove(&mut self, id: &str) -> Option<DownloadTask> {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == id) {
            Some(self.tasks.remove(pos))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    pub fn update_state(&mut self, id: &str, state: TaskState) {
        if let Some(task) = self.get_mut(id) {
            task.state = state;
        }
    }

    pub fn set_priority(&mut self, id: &str, priority: Priority) -> bool {
        if let Some(task) = self.get_mut(id) {
            task.priority = priority;
            self.sort();
            true
        } else {
            false
        }
    }

    pub fn reorder(&mut self, id: &str, new_index: usize) -> bool {
        let pos = self.tasks.iter().position(|t| t.id == id);
        let Some(pos) = pos else { return false };
        let new_index = new_index.min(self.tasks.len().saturating_sub(1));
        if pos == new_index {
            return true;
        }
        let task = self.tasks.remove(pos);
        self.tasks.insert(new_index, task);
        true
    }

    /// Удалить из очереди только успешно завершённые задачи (ушедшие в историю).
    /// Failed/Cancelled НЕ удаляются — пользователь должен видеть ошибку и иметь
    /// возможность повторить задачу (retry_task). Возвращает ID удалённых задач.
    pub fn trim_completed(&mut self) -> Vec<String> {
        let mut removed = Vec::new();
        self.tasks.retain(|t| {
            if t.state == TaskState::Completed {
                removed.push(t.id.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    /// Перенумеровать `ordering` всех задач в соответствии с их текущим порядком
    /// в векторе. Используется после ручного reorder_task, чтобы зафиксировать
    /// явный порядок (см. Orchestrator::reorder_task).
    pub fn renumber_ordering(&mut self) {
        for (i, t) in self.tasks.iter_mut().enumerate() {
            t.ordering = Some(i as u32);
        }
    }

    pub fn active_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state == TaskState::Downloading || t.state == TaskState::Converting)
            .count()
    }

    fn sort(&mut self) {
        self.tasks.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
    }
}
