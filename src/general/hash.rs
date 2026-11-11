use std::hash::{DefaultHasher, Hash as _, Hasher as _};

pub fn deterministic_number_from_string(input: &str, min: u32, max: u32) -> u32 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    let hash_value = hasher.finish();

    // Scale the hash value to the range [min, max]
    min + (hash_value % (max - min + 1) as u64) as u32
}
