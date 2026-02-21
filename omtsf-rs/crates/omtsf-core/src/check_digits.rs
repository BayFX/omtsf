//! Pure check-digit verification functions for OMTSF identifier schemes.
//!
//! These functions are called by the L1-EID validation rules after a regex
//! pre-check confirms the input has the correct shape.  Both functions are
//! zero-allocation: they operate directly on the byte slice of the input
//! string slice without any heap allocation.
//!
//! # References
//!
//! - MOD 97-10 (ISO 7064) for LEI — validation.md Section 5.1
//! - GS1 Mod-10 for GLN — validation.md Section 5.2

/// Verifies the ISO 7064 MOD 97-10 check digit for a Legal Entity Identifier.
///
/// **Pre-condition:** The caller MUST have already confirmed that `lei` matches
/// `^[A-Z0-9]{18}[0-9]{2}$`.  If called on a string that does not match,
/// the returned boolean is meaningless (though the function is still safe).
///
/// # Algorithm
///
/// Each character is converted to its numeric value:
/// - Digits `0`–`9` map to 0–9.
/// - Letters `A`–`Z` map to 10–35.
///
/// The resulting numeric string is reduced modulo 97 incrementally: for each
/// character, `remainder = (remainder * base + digit_value) % 97`, where
/// `base` is 10 for a one-digit expansion (digits 0–9) and 100 for a
/// two-digit expansion (letters A–Z, which expand to 10–35).
///
/// A valid LEI produces a final remainder of 1.
///
/// # Examples
///
/// ```
/// use omtsf_core::check_digits::mod97_10;
///
/// // A known-valid LEI from GLEIF.
/// assert!(mod97_10("5493006MHB84DD0ZWV18"));
///
/// // Corrupting the last digit invalidates the check.
/// assert!(!mod97_10("5493006MHB84DD0ZWV19"));
/// ```
pub fn mod97_10(lei: &str) -> bool {
    let mut remainder: u64 = 0;
    for byte in lei.as_bytes() {
        match byte {
            b'0'..=b'9' => {
                let digit = u64::from(byte - b'0');
                remainder = (remainder * 10 + digit) % 97;
            }
            b'A'..=b'Z' => {
                let value = u64::from(byte - b'A') + 10;
                remainder = (remainder * 100 + value) % 97;
            }
            _ => {}
        }
    }
    remainder == 1
}

/// Verifies the GS1 Mod-10 check digit for a Global Location Number.
///
/// **Pre-condition:** The caller MUST have already confirmed that `gln` matches
/// `^[0-9]{13}$`.  If called on a string that does not match, the returned
/// boolean is meaningless (though the function is still safe).
///
/// # Algorithm
///
/// Positions are numbered 1–13 from left to right.  The check digit is at
/// position 13.  Weights alternate starting from the rightmost non-check
/// position (position 12): positions 12, 10, 8, … have weight 1; positions
/// 11, 9, 7, … have weight 3.  Equivalently, position `i` (1-indexed) has
/// weight 3 if `(13 - i)` is odd, else weight 1.
///
/// The check digit `d` at position 13 satisfies:
/// `(sum_of_weighted_products + d) mod 10 == 0`
///
/// Which is equivalent to:
/// `d == (10 - (sum mod 10)) mod 10`
///
/// # Examples
///
/// ```
/// use omtsf_core::check_digits::gs1_mod10;
///
/// // A known-valid GLN (GS1 example).
/// assert!(gs1_mod10("0614141000418"));
///
/// // Corrupting the check digit invalidates it.
/// assert!(!gs1_mod10("0614141000419"));
/// ```
pub fn gs1_mod10(gln: &str) -> bool {
    let bytes = gln.as_bytes();
    if bytes.len() != 13 {
        return false;
    }

    let mut sum: u32 = 0;
    for (i, byte) in bytes[..12].iter().enumerate() {
        let digit = u32::from(byte - b'0');
        let weight: u32 = if i % 2 == 1 { 3 } else { 1 };
        sum += digit * weight;
    }

    let expected_check = (10 - (sum % 10)) % 10;
    let actual_check = u32::from(bytes[12] - b'0');
    expected_check == actual_check
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// Known-valid LEI from the GLEIF public database.
    /// `5493006MHB84DD0ZWV18` is the LEI for the Bank for International Settlements.
    #[test]
    fn mod97_10_valid_bis_lei() {
        assert!(mod97_10("5493006MHB84DD0ZWV18"));
    }

    /// A second known-valid LEI (Deutsche Bank AG).
    #[test]
    fn mod97_10_valid_deutsche_bank_lei() {
        assert!(mod97_10("7LTWFZYICNSX8D621K86"));
    }

    /// A third known-valid LEI (Apple Inc.).
    #[test]
    fn mod97_10_valid_apple_lei() {
        assert!(mod97_10("HWUPKR0MPOU8FGXBT394"));
    }

    /// Corrupting the last check digit must invalidate the LEI.
    #[test]
    fn mod97_10_invalid_corrupt_check_digit() {
        assert!(!mod97_10("5493006MHB84DD0ZWV19"));
    }

    /// Corrupting a middle character must invalidate the LEI.
    #[test]
    fn mod97_10_invalid_corrupt_body() {
        assert!(!mod97_10("5493007MHB84DD0ZWV18"));
    }

    /// A string of 20 zeros fails (remainder 0, not 1).
    #[test]
    fn mod97_10_invalid_all_zeros() {
        assert!(!mod97_10("00000000000000000000"));
    }

    /// Flipping two adjacent characters should almost always invalidate the LEI.
    #[test]
    fn mod97_10_invalid_transposition() {
        assert!(!mod97_10("5493060MHB84DD0ZWV18"));
    }

    /// Known-valid GLN from the GS1 specification example.
    /// `0614141000418` is a standard GS1 example GLN.
    #[test]
    fn gs1_mod10_valid_gs1_example() {
        assert!(gs1_mod10("0614141000418"));
    }

    /// A second known-valid GLN.
    /// `5901234123457` is a commonly cited GS1 test vector.
    #[test]
    fn gs1_mod10_valid_second_example() {
        assert!(gs1_mod10("5901234123457"));
    }

    /// A third known-valid GLN (all-zero prefix with valid check digit 0).
    #[test]
    fn gs1_mod10_valid_third_example() {
        assert!(gs1_mod10("4000000000006"));
    }

    /// Corrupting the last digit (check digit) invalidates the GLN.
    #[test]
    fn gs1_mod10_invalid_corrupt_check_digit() {
        assert!(!gs1_mod10("0614141000419"));
    }

    /// Corrupting a body digit invalidates the GLN.
    #[test]
    fn gs1_mod10_invalid_corrupt_body() {
        assert!(!gs1_mod10("0614141000428"));
    }

    /// Wrong length (12 digits) is rejected.
    #[test]
    fn gs1_mod10_invalid_too_short() {
        assert!(!gs1_mod10("061414100041"));
    }

    /// Wrong length (14 digits) is rejected.
    #[test]
    fn gs1_mod10_invalid_too_long() {
        assert!(!gs1_mod10("06141410004180"));
    }
}
