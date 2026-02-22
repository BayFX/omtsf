/// Boundary reference hashing for the selective disclosure / redaction engine.
///
/// This module implements Section 4 of the redaction specification:
/// - Deterministic path (Section 4.1, steps 3–5): sort canonical identifier
///   strings lexicographically by UTF-8 byte order, join with newline,
///   concatenate with decoded 32-byte file salt, compute SHA-256, and
///   hex-encode the digest.
/// - Random path (Section 4.1, step 6): 32 CSPRNG bytes hex-encoded when the
///   node has zero public identifiers.
///
/// Use [`boundary_ref_value`] as the primary entry point.
use std::fmt;

use sha2::{Digest, Sha256};

use crate::canonical::CanonicalId;
use crate::newtypes::FileSalt;

/// Errors that can occur when computing a boundary reference value.
#[derive(Debug)]
pub enum BoundaryHashError {
    /// The file salt hex string could not be decoded (not 64 lowercase hex chars).
    InvalidSalt(String),
    /// The platform CSPRNG failed when generating a random token.
    CsprngFailure(getrandom::Error),
}

impl fmt::Display for BoundaryHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSalt(s) => write!(f, "invalid file salt: {s}"),
            Self::CsprngFailure(e) => write!(f, "CSPRNG failure: {e}"),
        }
    }
}

impl std::error::Error for BoundaryHashError {}

/// Encodes a byte slice as a lowercase hexadecimal string.
fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX_CHARS[(b >> 4) as usize] as char);
        out.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    out
}

/// Decodes a 64-character lowercase hex string into a 32-byte array.
///
/// Returns `Err` if the string length is not exactly 64 or any character is
/// not a valid lowercase hex digit.
fn hex_decode_salt(s: &str) -> Result<[u8; 32], BoundaryHashError> {
    let bytes = s.as_bytes();
    if bytes.len() != 64 {
        return Err(BoundaryHashError::InvalidSalt(format!(
            "expected 64 hex chars, got {}",
            bytes.len()
        )));
    }
    let mut out = [0u8; 32];
    for (i, chunk) in bytes.chunks(2).enumerate() {
        let hi = hex_nibble(chunk[0]).ok_or_else(|| {
            BoundaryHashError::InvalidSalt(format!("invalid hex char 0x{:02x}", chunk[0]))
        })?;
        let lo = hex_nibble(chunk[1]).ok_or_else(|| {
            BoundaryHashError::InvalidSalt(format!("invalid hex char 0x{:02x}", chunk[1]))
        })?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

/// Returns the numeric value of a single lowercase hex ASCII byte, or `None`
/// if the byte is not a valid lowercase hex digit.
fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

/// Computes the boundary reference value for a node given its public
/// canonical identifiers and the decoded 32-byte file salt.
///
/// # Algorithm (Section 4.1)
///
/// - **Non-empty** `public_ids`: sort canonical strings lexicographically by
///   UTF-8 byte order, join with `\n`, concatenate with the 32 raw salt bytes,
///   compute SHA-256, return the 64-character lowercase hex digest.
/// - **Empty** `public_ids`: generate 32 bytes from the platform CSPRNG and
///   return the 64-character lowercase hex encoding.
///
/// # Errors
///
/// Returns [`BoundaryHashError::CsprngFailure`] if CSPRNG entropy is
/// unavailable (only possible on the random path when `public_ids` is empty).
pub fn boundary_ref_value(
    public_ids: &[CanonicalId],
    salt: &[u8; 32],
) -> Result<String, BoundaryHashError> {
    if public_ids.is_empty() {
        let mut buf = [0u8; 32];
        getrandom::getrandom(&mut buf).map_err(BoundaryHashError::CsprngFailure)?;
        return Ok(hex_encode(&buf));
    }

    let mut canonicals: Vec<&str> = public_ids.iter().map(CanonicalId::as_str).collect();
    canonicals.sort_unstable();

    let joined = canonicals.join("\n");

    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    hasher.update(salt.as_slice());

    Ok(hex_encode(&hasher.finalize()))
}

/// Decodes a [`FileSalt`] from its 64-character hex representation to a
/// 32-byte array suitable for use with [`boundary_ref_value`].
///
/// # Errors
///
/// Returns [`BoundaryHashError::InvalidSalt`] if the inner string is not valid
/// lowercase hex (this should not happen for a correctly constructed
/// [`FileSalt`], but is provided for defensive completeness).
pub fn decode_salt(salt: &FileSalt) -> Result<[u8; 32], BoundaryHashError> {
    hex_decode_salt(salt.as_ref())
}

/// Generates a fresh [`FileSalt`] using 32 CSPRNG bytes.
///
/// The result is a 64-character lowercase hex string suitable for use as the
/// `file_salt` field in a new OMTSF file (SPEC-001 Section 2).
///
/// # Errors
///
/// Returns [`BoundaryHashError::CsprngFailure`] if the platform CSPRNG is
/// unavailable.
pub fn generate_file_salt() -> Result<FileSalt, BoundaryHashError> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).map_err(BoundaryHashError::CsprngFailure)?;
    let hex = hex_encode(&buf);
    // SAFETY: hex_encode always produces exactly 64 lowercase hex characters,
    // which satisfies the FileSalt invariant.
    FileSalt::try_from(hex.as_str()).map_err(|e| BoundaryHashError::InvalidSalt(e.to_string()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeMap;

    use super::*;
    use crate::canonical::CanonicalId;
    use crate::types::Identifier;

    /// Test salt from spec Section 4.3:
    /// `0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff`
    const TEST_SALT_HEX: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    fn test_salt() -> [u8; 32] {
        hex_decode_salt(TEST_SALT_HEX).expect("test salt is valid")
    }

    /// Constructs a [`CanonicalId`] from an [`Identifier`] for test vector use.
    fn make_id(scheme: &str, value: &str, authority: Option<&str>) -> CanonicalId {
        let id = Identifier {
            scheme: scheme.to_owned(),
            value: value.to_owned(),
            authority: authority.map(str::to_owned),
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: BTreeMap::new(),
        };
        CanonicalId::from_identifier(&id)
    }

    #[test]
    fn tv1_multiple_public_identifiers() {
        let ids = vec![
            make_id("lei", "5493006MHB84DD0ZWV18", None),
            make_id("duns", "081466849", None),
        ];
        let result = boundary_ref_value(&ids, &test_salt()).expect("deterministic path succeeds");
        assert_eq!(
            result,
            "e8798687b081da98b7cd1c4e5e2423bd3214fbab0f1f476a2dcdbf67c2e21141"
        );
    }

    #[test]
    fn tv2_single_identifier() {
        let ids = vec![make_id("lei", "5493006MHB84DD0ZWV18", None)];
        let result = boundary_ref_value(&ids, &test_salt()).expect("deterministic path succeeds");
        assert_eq!(
            result,
            "7849e55c4381ba852a2ada50f15e58d871de085893b7be8826f75560854c78c8"
        );
    }

    #[test]
    fn tv3_percent_encoded_identifier() {
        // Value contains a literal colon; from_identifier will percent-encode it.
        let ids = vec![make_id("nat-reg", "HRB:86891", Some("RA000548"))];
        let result = boundary_ref_value(&ids, &test_salt()).expect("deterministic path succeeds");
        assert_eq!(
            result,
            "7b33571d3bba150f4dfd9609c38b4f9acc9a3a8dbfa3121418a35264562ca5d9"
        );
    }

    #[test]
    fn tv4_no_public_identifiers_returns_64_hex_chars() {
        let result =
            boundary_ref_value(&[], &test_salt()).expect("CSPRNG path succeeds on this platform");
        assert_eq!(result.len(), 64, "must be exactly 64 characters");
        assert!(
            result
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f')),
            "must be lowercase hex: {result}"
        );
    }

    #[test]
    fn tv4_random_tokens_differ() {
        let r1 = boundary_ref_value(&[], &test_salt()).expect("CSPRNG succeeds");
        let r2 = boundary_ref_value(&[], &test_salt()).expect("CSPRNG succeeds");
        // 2^-256 collision probability; will never fail in practice.
        assert_ne!(r1, r2, "two random tokens should differ");
    }

    #[test]
    fn deterministic_regardless_of_input_order() {
        // TV1 identifiers provided in reverse order → same hash.
        let ids_forward = vec![
            make_id("duns", "081466849", None),
            make_id("lei", "5493006MHB84DD0ZWV18", None),
        ];
        let ids_reverse = vec![
            make_id("lei", "5493006MHB84DD0ZWV18", None),
            make_id("duns", "081466849", None),
        ];
        let r1 = boundary_ref_value(&ids_forward, &test_salt()).expect("ok");
        let r2 = boundary_ref_value(&ids_reverse, &test_salt()).expect("ok");
        assert_eq!(r1, r2);
    }

    #[test]
    fn decode_salt_round_trip() {
        let fs = FileSalt::try_from(TEST_SALT_HEX).expect("valid FileSalt");
        let bytes = decode_salt(&fs).expect("decode succeeds");
        assert_eq!(bytes[0], 0x00);
        assert_eq!(bytes[1], 0x11);
        assert_eq!(bytes[2], 0x22);
        assert_eq!(bytes[31], 0xff);
    }

    #[test]
    fn hex_encode_all_zeros() {
        assert_eq!(hex_encode(&[0u8; 4]), "00000000");
    }

    #[test]
    fn hex_encode_all_ff() {
        assert_eq!(hex_encode(&[0xffu8; 4]), "ffffffff");
    }

    #[test]
    fn hex_encode_mixed_bytes() {
        assert_eq!(hex_encode(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
    }

    #[test]
    fn hex_decode_salt_rejects_too_short() {
        assert!(hex_decode_salt("abcd").is_err());
    }

    #[test]
    fn hex_decode_salt_rejects_too_long() {
        let long = "a".repeat(66);
        assert!(hex_decode_salt(&long).is_err());
    }

    #[test]
    fn hex_decode_salt_rejects_uppercase() {
        let upper = "A".repeat(64);
        assert!(hex_decode_salt(&upper).is_err());
    }

    #[test]
    fn hex_decode_salt_rejects_non_hex_char() {
        let base = "0".repeat(64);
        let mut chars: Vec<char> = base.chars().collect();
        chars[10] = 'g';
        let s: String = chars.into_iter().collect();
        assert!(hex_decode_salt(&s).is_err());
    }

    #[test]
    fn hex_decode_salt_all_zeros_succeeds() {
        let zeros = "0".repeat(64);
        let result = hex_decode_salt(&zeros).expect("all zeros is valid");
        assert_eq!(result, [0u8; 32]);
    }

    #[test]
    fn error_invalid_salt_display() {
        let e = BoundaryHashError::InvalidSalt("too short".to_owned());
        assert!(e.to_string().contains("invalid file salt"));
        assert!(e.to_string().contains("too short"));
    }
}
