pub mod apache;
pub mod journalctl;
pub mod nginx;
pub mod python_web;
pub mod syslog;

use crate::config::LogEntry;
use anyhow::{anyhow, Result};

pub fn parse(parser: &str, input: &str) -> Result<Vec<LogEntry>> {
    match parser {
        "syslog" => syslog::parse_syslog(input),
        "nginx" => nginx::parse_nginx(input),
        "apache" => apache::parse_apache(input),
        "journalctl" => journalctl::parse_journal(input),
        "python_web" => python_web::parse_python_logs(input),
        _ => Err(anyhow!("Unknown parser: {}", parser)),
    }
}
