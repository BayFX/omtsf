/// Detected serialization encoding of an `.omts` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// JSON encoding (ECMA-404 / RFC 8259).
    Json,
    /// CBOR encoding (RFC 8949), identified by self-describing tag 55799.
    Cbor,
    /// zstd-compressed payload; decompress then re-detect the inner encoding.
    Zstd,
}

/// Error returned when the initial bytes of a file do not match any known encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingDetectionError {
    /// The first bytes that were inspected (up to 4 bytes).
    pub first_bytes: Vec<u8>,
}

impl std::fmt::Display for EncodingDetectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unrecognized encoding: first bytes are {:02X?}",
            self.first_bytes
        )
    }
}

impl std::error::Error for EncodingDetectionError {}

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];
const CBOR_TAG: [u8; 3] = [0xD9, 0xD9, 0xF7];
const JSON_OPEN_BRACE: u8 = 0x7B;
const JSON_WHITESPACE: [u8; 4] = [0x09, 0x0A, 0x0D, 0x20];

/// Detects the encoding of an `.omts` file by inspecting its initial bytes.
///
/// Detection order per SPEC-007 Section 2:
/// 1. zstd magic (`0x28 0xB5 0x2F 0xFD`) → [`Encoding::Zstd`]
/// 2. CBOR self-describing tag 55799 (`0xD9 0xD9 0xF7`) → [`Encoding::Cbor`]
/// 3. First non-whitespace byte is `{` (`0x7B`) → [`Encoding::Json`]
///
/// Returns [`EncodingDetectionError`] if the bytes match none of the above.
pub fn detect_encoding(bytes: &[u8]) -> Result<Encoding, EncodingDetectionError> {
    if bytes.starts_with(&ZSTD_MAGIC) {
        return Ok(Encoding::Zstd);
    }

    if bytes.starts_with(&CBOR_TAG) {
        return Ok(Encoding::Cbor);
    }

    for &byte in bytes {
        if JSON_WHITESPACE.contains(&byte) {
            continue;
        }
        if byte == JSON_OPEN_BRACE {
            return Ok(Encoding::Json);
        }
        break;
    }

    Err(EncodingDetectionError {
        first_bytes: bytes.iter().copied().take(4).collect(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn detect_zstd() {
        let input = [0x28u8, 0xB5, 0x2F, 0xFD, 0x00, 0x00];
        assert_eq!(detect_encoding(&input).expect("zstd"), Encoding::Zstd);
    }

    #[test]
    fn detect_cbor() {
        let input = [0xD9u8, 0xD9, 0xF7, 0xA1, 0x00];
        assert_eq!(detect_encoding(&input).expect("cbor"), Encoding::Cbor);
    }

    #[test]
    fn detect_json_no_whitespace() {
        let input = b"{\"omtsf_version\":\"1.0\"}";
        assert_eq!(detect_encoding(input).expect("json"), Encoding::Json);
    }

    #[test]
    fn detect_json_with_leading_space() {
        let input = b"  {\"omtsf_version\":\"1.0\"}";
        assert_eq!(
            detect_encoding(input).expect("json with space"),
            Encoding::Json
        );
    }

    #[test]
    fn detect_json_with_all_whitespace_chars() {
        let mut input = Vec::new();
        input.extend_from_slice(&[0x20, 0x09, 0x0A, 0x0D]);
        input.extend_from_slice(b"{\"omtsf_version\":\"1.0\"}");
        assert_eq!(
            detect_encoding(&input).expect("json all whitespace"),
            Encoding::Json
        );
    }

    #[test]
    fn detect_unknown_bytes_returns_error() {
        let input = [0xFFu8, 0x00, 0x01, 0x02];
        let err = detect_encoding(&input).expect_err("should fail");
        assert_eq!(err.first_bytes, vec![0xFF, 0x00, 0x01, 0x02]);
    }

    #[test]
    fn detect_empty_returns_error() {
        let err = detect_encoding(&[]).expect_err("empty should fail");
        assert!(err.first_bytes.is_empty());
    }

    #[test]
    fn detect_only_whitespace_returns_error() {
        let input = [0x20u8, 0x09, 0x0A, 0x0D];
        detect_encoding(&input).expect_err("only whitespace should fail");
    }

    #[test]
    fn detect_wrong_first_byte_after_whitespace_returns_error() {
        let input = b"  [1,2,3]";
        detect_encoding(input).expect_err("array is not a valid omts opener");
    }

    #[test]
    fn error_display_contains_hex() {
        let input = [0xABu8, 0xCD];
        let err = detect_encoding(&input).expect_err("should fail");
        let msg = err.to_string();
        assert!(msg.contains("AB"), "display should include hex: {msg}");
        assert!(msg.contains("CD"), "display should include hex: {msg}");
    }

    #[test]
    fn detect_zstd_exact_magic_only() {
        let input = [0x28u8, 0xB5, 0x2F, 0xFD];
        assert_eq!(
            detect_encoding(&input).expect("zstd exact magic"),
            Encoding::Zstd
        );
    }

    #[test]
    fn detect_cbor_exact_tag_only() {
        let input = [0xD9u8, 0xD9, 0xF7];
        assert_eq!(
            detect_encoding(&input).expect("cbor exact tag"),
            Encoding::Cbor
        );
    }

    #[test]
    fn detect_json_with_leading_tab() {
        let input = b"\t{\"key\":\"val\"}";
        assert_eq!(
            detect_encoding(input).expect("tab-prefixed json"),
            Encoding::Json
        );
    }

    #[test]
    fn detect_json_with_leading_cr() {
        let input = b"\r{\"key\":\"val\"}";
        assert_eq!(
            detect_encoding(input).expect("cr-prefixed json"),
            Encoding::Json
        );
    }

    #[test]
    fn detect_error_stores_at_most_four_bytes() {
        let input = [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06];
        let err = detect_encoding(&input).expect_err("should fail");
        assert_eq!(err.first_bytes.len(), 4);
    }

    #[test]
    fn detect_error_stores_fewer_when_input_short() {
        let input = [0x01u8, 0x02];
        let err = detect_encoding(&input).expect_err("should fail");
        assert_eq!(err.first_bytes, vec![0x01, 0x02]);
    }

    #[test]
    fn encoding_eq_and_clone() {
        assert_eq!(Encoding::Json, Encoding::Json.clone());
        assert_ne!(Encoding::Json, Encoding::Cbor);
        assert_ne!(Encoding::Cbor, Encoding::Zstd);
    }
}
