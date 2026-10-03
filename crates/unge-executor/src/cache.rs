use crate::{NodeExecutor, Outputs};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    io::{self, Write},
    sync::Arc,
};

/// Serialized key + output bytes; excludes allocator overhead and host resource data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheLimits {
    pub max_entries: usize,
    pub max_bytes: usize,
}
impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            max_entries: 128,
            max_bytes: 16 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheUsage {
    pub entries: usize,
    pub bytes: usize,
    pub limits: CacheLimits,
}
struct Entry {
    outputs: Outputs,
    bytes: usize,
    // Keep executor identity alive so an address cannot be recycled while cached.
    _executor: Arc<dyn NodeExecutor>,
}
pub(crate) struct Cache {
    entries: HashMap<Arc<str>, Entry>,
    order: VecDeque<Arc<str>>,
    bytes: usize,
    limits: CacheLimits,
}
impl Cache {
    pub fn new(limits: CacheLimits) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            limits,
        }
    }
    pub fn enabled(&self) -> bool {
        self.limits.max_entries > 0 && self.limits.max_bytes > 0
    }
    pub fn usage(&self) -> CacheUsage {
        CacheUsage {
            entries: self.entries.len(),
            bytes: self.bytes,
            limits: self.limits,
        }
    }
    pub fn clear(&mut self) {
        self.entries = HashMap::new();
        self.order = VecDeque::new();
        self.bytes = 0;
    }
    pub fn set_limits(&mut self, limits: CacheLimits) {
        self.limits = limits;
        if !self.enabled() {
            self.clear();
            return;
        }
        while self.entries.len() > limits.max_entries || self.bytes > limits.max_bytes {
            self.evict();
        }
    }
    pub fn key(&self, value: &impl Serialize) -> Option<String> {
        let mut writer = BoundedWriter {
            limit: self.limits.max_bytes,
            bytes: 0,
            data: Some(Vec::new()),
        };
        serde_json::to_writer(&mut writer, value).ok()?;
        String::from_utf8(writer.data?).ok()
    }
    pub fn get(&self, key: &str) -> Option<Outputs> {
        self.entries.get(key).map(|entry| entry.outputs.clone())
    }
    pub fn insert(&mut self, key: String, outputs: &Outputs, executor: Arc<dyn NodeExecutor>) {
        // Same-layer duplicate computations share one entry without refreshing FIFO order.
        if !self.enabled() || self.entries.contains_key(key.as_str()) {
            return;
        }
        let mut writer = BoundedWriter {
            limit: self.limits.max_bytes.saturating_sub(key.len()),
            bytes: 0,
            data: None,
        };
        if serde_json::to_writer(&mut writer, outputs).is_err() {
            return;
        }
        let bytes = key.len() + writer.bytes;
        while self.entries.len() >= self.limits.max_entries
            || self.bytes > self.limits.max_bytes - bytes
        {
            self.evict();
        }
        let key: Arc<str> = key.into();
        self.order.push_back(key.clone());
        self.entries.insert(
            key,
            Entry {
                outputs: outputs.clone(),
                bytes,
                _executor: executor,
            },
        );
        self.bytes += bytes;
    }
    fn evict(&mut self) {
        if let Some(key) = self.order.pop_front() {
            self.bytes -= self
                .entries
                .remove(&key)
                .expect("cache FIFO invariant")
                .bytes;
        }
    }
}
// Count outputs without creating another serialized output buffer; stop at the budget.
struct BoundedWriter {
    limit: usize,
    bytes: usize,
    data: Option<Vec<u8>>,
}
impl Write for BoundedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.limit.saturating_sub(self.bytes) {
            return Err(io::Error::other("cache payload exceeds byte limit"));
        }
        if let Some(data) = &mut self.data {
            data.extend_from_slice(buf);
        }
        self.bytes += buf.len();
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
