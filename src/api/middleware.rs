use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::config::ApiConfig;

pub struct Middleware
{
    seen: HashMap<String, Instant>,
    repeat: Duration,
    cleaned: Instant
}

impl Middleware
{
    pub fn new(cfg: &ApiConfig) -> Self
    {
        return Self
        {
            seen: HashMap::new(),
            repeat: Duration::from_millis(cfg.repeat_time_ms),
            cleaned: Instant::now()
        };
    }
    pub fn allow(&mut self, code: &str) -> bool
    {
        let now = Instant::now();

        if now.duration_since(self.cleaned) >= self.repeat
        {
            self.seen.retain(|_, time| now.duration_since(*time) < self.repeat);
            self.cleaned = now;
        }

        if let Some(time) = self.seen.get(code)
        {
            if now.duration_since(*time) < self.repeat
            {
                return false;
            }
        }

        self.seen.insert(code.to_string(), now);
        return true;
    }
}
