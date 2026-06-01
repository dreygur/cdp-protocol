use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::task::JoinSet;

use crate::client::CdpClient;
use crate::config::Config;
use crate::error::{CdpError, Result};

pub struct ClusterConfig {
    pub host: String,
    pub port: u16,
    pub concurrency: usize,
    pub retries: u32,
    pub monitor: bool,
    pub viewport_width: i32,
    pub viewport_height: i32,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        let base = Config::default();
        Self::from(base)
    }
}

impl From<Config> for ClusterConfig {
    fn from(c: Config) -> Self {
        ClusterConfig {
            host: c.host,
            port: c.port,
            concurrency: 5,
            retries: 2,
            monitor: false,
            viewport_width: c.viewport_width,
            viewport_height: c.viewport_height,
        }
    }
}

pub struct TaskResult<R> {
    pub result: std::result::Result<R, String>,
    pub elapsed: Duration,
    pub attempts: u32,
}

impl<R> TaskResult<R> {
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }
}

struct Pool {
    clients: Mutex<Vec<Arc<CdpClient>>>,
    semaphore: tokio::sync::Semaphore,
}

pub struct Cluster {
    pool: Arc<Pool>,
    config: Arc<ClusterConfig>,
}

impl Cluster {
    pub async fn new(config: ClusterConfig) -> Result<Self> {
        let mut clients = Vec::with_capacity(config.concurrency);

        for i in 0..config.concurrency {
            let target = CdpClient::create_tab(&config.host, config.port, None).await?;
            let ws_url = target
                .web_socket_debugger_url
                .ok_or_else(|| CdpError::InvalidUrl(format!("worker {i}: no WS URL")))?;
            let client = CdpClient::connect(&ws_url).await?;
            client.enable_domain("Page").await?;
            client.enable_domain("Runtime").await?;
            client.set_viewport(config.viewport_width, config.viewport_height, false).await?;
            clients.push(Arc::new(client));
        }

        Ok(Cluster {
            pool: Arc::new(Pool {
                semaphore: tokio::sync::Semaphore::new(config.concurrency),
                clients: Mutex::new(clients),
            }),
            config: Arc::new(config),
        })
    }

    pub async fn execute<D, R, F, Fut>(&self, data: D, task: F) -> TaskResult<R>
    where
        D: Clone + Send + 'static,
        R: Send + 'static,
        F: Fn(Arc<CdpClient>, D) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
    {
        let _permit = self.pool.semaphore.acquire().await.expect("semaphore closed");
        let client = self.pool.clients.lock().await.pop().expect("worker missing");

        let (result, elapsed, attempts) = run_with_retries(
            Arc::clone(&client),
            data,
            &task,
            self.config.retries,
        )
        .await;

        self.pool.clients.lock().await.push(client);

        if self.config.monitor {
            match &result {
                Ok(_) => println!("[cluster] ok  {:.1}s ({attempts}x)", elapsed.as_secs_f64()),
                Err(e) => println!("[cluster] err {:.1}s ({attempts}x): {e}", elapsed.as_secs_f64()),
            }
        }

        TaskResult { result, elapsed, attempts }
    }

    pub async fn run<D, R, F, Fut>(
        &self,
        items: impl IntoIterator<Item = D>,
        task: F,
    ) -> Vec<TaskResult<R>>
    where
        D: Clone + Send + Sync + 'static,
        R: Send + 'static,
        F: Fn(Arc<CdpClient>, D) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
    {
        let mut set = JoinSet::new();

        for data in items {
            let pool = self.pool.clone();
            let config = self.config.clone();
            let task = task.clone();

            set.spawn(async move {
                let _permit = pool.semaphore.acquire().await.expect("semaphore closed");
                let client = pool.clients.lock().await.pop().expect("worker missing");

                let (result, elapsed, attempts) =
                    run_with_retries(Arc::clone(&client), data, &task, config.retries).await;

                pool.clients.lock().await.push(client);

                if config.monitor {
                    match &result {
                        Ok(_) => println!("[cluster] ok  {:.1}s ({attempts}x)", elapsed.as_secs_f64()),
                        Err(e) => println!("[cluster] err {:.1}s ({attempts}x): {e}", elapsed.as_secs_f64()),
                    }
                }

                TaskResult { result, elapsed, attempts }
            });
        }

        let mut results = Vec::new();
        while let Some(res) = set.join_next().await {
            if let Ok(r) = res {
                results.push(r);
            }
        }
        results
    }

    pub async fn close(self) {
        let clients = self.pool.clients.lock().await;
        for client in clients.iter() {
            let _ = client.close().await;
        }
    }
}

async fn run_with_retries<D, R, F, Fut>(
    client: Arc<CdpClient>,
    data: D,
    task: &F,
    retries: u32,
) -> (std::result::Result<R, String>, Duration, u32)
where
    D: Clone,
    F: Fn(Arc<CdpClient>, D) -> Fut,
    Fut: Future<Output = Result<R>>,
{
    let start = Instant::now();
    let mut attempts = 0u32;

    loop {
        attempts += 1;
        match task(Arc::clone(&client), data.clone()).await {
            Ok(r) => return (Ok(r), start.elapsed(), attempts),
            Err(e) => {
                if attempts > retries {
                    return (Err(e.to_string()), start.elapsed(), attempts);
                }
            }
        }
    }
}
