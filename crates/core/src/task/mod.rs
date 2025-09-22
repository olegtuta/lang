use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;

use crate::{LangResult, Value};

pub type TaskFuture = Pin<Box<dyn Future<Output = LangResult<Value>> + Send>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    pub fn get(&self) -> u64 {
        self.0
    }
}

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

fn next_task_id() -> TaskId {
    TaskId(NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed))
}

pub struct LangTask {
    id: TaskId,
    future: TaskFuture,
}

impl LangTask {
    pub fn new<F>(future: F) -> Self
    where
        F: Future<Output = LangResult<Value>> + Send + 'static,
    {
        Self {
            id: next_task_id(),
            future: Box::pin(future),
        }
    }

    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn into_future(self) -> TaskFuture {
        self.future
    }
}

pub struct TaskHandle {
    id: TaskId,
    future: TaskFuture,
}

impl TaskHandle {
    pub fn new(id: TaskId, future: TaskFuture) -> Self {
        Self { id, future }
    }

    pub fn id(&self) -> TaskId {
        self.id
    }

    pub async fn join(self) -> LangResult<Value> {
        self.future.await
    }
}

#[async_trait]
pub trait TaskScheduler: Send + Sync {
    async fn schedule(&self, task: LangTask) -> LangResult<TaskHandle>;
}

pub struct ImmediateScheduler;

#[async_trait]
impl TaskScheduler for ImmediateScheduler {
    async fn schedule(&self, task: LangTask) -> LangResult<TaskHandle> {
        Ok(TaskHandle::new(task.id(), task.into_future()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn schedule_executes_task() {
        let scheduler = ImmediateScheduler;
        let task = LangTask::new(async { Ok(Value::from(7)) });
        let handle = block_on(scheduler.schedule(task)).unwrap();
        let result = block_on(handle.join()).unwrap();
        assert_eq!(result.expect_integer().unwrap(), 7);
    }

    #[test]
    fn task_id_is_unique() {
        let task_a = LangTask::new(async { Ok(Value::from(1)) });
        let task_b = LangTask::new(async { Ok(Value::from(2)) });
        assert_ne!(task_a.id().get(), task_b.id().get());
    }
}
