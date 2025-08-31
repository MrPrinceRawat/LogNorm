use crate::config::LogEntry;
use anyhow::Result;
use memchr::memchr_iter;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::ops::Range;

const CHUNK_BYTES: usize = 4 * 1024 * 1024; // 8MB per chunk
const MIN_LINE_LEN: usize = 20;

static SERVICE_JOURNAL: &str = "journalctl";
static LEVEL_INFO: &str = "info";
static LEVEL_WARN: &str = "warn";
static LEVEL_ERROR: &str = "error";

pub fn parse_journal(input: &str) -> Result<Vec<LogEntry>> {
    #[cfg(feature = "parallel")]
    {
        if input.len() > CHUNK_BYTES {
            return Ok(parse_parallel(input));
        }
    }
    Ok(parse_single(input.as_bytes()))
}

#[cfg(feature = "parallel")]
fn parse_parallel(input: &str) -> Vec<LogEntry> {
    let bytes = input.as_bytes();
    let len = bytes.len();

    // Build chunk ranges aligned to newline
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < len {
        let mut end = (start + CHUNK_BYTES).min(len);
        if end < len {
            while end < len && bytes[end] != b'\n' {
                end += 1;
            }
            if end < len {
                end += 1;
            }
        } else {
            end = len;
        }
        ranges.push(start..end);
        start = end;
    }

    // Parse chunks in parallel
    let parts: Vec<Vec<LogEntry>> = ranges
        .into_par_iter()
        .map(|r| {
            let approx_lines = (r.end - r.start) / 80;
            let mut out = Vec::with_capacity(approx_lines);
            parse_chunk(&bytes[r], &mut out);
            out
        })
        .collect();

    // Flatten
    let total: usize = parts.iter().map(Vec::len).sum();
    let mut result = Vec::with_capacity(total);
    for mut p in parts {
        result.append(&mut p);
    }
    result
}

fn parse_single(bytes: &[u8]) -> Vec<LogEntry> {
    let mut out = Vec::with_capacity(bytes.len() / 80);
    parse_chunk(bytes, &mut out);
    out
}

fn parse_chunk(bytes: &[u8], out: &mut Vec<LogEntry>) {
    let mut start = 0;
    for nl in memchr_iter(b'\n', bytes) {
        let line = &bytes[start..nl];
        if line.len() >= MIN_LINE_LEN {
            if let Some(entry) = parse_line(line) {
                out.push(entry);
            }
        }
        start = nl + 1;
    }
    if start < bytes.len() {
        let line = &bytes[start..];
        if line.len() >= MIN_LINE_LEN {
            if let Some(entry) = parse_line(line) {
                out.push(entry);
            }
        }
    }
}

/// Zero-copy parser: no `.to_lowercase()`
fn parse_line(line: &[u8]) -> Option<LogEntry> {
    let len = line.len();
    if len < 16 {
        return None;
    }

    // Timestamp: first 15 bytes
    let timestamp = unsafe { std::str::from_utf8_unchecked(&line[0..15]) };

    // Hostname: next token
    let mut i = 16;
    let host_start = i;
    while i < len && line[i] != b' ' {
        i += 1;
    }
    let hostname = unsafe { std::str::from_utf8_unchecked(&line[host_start..i]) };
    i += 1;

    // Service: up to ':' or '['
    let svc_start = i;
    while i < len && line[i] != b':' && line[i] != b'[' {
        i += 1;
    }
    let service = unsafe { std::str::from_utf8_unchecked(&line[svc_start..i]) };
    i += 1;

    // Remaining message
    let message = if i < len {
        unsafe { std::str::from_utf8_unchecked(&line[i..]) }
    } else {
        ""
    };

    // Level heuristics without allocations
    let msg_bytes = message.as_bytes();
    let mut level = LEVEL_INFO;
    for &b in msg_bytes {
        match b | 0x20 {
            // lowercase ASCII
            b'e' => {
                if msg_bytes
                    .windows(5)
                    .any(|w| w.eq_ignore_ascii_case(b"error"))
                {
                    level = LEVEL_ERROR;
                    break;
                }
            }
            b'f' => {
                if msg_bytes
                    .windows(4)
                    .any(|w| w.eq_ignore_ascii_case(b"fail"))
                {
                    level = LEVEL_ERROR;
                    break;
                }
            }
            b'w' => {
                if msg_bytes
                    .windows(4)
                    .any(|w| w.eq_ignore_ascii_case(b"warn"))
                {
                    level = LEVEL_WARN;
                    break;
                }
            }
            _ => {}
        }
    }

    Some(LogEntry {
        timestamp: Some(timestamp.to_string()),
        host: Some(hostname.to_string()),
        service: Some(SERVICE_JOURNAL.to_string()),
        level: Some(level.to_string()),
        message: Some(message.to_string()),
    })
}
