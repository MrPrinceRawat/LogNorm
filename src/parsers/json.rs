pub fn parse(line: &str) -> Option<serde_json::Value> {
    serde_json::from_str(line).ok()
}
