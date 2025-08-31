use crate::config::LogEntry;
use anyhow::Result;
use memchr::memchr_iter;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

const CHUNK_BYTES: usize = 4 * 1024 * 1024;
const MIN_LINE_LEN: usize = 15;

static SERVICE_SYSLOG: &str = "syslog";
static LEVEL_INFO: &str = "info";
static LEVEL_WARN: &str = "warn";
static LEVEL_ERROR: &str = "error";

pub fn parse_syslog(input: &str) -> Result<Vec<LogEntry>> {
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

    let parts: Vec<Vec<LogEntry>> = ranges
        .into_par_iter()
        .map(|r| {
            let mut out = Vec::with_capacity((r.end - r.start) / 80);
            parse_chunk_into_vec(&bytes[r], &mut out);
            out
        })
        .collect();

    let total: usize = parts.iter().map(Vec::len).sum();
    let mut result = Vec::with_capacity(total);
    for mut p in parts {
        result.append(&mut p);
    }
    result
}

fn parse_single(bytes: &[u8]) -> Vec<LogEntry> {
    let mut out = Vec::with_capacity(bytes.len() / 80);
    parse_chunk_into_vec(bytes, &mut out);
    out
}

fn parse_chunk_into_vec(bytes: &[u8], out: &mut Vec<LogEntry>) {
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

fn parse_line(line: &[u8]) -> Option<LogEntry> {
    let bytes = line;
    let len = bytes.len();
    let mut i = 0;

    if line.get(0) == Some(&b'<') {
        while i < len && bytes[i] != b'>' {
            i += 1;
        }
        i += 1;
    }

    if i + 15 > len {
        return None;
    }
    let timestamp = unsafe { std::str::from_utf8_unchecked(&bytes[i..i + 15]) };
    i += 16;

    let host_start = i;
    while i < len && bytes[i] != b' ' {
        i += 1;
    }
    if i >= len {
        return None;
    }
    let hostname = unsafe { std::str::from_utf8_unchecked(&bytes[host_start..i]) };
    i += 1;

    let app_start = i;
    while i < len && bytes[i] != b':' {
        i += 1;
    }
    if i >= len {
        return None;
    }
    let app = unsafe { std::str::from_utf8_unchecked(&bytes[app_start..i]) };
    i += 2;

    let message = if i < len {
        unsafe { std::str::from_utf8_unchecked(&bytes[i..]) }
    } else {
        ""
    };

    // Level detection without allocating new String
    let msg_bytes = message.as_bytes();
    let mut level_static = LEVEL_INFO;
    for b in msg_bytes {
        match *b {
            b'E' | b'e' => {
                if message.to_lowercase().contains("error")
                    || message.to_lowercase().contains("fail")
                {
                    level_static = LEVEL_ERROR;
                    break;
                }
            }
            b'W' | b'w' => {
                if message.to_lowercase().contains("warn") {
                    level_static = LEVEL_WARN;
                }
            }
            _ => {}
        }
    }

    Some(LogEntry {
        timestamp: Some(timestamp.to_string()),
        host: Some(hostname.to_string()),
        service: Some(SERVICE_SYSLOG.to_string()),
        level: Some(level_static.to_string()),
        message: Some(format!("{}: {}", app, message)),
    })
}
