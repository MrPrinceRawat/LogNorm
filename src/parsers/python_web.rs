use crate::config::LogEntry;
use anyhow::Result;
use memchr::memchr_iter;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

const CHUNK_BYTES: usize = 4 * 1024 * 1024; // 16 MB chunks
const MIN_LINE_LEN: usize = 20;

static SERVICE_PYTHON: &str = "python_web";
static LEVEL_INFO: &str = "info";
static LEVEL_WARN: &str = "warn";
static LEVEL_ERROR: &str = "error";

pub fn parse_python_logs(input: &str) -> Result<Vec<LogEntry>> {
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

    // split into newline-aligned chunks
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
            let approx_lines = (r.end - r.start) / 80;
            let mut out = Vec::with_capacity(approx_lines);
            parse_chunk(&bytes[r], &mut out);
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
    parse_chunk(bytes, &mut out);
    out
}

// fast chunk parser using memchr
fn parse_chunk(bytes: &[u8], out: &mut Vec<LogEntry>) {
    let mut start = 0;
    let mut line_starts = Vec::with_capacity(64);
    let mut line_ends = Vec::with_capacity(64);

    // collect line boundaries in batch
    for nl in memchr_iter(b'\n', bytes) {
        line_starts.push(start);
        line_ends.push(nl);
        start = nl + 1;
    }
    if start < bytes.len() {
        line_starts.push(start);
        line_ends.push(bytes.len());
    }

    // parse each line
    for (&s, &e) in line_starts.iter().zip(line_ends.iter()) {
        let line = &bytes[s..e];
        if line.len() >= MIN_LINE_LEN {
            if let Some(entry) = parse_line(line) {
                out.push(entry);
            }
        }
    }
}

// parse single line without allocations
fn parse_line(line: &[u8]) -> Option<LogEntry> {
    let s = unsafe { std::str::from_utf8_unchecked(line) };
    let len = s.len();
    let bytes = s.as_bytes();

    let mut i = 0;

    // Level (first token)
    while i < len && bytes[i] != b' ' {
        i += 1;
    }
    let level_str = &s[0..i];
    i += 1;

    // Timestamp (next token)
    let ts_start = i;
    while i < len && bytes[i] != b' ' {
        i += 1;
    }
    let timestamp = &s[ts_start..i];
    i += 1;

    // Skip microseconds if present
    if i < len && bytes[i] == b',' {
        while i < len && bytes[i] != b' ' {
            i += 1;
        }
        i += 1;
    }

    // Module name
    let mod_start = i;
    while i < len && bytes[i] != b' ' {
        i += 1;
    }
    let module = &s[mod_start..i];
    i += 1;

    // Message
    let message = if i < len { &s[i..] } else { "" };

    let level_static = match level_str.to_ascii_lowercase().as_str() {
        "error" => LEVEL_ERROR,
        "warn" | "warning" => LEVEL_WARN,
        _ => LEVEL_INFO,
    };

    Some(LogEntry {
        timestamp: Some(timestamp.to_string()),
        host: Some(module.to_string()),
        service: Some(SERVICE_PYTHON.to_string()),
        level: Some(level_static.to_string()),
        message: Some(message.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"INFO 2025-08-31 22:50:01,234 views.index Some log message
WARNING 2025-08-31 22:51:02,567 views.auth Something might be wrong
ERROR 2025-08-31 22:52:03,890 views.api Exception occurred
"#;

    #[test]
    fn parse_sample() {
        let v = parse_python_logs(SAMPLE).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].host.as_deref(), Some("views.index"));
        assert_eq!(v[1].level.as_deref(), Some("warn"));
        assert_eq!(v[2].level.as_deref(), Some("error"));
    }
}
