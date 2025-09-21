use std::sync::Arc;

use futures::executor::ThreadPool;
use futures::task::SpawnExt;

use super::{LangTask, RuntimeError, RuntimeResult, TaskHandle, TaskScheduler};

pub struct ParallelExecutor {
    pool: Arc<ThreadPool>,
}

impl ParallelExecutor {
    pub fn new() -> RuntimeResult<Self> {
        let pool = ThreadPool::new()
            .map_err(|err| RuntimeError::new(format!("failed to create thread pool: {err}")))?;
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

#[async_trait::async_trait]
impl TaskScheduler for ParallelExecutor {
    async fn schedule(&self, task: LangTask) -> RuntimeResult<TaskHandle> {
        let id = task.id();
        let future = task.into_future();
        let handle = self
            .pool
            .spawn_with_handle(async move { future.await })
            .map_err(|err| {
                RuntimeError::new(format!("failed to schedule task {}: {err}", id.get()))
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
        let task = LangTask::new(async { Ok::<_, RuntimeError>(21_i64) });
        let handle = block_on(executor.schedule(task)).unwrap();
        let result = block_on(handle.join_typed::<i64>()).unwrap();
        assert_eq!(result, 21);
    }
}
