use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use thiserror::Error;

pub type TaskOutput = Box<dyn Any + Send>;
pub type TaskFuture = Pin<Box<dyn Future<Output = RuntimeResult<TaskOutput>> + Send>>;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("{0}")]
    Message(String),
}

impl RuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        RuntimeError::Message(message.into())
    }
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;

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
    pub fn new<F, T>(future: F) -> Self
    where
        F: Future<Output = RuntimeResult<T>> + Send + 'static,
        T: Any + Send,
    {
        let id = next_task_id();
        let wrapped = async move {
            match future.await {
                Ok(value) => Ok(Box::new(value) as TaskOutput),
                Err(err) => Err(err),
            }
        };
        Self {
            id,
            future: Box::pin(wrapped),
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

    pub async fn join(self) -> RuntimeResult<TaskOutput> {
        self.future.await
    }

    pub async fn join_typed<T: Any + Send>(self) -> RuntimeResult<T> {
        let value = self.join().await?;
        value
            .downcast::<T>()
            .map(|boxed| *boxed)
            .map_err(|_| RuntimeError::new("failed to downcast task output"))
    }
}

#[async_trait]
pub trait TaskScheduler: Send + Sync {
    async fn schedule(&self, task: LangTask) -> RuntimeResult<TaskHandle>;
}

pub mod executor;

pub use executor::ParallelExecutor;

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn schedule_executes_task() {
        let scheduler = ParallelExecutor::new().unwrap();
        let task = LangTask::new(async { Ok::<_, RuntimeError>(7_i64) });
        let handle = block_on(scheduler.schedule(task)).unwrap();
        let result = block_on(handle.join_typed::<i64>()).unwrap();
        assert_eq!(result, 7);
    }

    #[test]
    fn task_id_is_unique() {
        let task_a = LangTask::new(async { Ok::<_, RuntimeError>(1_i64) });
        let task_b = LangTask::new(async { Ok::<_, RuntimeError>(2_i64) });
        assert_ne!(task_a.id().get(), task_b.id().get());
    }
}
