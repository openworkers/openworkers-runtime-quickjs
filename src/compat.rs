/// Script wrapper for compatibility with other runtimes
#[derive(Clone)]
pub struct Script {
    pub code: String,
}

impl Script {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_string(),
        }
    }
}

/// Runtime limits (placeholder for compatibility)
#[derive(Clone, Debug, Default)]
pub struct RuntimeLimits {
    pub max_memory_mb: Option<u64>,
    pub max_execution_time_ms: Option<u64>,
}

/// Log level for worker logs
#[derive(Clone, Debug)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Log event from worker
#[derive(Clone, Debug)]
pub struct LogEvent {
    pub level: LogLevel,
    pub message: String,
}

/// Reason for worker termination
#[derive(Clone, Debug)]
pub enum TerminationReason {
    Complete,
    Timeout,
    MemoryLimit,
    Error(String),
}
