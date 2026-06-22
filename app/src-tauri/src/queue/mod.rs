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
        self.sort();
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
            .max_by(|a, b| a.priority.cmp(&b.priority).then(b.created_at.cmp(&a.created_at)))
    }

    pub fn remove(&mut self, id: &str) -> Option<DownloadTask> {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == id) {
            Some(self.tasks.remove(pos))
        } else {
            None
        }
    }

    pub fn update_state(&mut self, id: &str, state: TaskState) {
        if let Some(task) = self.get_mut(id) {
            task.state = state;
        }
    }

    pub fn set_priority(&mut self, id: &str, priority: Priority) {
        if let Some(task) = self.get_mut(id) {
            task.priority = priority;
            self.sort();
        }
    }

    pub fn active_count(&self) -> usize {
        self.tasks.iter()
            .filter(|t| t.state == TaskState::Downloading || t.state == TaskState::Converting)
            .count()
    }

    fn sort(&mut self) {
        self.tasks.sort_by(|a, b| {
            b.priority.cmp(&a.priority).then(a.created_at.cmp(&b.created_at))
        });
    }
}
