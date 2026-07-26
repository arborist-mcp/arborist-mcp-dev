use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub(crate) fn source_fingerprint(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish() & i64::MAX as u64
}

#[cfg(test)]
mod tests {
    use super::source_fingerprint;

    #[test]
    fn source_fingerprint_fits_persisted_sqlite_integer_range() {
        assert!(source_fingerprint("sample source") <= i64::MAX as u64);
    }
}
