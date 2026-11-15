use serde_json::Value;

pub fn is_valid_json(input: &str) -> bool {
    serde_json::from_str::<Value>(input).is_ok()
}
