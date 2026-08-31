use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Outcome of a rate-limit check. `remaining` and `retry_after` are for
/// HTTP headers; they must not include the key (keys can contain emails).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub retry_after: Option<Duration>,
    pub window: Duration,
}

impl RateLimitDecision {
    pub fn retry_after_secs(&self) -> u64 {
        self.retry_after.map(|d| d.as_secs().max(1)).unwrap_or(1)
    }
}

/// Persistence boundary for counters. The in-memory store is the default.
/// A Redis (or similar) adapter can implement this later without changing
/// HTTP callers. Single-instance deployments share one process map.
/// Multi-instance deployments do **not** share counters unless a distributed
/// store is configured — put a reverse proxy limiter in front, or provide
/// another `RateLimitStore`.
pub trait RateLimitStore: Send + Sync {
    fn hit(&self, key: &str, limit: u32, window: Duration) -> RateLimitDecision;
}

/// Caller-facing limiter with a configured limit/window.
pub trait RateLimiter: Send + Sync {
    fn allow(&self, key: &str) -> bool;
    fn check(&self, key: &str) -> RateLimitDecision;
}

struct Window {
    count: u32,
    start: Instant,
}

/// Process-local sliding-fixed window. Not a distributed limiter.
pub struct MemoryRateLimiter {
    inner: Mutex<HashMap<String, Window>>,
    limit: u32,
    window: Duration,
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

    fn prune_if_needed(map: &mut HashMap<String, Window>, window: Duration, now: Instant) {
        if map.len() < 8_192 {
            return;
        }
        map.retain(|_, entry| now.duration_since(entry.start) < window);
    }
}

impl Default for MemoryRateLimiter {
    fn default() -> Self {
        Self::new(1_000, Duration::from_secs(60))
    }
}

impl RateLimitStore for MemoryRateLimiter {
    fn hit(&self, key: &str, limit: u32, window: Duration) -> RateLimitDecision {
        if limit == u32::MAX {
            return RateLimitDecision {
                allowed: true,
                limit,
                remaining: u32::MAX,
                retry_after: None,
                window,
            };
        }
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        Self::prune_if_needed(&mut map, window, now);
        let entry = map.entry(key.to_string()).or_insert(Window {
            count: 0,
            start: now,
        });
        if now.duration_since(entry.start) >= window {
            entry.count = 0;
            entry.start = now;
        }
        if entry.count >= limit {
            let elapsed = now.duration_since(entry.start);
            let retry = window.saturating_sub(elapsed);
            return RateLimitDecision {
                allowed: false,
                limit,
                remaining: 0,
                retry_after: Some(retry),
                window,
            };
        }
        entry.count += 1;
        RateLimitDecision {
            allowed: true,
            limit,
            remaining: limit.saturating_sub(entry.count),
            retry_after: None,
            window,
        }
    }
}

impl RateLimiter for MemoryRateLimiter {
    fn allow(&self, key: &str) -> bool {
        self.check(key).allowed
    }

    fn check(&self, key: &str) -> RateLimitDecision {
        self.hit(key, self.limit, self.window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_limit() {
        let lim = MemoryRateLimiter::new(2, Duration::from_secs(60));
        assert!(lim.allow("t1:/x"));
        let second = lim.check("t1:/x");
        assert!(second.allowed);
        assert_eq!(second.remaining, 0);
        let denied = lim.check("t1:/x");
        assert!(!denied.allowed);
        assert_eq!(denied.remaining, 0);
        assert!(denied.retry_after.is_some());
        assert!(lim.allow("t2:/x"));
    }

    #[test]
    fn store_can_use_per_call_limit() {
        let store = MemoryRateLimiter::default();
        let tight = store.hit("login:a@b.c", 1, Duration::from_secs(60));
        assert!(tight.allowed);
        let again = store.hit("login:a@b.c", 1, Duration::from_secs(60));
        assert!(!again.allowed);
    }
}
