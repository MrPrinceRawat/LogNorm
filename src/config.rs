use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LogEntry {
    pub timestamp: Option<String>,
    pub host: Option<String>,
    pub service: Option<String>,
    pub level: Option<String>,
    pub message: Option<String>,
}
