use std::sync::Arc;

use async_trait::async_trait;
use futures::executor::ThreadPool;
use futures::task::SpawnExt;

use lang_core::task::{LangTask, TaskHandle, TaskScheduler};
use lang_core::{LangError, LangResult};

pub struct ParallelExecutor {
    pool: Arc<ThreadPool>,
}

impl ParallelExecutor {
    pub fn new() -> LangResult<Self> {
        let pool = ThreadPool::new()
            .map_err(|err| LangError::Runtime(format!("failed to create thread pool: {err}")))?;
        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    pub fn with_pool(pool: ThreadPool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }
}

#[async_trait]
impl TaskScheduler for ParallelExecutor {
    async fn schedule(&self, task: LangTask) -> LangResult<TaskHandle> {
        let id = task.id();
        let future = task.into_future();
        let handle = self
            .pool
            .spawn_with_handle(async move { future.await })
            .map_err(|err| {
                LangError::Runtime(format!("failed to schedule task {}: {err}", id.get()))
            })?;

        Ok(TaskHandle::new(id, Box::pin(handle)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn parallel_executor_runs_tasks() {
        let executor = ParallelExecutor::new().unwrap();
        let task = LangTask::new(async { Ok(lang_core::Value::from(21)) });
        let handle = block_on(executor.schedule(task)).unwrap();
        let result = block_on(handle.join()).unwrap();
        assert_eq!(result.expect_integer().unwrap(), 21);
    }
}
