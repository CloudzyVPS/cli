use serde::Serialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Maximum number of log entries kept in memory.
const MAX_LOG_ENTRIES: usize = 200;

/// A single MCP call log entry.
#[derive(Clone, Debug, Serialize)]
pub struct McpLogEntry {
    /// Monotonically increasing identifier.
    pub id: u64,
    /// ISO-8601 timestamp of when the request was received.
    pub timestamp: String,
    /// JSON-RPC method name (e.g. "tools/call", "initialize").
    pub method: String,
    /// Raw JSON request body.
    pub request: Value,
    /// Raw JSON response body.
    pub response: Value,
    /// Duration of the call in milliseconds.
    pub duration_ms: u64,
    /// Whether the response contained an error.
    pub is_error: bool,
}

/// Thread-safe, bounded in-memory store for MCP call logs.
#[derive(Clone, Debug)]
pub struct McpLogStore {
    inner: Arc<Mutex<LogStoreInner>>,
}

#[derive(Debug)]
struct LogStoreInner {
    entries: Vec<McpLogEntry>,
    next_id: u64,
}

/// Paginated response returned by the logs API.
#[derive(Serialize)]
pub struct PaginatedLogs {
    pub logs: Vec<McpLogEntry>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
}

impl McpLogStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LogStoreInner {
                entries: Vec::new(),
                next_id: 1,
            })),
        }
    }

    /// Record a new log entry. Old entries are evicted when the buffer is full.
    pub fn push(&self, method: String, request: Value, response: Value, duration_ms: u64, is_error: bool) {
        let mut inner = self.inner.lock().unwrap();
        let id = inner.next_id;
        inner.next_id += 1;
        let timestamp = chrono::Utc::now().to_rfc3339();
        inner.entries.push(McpLogEntry {
            id,
            timestamp,
            method,
            request,
            response,
            duration_ms,
            is_error,
        });
        // Evict oldest entries when over the cap.
        if inner.entries.len() > MAX_LOG_ENTRIES {
            let excess = inner.entries.len() - MAX_LOG_ENTRIES;
            inner.entries.drain(..excess);
        }
    }

    /// Return a page of log entries in reverse-chronological order (newest first).
    pub fn list(&self, page: usize, per_page: usize) -> PaginatedLogs {
        let inner = self.inner.lock().unwrap();
        let total = inner.entries.len();
        let per_page = if per_page == 0 { 20 } else { per_page };

        if total == 0 {
            return PaginatedLogs { logs: vec![], total: 0, page: 1, per_page, total_pages: 1 };
        }

        let total_pages = (total + per_page - 1) / per_page;
        let page = page.max(1).min(total_pages);

        // Reverse to show newest first.
        let mut reversed: Vec<McpLogEntry> = inner.entries.iter().rev().cloned().collect();
        let start = (page - 1) * per_page;
        let end = (start + per_page).min(reversed.len());
        let logs = if start < reversed.len() {
            reversed.drain(start..end).collect()
        } else {
            vec![]
        };

        PaginatedLogs {
            logs,
            total,
            page,
            per_page,
            total_pages,
        }
    }

    /// Fetch a single log entry by id.
    pub fn get(&self, id: u64) -> Option<McpLogEntry> {
        let inner = self.inner.lock().unwrap();
        inner.entries.iter().find(|e| e.id == id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_push_and_list() {
        let store = McpLogStore::new();
        store.push("tools/call".into(), json!({"id":1}), json!({"ok":true}), 42, false);
        store.push("initialize".into(), json!({"id":2}), json!({"ok":true}), 5, false);

        let page = store.list(1, 10);
        assert_eq!(page.total, 2);
        assert_eq!(page.logs.len(), 2);
        // Newest first
        assert_eq!(page.logs[0].method, "initialize");
        assert_eq!(page.logs[1].method, "tools/call");
    }

    #[test]
    fn test_get_by_id() {
        let store = McpLogStore::new();
        store.push("ping".into(), json!({}), json!({}), 1, false);
        let entry = store.get(1).expect("should exist");
        assert_eq!(entry.method, "ping");
        assert!(store.get(999).is_none());
    }

    #[test]
    fn test_eviction() {
        let store = McpLogStore::new();
        for i in 0..250 {
            store.push(format!("m{}", i), json!({}), json!({}), 0, false);
        }
        let page = store.list(1, 300);
        assert_eq!(page.total, 200); // MAX_LOG_ENTRIES
    }

    #[test]
    fn test_pagination() {
        let store = McpLogStore::new();
        for i in 0..25 {
            store.push(format!("m{}", i), json!({}), json!({}), 0, false);
        }
        let p1 = store.list(1, 10);
        assert_eq!(p1.logs.len(), 10);
        assert_eq!(p1.total_pages, 3);
        assert_eq!(p1.page, 1);

        let p3 = store.list(3, 10);
        assert_eq!(p3.logs.len(), 5);
    }

    #[test]
    fn test_is_error_flag() {
        let store = McpLogStore::new();
        store.push("bad".into(), json!({}), json!({"error":"x"}), 0, true);
        let entry = store.get(1).unwrap();
        assert!(entry.is_error);
    }
}
