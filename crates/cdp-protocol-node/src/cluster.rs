//! A fixed-size pool of worker tabs, bound for JS.

use std::sync::Arc;

use cdp_driver::{BrowserAgent as CoreAgent, CdpClient as CoreClient};
use napi::bindgen_prelude::{Error, Result};
use napi_derive::napi;
use serde_json::Value;
use tokio::sync::{Mutex, Semaphore};

use crate::action_result::{core_result, ActionResult};
use crate::errors::{json_err, to_napi};

/// Result of one clustered task.
#[napi(object)]
pub struct TaskResult {
    pub success: bool,
    pub results: Vec<ActionResult>,
    pub elapsed_ms: f64,
    pub attempts: u32,
    pub error: Option<String>,
}

/// Options for [`Cluster.create`].
#[napi(object)]
pub struct ClusterOptions {
    pub host: String,
    pub port: u16,
    /// Number of tabs / concurrent workers.
    pub concurrency: u32,
    /// Retries per task on failure (default 0).
    pub retries: Option<u32>,
    pub viewport_width: Option<i32>,
    pub viewport_height: Option<i32>,
}

struct Worker {
    agent: CoreAgent,
}

/// Fixed-size pool of worker tabs, one [`BrowserAgent`] each.
///
/// This is a purpose-built, action-batch oriented pool for JS, NOT a binding of
/// the Rust `cdp_driver::cluster::Cluster` (whose generic closure-based `run`
/// cannot cross the FFI boundary). Semantics: [`Cluster.execute`] checks out one
/// worker, runs the action batch in order, aborts the batch on the first failed
/// action, and retries the whole batch up to `retries` times.
#[napi]
pub struct Cluster {
    workers: Arc<Mutex<Vec<Arc<Worker>>>>,
    sem: Arc<Semaphore>,
    retries: u32,
}

#[napi]
impl Cluster {
    /// Open `concurrency` tabs and wrap each as a worker agent.
    ///
    /// If any worker fails to come up, every tab already opened is closed before
    /// returning the error, so a partial init never leaks tabs.
    #[napi(factory)]
    pub async fn create(opts: ClusterOptions) -> Result<Cluster> {
        let width = opts.viewport_width.unwrap_or(1920);
        let height = opts.viewport_height.unwrap_or(1200);
        let mut workers: Vec<Arc<Worker>> = Vec::with_capacity(opts.concurrency as usize);

        for i in 0..opts.concurrency {
            match Self::spawn_worker(&opts.host, opts.port, width, height, i).await {
                Ok(w) => workers.push(Arc::new(w)),
                Err(e) => {
                    // Roll back: close every tab opened so far.
                    for w in &workers {
                        let _ = w.agent.close().await;
                    }
                    return Err(e);
                }
            }
        }

        Ok(Cluster {
            workers: Arc::new(Mutex::new(workers)),
            sem: Arc::new(Semaphore::new(opts.concurrency as usize)),
            retries: opts.retries.unwrap_or(0),
        })
    }

    async fn spawn_worker(
        host: &str,
        port: u16,
        width: i32,
        height: i32,
        i: u32,
    ) -> Result<Worker> {
        let target = CoreClient::create_tab(host, port, None)
            .await
            .map_err(to_napi)?;
        let ws = target.web_socket_debugger_url.ok_or_else(|| {
            Error::from_reason(format!(
                "[NO_TARGET] worker {i}: target has no debugger URL"
            ))
        })?;
        let client = CoreClient::connect(&ws).await.map_err(to_napi)?;
        for d in ["Page", "Runtime", "DOM", "Network"] {
            client.enable_domain(d).await.map_err(to_napi)?;
        }
        client
            .set_viewport(width, height, false)
            .await
            .map_err(to_napi)?;
        Ok(Worker {
            agent: CoreAgent::from_client(client),
        })
    }

    /// Run one action batch on a free worker, with retries.
    #[napi]
    pub async fn execute(&self, actions: Vec<Value>) -> Result<TaskResult> {
        let jsons: std::result::Result<Vec<String>, _> =
            actions.iter().map(serde_json::to_string).collect();
        let jsons = jsons.map_err(json_err)?;

        let _permit = self.sem.acquire().await.expect("semaphore closed");
        let worker = self.workers.lock().await.pop().expect("worker missing");

        let start = std::time::Instant::now();
        let mut attempts = 0u32;
        let mut results;
        loop {
            attempts += 1;
            results = Vec::with_capacity(jsons.len());
            let mut ok = true;
            for j in &jsons {
                let r = core_result(worker.agent.execute_json(j).await);
                if !r.success {
                    ok = false;
                }
                let stop = !r.success;
                results.push(r);
                if stop {
                    break;
                }
            }
            if ok || attempts > self.retries {
                let success = ok;
                self.workers.lock().await.push(worker);
                return Ok(TaskResult {
                    success,
                    error: if success {
                        None
                    } else {
                        results.last().and_then(|r| r.error.clone())
                    },
                    results,
                    elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                    attempts,
                });
            }
        }
    }

    /// Close every worker tab.
    #[napi]
    pub async fn close(&self) -> Result<()> {
        let workers = self.workers.lock().await;
        for w in workers.iter() {
            let _ = w.agent.close().await;
        }
        Ok(())
    }
}
