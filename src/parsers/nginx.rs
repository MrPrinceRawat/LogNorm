use anyhow::Result;
use memchr::memchr_iter;
use std::ops::Range;

use crate::config::LogEntry;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Tunables
const CHUNK_BYTES: usize = 4 * 1024 * 1024; // 4MB chunk default (tune to CPU/cache)
const MIN_LINE_LEN: usize = 20; // skip tiny lines

// constant strings for service/levels
static SERVICE_NGINX: &str = "nginx";
static LEVEL_INFO: &str = "info";
static LEVEL_WARN: &str = "warn";
static LEVEL_ERROR: &str = "error";

pub fn parse_nginx(input: &str) -> Result<Vec<LogEntry>> {
    // Fast path: if parallel feature enabled and input large -> parallel
    #[cfg(feature = "parallel")]
    {
        if input.len() > CHUNK_BYTES {
            return Ok(parse_parallel(input));
        }
    }

    // Single-thread fallback
    Ok(parse_single(input.as_bytes()))
}

#[cfg(feature = "parallel")]
fn parse_parallel(input: &str) -> Vec<LogEntry> {
    let bytes = input.as_bytes();
    let len = bytes.len();

    // build ranges aligned to newline boundaries
    let mut ranges = Vec::<Range<usize>>::new();
    let mut start = 0usize;

    while start < len {
        let mut end = (start + CHUNK_BYTES).min(len);
        if end < len {
            // advance end to include the rest of the current line
            while end < len && bytes[end] != b'\n' {
                end += 1;
            }
            if end < len {
                end += 1; // include newline
            }
        } else {
            end = len;
        }
        ranges.push(start..end);
        start = end;
    }

    // Parse each range in parallel, each returns Vec<LogEntry>
    let parts: Vec<Vec<LogEntry>> = ranges
        .into_par_iter()
        .map(|r| {
            let approx_lines = (r.end - r.start) / 80;
            let mut out = Vec::with_capacity(approx_lines);
            parse_chunk_into_vec(&bytes[r], &mut out);
            out
        })
        .collect();

    // flatten vectors (preserves grouping)
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

/// Parse a chunk (doesn't contain split lines) and push LogEntry into `out`.
fn parse_chunk_into_vec(bytes: &[u8], out: &mut Vec<LogEntry>) {
    let mut start = 0usize;
    for nl in memchr_iter(b'\n', bytes) {
        let line = &bytes[start..nl];
        if line.len() >= MIN_LINE_LEN {
            if let Some(entry) = parse_line_to_logentry(line) {
                out.push(entry);
            }
        }
        start = nl + 1;
    }
    if start < bytes.len() {
        let line = &bytes[start..];
        if line.len() >= MIN_LINE_LEN {
            if let Some(entry) = parse_line_to_logentry(line) {
                out.push(entry);
            }
        }
    }
}

/// Parse single log line (byte slice). Returns owned `LogEntry`.
/// Assumes original input was valid UTF-8 (we use unchecked conversions).
fn parse_line_to_logentry(line: &[u8]) -> Option<LogEntry> {
    let s = unsafe { std::str::from_utf8_unchecked(line) };
    let bytes = line;
    let len = bytes.len();

    let mut ip_end = None;
    let mut ts_start = None;
    let mut ts_end = None;
    let mut req_start = None;
    let mut req_end = None;

    let mut i = 0usize;
    while i < len {
        match bytes[i] {
            b' ' if ip_end.is_none() => ip_end = Some(i),
            b'[' if ts_start.is_none() => ts_start = Some(i + 1),
            b']' if ts_start.is_some() && ts_end.is_none() => ts_end = Some(i),
            b'"' if ts_end.is_some() && req_start.is_none() => req_start = Some(i + 1),
            b'"' if req_start.is_some() && req_end.is_none() => {
                req_end = Some(i);
                break;
            }
            _ => {}
        }
        i += 1;
    }

    let ip_end = ip_end?;
    let ts_start = ts_start?;
    let ts_end = ts_end?;
    let req_start = req_start?;
    let req_end = req_end?;

    let ip = unsafe { std::str::from_utf8_unchecked(&bytes[..ip_end]) };
    let timestamp = unsafe { std::str::from_utf8_unchecked(&bytes[ts_start..ts_end]) };
    let request = unsafe { std::str::from_utf8_unchecked(&bytes[req_start..req_end]) };

    // parse status three-digit after request closing quote
    let mut status_start = req_end + 1;
    while status_start < len && bytes[status_start] == b' ' {
        status_start += 1;
    }
    if status_start + 3 > len {
        return None;
    }
    let status_slice = &bytes[status_start..status_start + 3];
    let status = unsafe { std::str::from_utf8_unchecked(status_slice) };
    let status_num = fast_parse_status(status)?;
    let level_static = if (400..500).contains(&status_num) {
        LEVEL_WARN
    } else if status_num >= 500 {
        LEVEL_ERROR
    } else {
        LEVEL_INFO
    };

    let (method, path) = if let Some(space_idx) = request.find(' ') {
        let method = &request[..space_idx];
        let rest = &request[space_idx + 1..];
        let path = rest.split(' ').next().unwrap_or("");
        (method, path)
    } else {
        (request, "")
    };

    let mut msg = String::with_capacity(method.len() + 1 + path.len() + 4);
    msg.push_str(method);
    msg.push(' ');
    msg.push_str(path);
    msg.push_str(" -> ");
    msg.push_str(status);

    Some(LogEntry {
        timestamp: Some(timestamp.to_string()),
        host: Some(ip.to_string()),
        service: Some(SERVICE_NGINX.to_string()),
        level: Some(level_static.to_string()),
        message: Some(msg),
    })
}

#[inline(always)]
fn fast_parse_status(status_str: &str) -> Option<usize> {
    let b = status_str.as_bytes();
    if b.len() != 3 {
        return None;
    }
    let a = (b[0].wrapping_sub(b'0')) as usize;
    let c = (b[1].wrapping_sub(b'0')) as usize;
    let d = (b[2].wrapping_sub(b'0')) as usize;
    if a > 9 || c > 9 || d > 9 {
        return None;
    }
    Some(a * 100 + c * 10 + d)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"127.0.0.1 - - [12/May/2025:06:25:24 +0000] "GET /index.html HTTP/1.1" 200 612 "-" "curl/7.68.0"
192.168.1.10 - - [12/May/2025:06:25:25 +0000] "POST /api/v1/data HTTP/1.1" 500 12 "-" "UA"
"#;

    #[test]
    fn parse_sample() {
        let v = parse_nginx(SAMPLE).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].host.as_deref(), Some("127.0.0.1"));
        assert_eq!(v[0].level.as_deref(), Some("info"));
        assert_eq!(v[1].level.as_deref(), Some("error"));
        assert!(v[1]
            .message
            .as_deref()
            .unwrap()
            .contains("POST /api/v1/data -> 500"));
    }
}
