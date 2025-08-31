use crate::config::LogEntry;

pub fn normalize(entries: Vec<LogEntry>) -> Vec<LogEntry> {
    entries
        .into_iter()
        .map(|mut e| {
            // Example: normalize timestamps (TODO)
            if let Some(ts) = e.timestamp.clone() {
                e.timestamp = Some(ts); // convert to ISO-8601 later
            }
            e
        })
        .collect()
}
