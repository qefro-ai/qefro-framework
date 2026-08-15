use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Pluggable rate limiter. In-memory for development; a Redis adapter can
/// implement this later without changing callers.
pub trait RateLimiter: Send + Sync {
    fn allow(&self, key: &str) -> bool;
}

pub struct MemoryRateLimiter {
    inner: Mutex<HashMap<String, Window>>,
    limit: u32,
    window: Duration,
}

struct Window {
    count: u32,
    start: Instant,
}

impl MemoryRateLimiter {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            limit,
            window,
        }
    }

    pub fn disabled() -> Self {
        Self::new(u32::MAX, Duration::from_secs(60))
    }
}

impl Default for MemoryRateLimiter {
    fn default() -> Self {
        Self::new(1_000, Duration::from_secs(60))
    }
}

impl RateLimiter for MemoryRateLimiter {
    fn allow(&self, key: &str) -> bool {
        if self.limit == u32::MAX {
            return true;
        }
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert(Window {
            count: 0,
            start: now,
        });
        if now.duration_since(entry.start) >= self.window {
            entry.count = 0;
            entry.start = now;
        }
        if entry.count >= self.limit {
            return false;
        }
        entry.count += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_limit() {
        let lim = MemoryRateLimiter::new(2, Duration::from_secs(60));
        assert!(lim.allow("t1:/x"));
        assert!(lim.allow("t1:/x"));
        assert!(!lim.allow("t1:/x"));
        assert!(lim.allow("t2:/x"));
    }
}
