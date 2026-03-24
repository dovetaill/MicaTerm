//! Shared Tokio runtime bootstrap for application-level background services.

use std::future::Future;
use std::sync::Arc;

/// Centralized app runtime so future SSH and background services share one executor.
#[derive(Clone)]
pub struct AppAsyncRuntime {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl AppAsyncRuntime {
    pub fn new() -> anyhow::Result<Self> {
        let worker_threads = std::thread::available_parallelism()
            .map(|value| value.get().min(2))
            .unwrap_or(2);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("mica-term-bg")
            .worker_threads(worker_threads)
            .build()?;

        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        self.runtime.block_on(future)
    }
}
