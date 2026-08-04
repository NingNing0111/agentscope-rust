//! WorkspaceManager — multi-tenant workspace lifecycle management.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::base::WorkspaceBase;
use crate::local_workspace::{LocalWorkspace, LocalWorkspaceConfig};

/// Entry in the manager's workspace map.
struct ManagerEntry {
    workspace: Arc<dyn WorkspaceBase>,
    last_access: Instant,
}

type FactoryFn = Arc<dyn Fn(String) -> LocalWorkspaceConfig + Send + Sync>;

/// Manages multiple workspace instances by key, with optional TTL eviction.
pub struct WorkspaceManager {
    entries: Arc<RwLock<HashMap<String, ManagerEntry>>>,
    #[allow(dead_code)]
    ttl: Option<Duration>,
    cleanup_handle: Option<JoinHandle<()>>,
    factory: FactoryFn,
}

impl WorkspaceManager {
    /// Create a new WorkspaceManager with optional TTL.
    /// If TTL is None, workspaces are never evicted.
    #[must_use]
    pub fn new(
        ttl: Option<Duration>,
        factory: impl Fn(String) -> LocalWorkspaceConfig + Send + Sync + 'static,
    ) -> Self {
        let entries: Arc<RwLock<HashMap<String, ManagerEntry>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let cleanup_handle = ttl.map(|ttl| {
            let entries = Arc::clone(&entries);
            let interval = ttl / 2;
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(interval).await;
                    let now = Instant::now();
                    let mut map = entries.write().await;
                    map.retain(|_key, entry| now.duration_since(entry.last_access) < ttl);
                }
            })
        });

        Self {
            entries,
            ttl,
            cleanup_handle,
            factory: Arc::new(factory),
        }
    }

    /// Get or create a workspace for the given key.
    pub async fn get(
        &self,
        key: &str,
    ) -> Result<Arc<dyn WorkspaceBase>, crate::error::WorkspaceError> {
        {
            let map = self.entries.read().await;
            if let Some(_entry) = map.get(key) {
                drop(map);
                let mut map = self.entries.write().await;
                if let Some(entry) = map.get_mut(key) {
                    entry.last_access = Instant::now();
                    return Ok(Arc::clone(&entry.workspace));
                }
            }
        }

        // Fast path missed (or raced): build the workspace outside the lock, but
        // re-check under the write lock before inserting so two concurrent `get`
        // calls for the same key don't both `initialize` and one clobber the
        // other (audit S8). The losing instance is simply dropped — for a
        // LocalWorkspace that only means its initialize side-effects (seed
        // files) ran once more, which is idempotent.
        let config = (self.factory)(key.to_string());
        let mut ws = LocalWorkspace::new(config);
        ws.initialize().await?;
        let ws: Arc<dyn WorkspaceBase> = Arc::new(ws);

        let mut map = self.entries.write().await;
        if let Some(entry) = map.get_mut(key) {
            entry.last_access = Instant::now();
            return Ok(Arc::clone(&entry.workspace));
        }
        map.insert(
            key.to_string(),
            ManagerEntry {
                workspace: Arc::clone(&ws),
                last_access: Instant::now(),
            },
        );

        Ok(ws)
    }
}

impl Drop for WorkspaceManager {
    fn drop(&mut self) {
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
    }
}
