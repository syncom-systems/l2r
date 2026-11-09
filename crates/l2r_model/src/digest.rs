//! Content digests and canonical encoding.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Returns `sha256:<hex>` for `bytes`.
pub fn sha256(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for byte in hash {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Encodes `value` as compact JSON with a fixed field order.
///
/// Field order follows struct declaration order and every map in the encoded
/// types is a `BTreeMap`, so the encoding does not depend on hash iteration.
pub fn canonical_json<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("model types always serialize")
}

/// Returns the digest of the canonical JSON encoding of `value`.
pub fn digest_of<T: Serialize>(value: &T) -> String {
    sha256(&canonical_json(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_empty_input_is_the_published_constant() {
        assert_eq!(
            sha256(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
