use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::config::ApiConfig;

pub struct Middleware
{
    seen: HashMap<String, Instant>,
    repeat: Duration
}

impl Middleware
{
    pub fn new(cfg: &ApiConfig) -> Self
    {
        return Self
        {
            seen: HashMap::new(),
            repeat: Duration::from_millis(cfg.repeat_time_ms)
        };
    }

    // true — код можно отправлять, false — недавно уже отправляли.
    pub fn allow(&mut self, code: &str) -> bool
    {
        let now = Instant::now();

        self.seen.retain(|_, time| now.duration_since(*time) < self.repeat);

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
