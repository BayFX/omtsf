//! Identifier generators with valid check digits.
//!
//! Produces LEI (MOD 97-10), DUNS (9 digits), GLN (GS1 Mod-10),
//! nat-reg, vat, internal, and opaque identifiers.

use std::collections::BTreeMap;

use omtsf_core::types::Identifier;
use rand::Rng;
use rand::rngs::StdRng;

const ALPHANUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// Generates a valid LEI with correct MOD 97-10 check digits.
///
/// Algorithm: generate 18-char `[A-Z0-9]` body, then compute the two check
/// digits by reversing the MOD 97-10 verification.
pub fn gen_lei(rng: &mut StdRng) -> Identifier {
    let body: String = (0..18)
        .map(|_| {
            let idx = rng.gen_range(0..ALPHANUM.len());
            ALPHANUM[idx] as char
        })
        .collect();

    let with_zeros = format!("{body}00");
    let remainder = mod97_remainder(&with_zeros);
    let check = 98 - remainder;

    Identifier {
        scheme: "lei".to_owned(),
        value: format!("{body}{check:02}"),
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

/// Computes the MOD 97 remainder of an alphanumeric string.
fn mod97_remainder(s: &str) -> u64 {
    let mut remainder: u64 = 0;
    for byte in s.as_bytes() {
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
    remainder
}

/// Generates a valid DUNS number (9 random digits, no check digit).
pub fn gen_duns(rng: &mut StdRng) -> Identifier {
    let value: String = (0..9).map(|_| rng.gen_range(b'0'..=b'9') as char).collect();
    Identifier {
        scheme: "duns".to_owned(),
        value,
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

/// Generates a valid GLN with correct GS1 Mod-10 check digit.
pub fn gen_gln(rng: &mut StdRng) -> Identifier {
    let body: Vec<u8> = (0..12).map(|_| rng.gen_range(0u8..10)).collect();

    let mut sum: u32 = 0;
    for (i, &digit) in body.iter().enumerate() {
        let weight: u32 = if i % 2 == 1 { 3 } else { 1 };
        sum += u32::from(digit) * weight;
    }
    let check = (10 - (sum % 10)) % 10;

    let value: String = body
        .iter()
        .map(|d| (b'0' + d) as char)
        .chain(std::iter::once((b'0' + check as u8) as char))
        .collect();

    Identifier {
        scheme: "gln".to_owned(),
        value,
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

/// Generates a nat-reg identifier with authority.
pub fn gen_nat_reg(rng: &mut StdRng) -> Identifier {
    let value: String = (0..10)
        .map(|_| {
            let idx = rng.gen_range(0..ALPHANUM.len());
            ALPHANUM[idx] as char
        })
        .collect();

    let authorities = [
        "uk-companies-house",
        "de-handelsregister",
        "fr-rcs",
        "nl-kvk",
    ];
    let authority = authorities[rng.gen_range(0..authorities.len())];

    Identifier {
        scheme: "nat-reg".to_owned(),
        value,
        authority: Some(authority.to_owned()),
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

/// Generates a VAT identifier with authority.
pub fn gen_vat(rng: &mut StdRng) -> Identifier {
    let value: String = (0..12)
        .map(|_| {
            let idx = rng.gen_range(0..ALPHANUM.len());
            ALPHANUM[idx] as char
        })
        .collect();

    let authorities = ["eu-vies", "gb-hmrc", "ch-uid"];
    let authority = authorities[rng.gen_range(0..authorities.len())];

    Identifier {
        scheme: "vat".to_owned(),
        value,
        authority: Some(authority.to_owned()),
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

/// Generates an internal identifier with incrementing counter.
pub fn gen_internal(counter: usize) -> Identifier {
    Identifier {
        scheme: "internal".to_owned(),
        value: format!("INT-{counter:06}"),
        authority: Some("erp-system".to_owned()),
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

/// Generates an opaque identifier (SHA-256-like hex string for `boundary_ref` nodes).
pub fn gen_opaque(rng: &mut StdRng) -> Identifier {
    let hex_chars = b"0123456789abcdef";
    let value: String = (0..64)
        .map(|_| {
            let idx = rng.gen_range(0..hex_chars.len());
            hex_chars[idx] as char
        })
        .collect();

    Identifier {
        scheme: "opaque".to_owned(),
        value,
        authority: None,
        valid_from: None,
        valid_to: None,
        sensitivity: None,
        verification_status: None,
        verification_date: None,
        extra: BTreeMap::new(),
    }
}

/// Generates a set of identifiers for a node based on the desired density.
///
/// Always includes an internal identifier and one DUNS identifier (to ensure
/// canonical matching works in diff/merge). Then adds 0-1 additional
/// external identifiers (lei, gln, nat-reg, vat) based on `identifier_density`.
pub fn gen_identifiers(
    rng: &mut StdRng,
    counter: usize,
    identifier_density: f64,
) -> Vec<Identifier> {
    // Always include an internal identifier + one DUNS for canonical matching.
    // The `internal` scheme is excluded from the canonical index, so we need
    // at least one external identifier for diff/merge to work.
    let mut ids = vec![gen_internal(counter), gen_duns(rng)];

    let extra_count = if identifier_density <= 2.0 {
        0
    } else {
        let p = (identifier_density - 2.0).clamp(0.0, 1.0);
        if rng.gen_bool(p) { 1 } else { 0 }
    };

    for _ in 0..extra_count {
        let kind = rng.gen_range(0..4);
        let id = match kind {
            0 => gen_lei(rng),
            1 => gen_gln(rng),
            2 => gen_nat_reg(rng),
            _ => gen_vat(rng),
        };
        ids.push(id);
    }

    ids
}

/// Generates identifiers for a `boundary_ref` node.
pub fn gen_boundary_ref_identifiers(rng: &mut StdRng) -> Vec<Identifier> {
    vec![gen_opaque(rng)]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use omtsf_core::check_digits::{gs1_mod10, mod97_10};
    use rand::SeedableRng;

    #[test]
    fn generated_lei_passes_check_digit() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..100 {
            let id = gen_lei(&mut rng);
            assert!(
                mod97_10(&id.value),
                "LEI {} failed check digit validation",
                id.value
            );
        }
    }

    #[test]
    fn generated_gln_passes_check_digit() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..100 {
            let id = gen_gln(&mut rng);
            assert!(
                gs1_mod10(&id.value),
                "GLN {} failed check digit validation",
                id.value
            );
        }
    }

    #[test]
    fn generated_duns_is_9_digits() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..100 {
            let id = gen_duns(&mut rng);
            assert_eq!(id.value.len(), 9);
            assert!(id.value.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn gen_identifiers_respects_density() {
        let mut rng = StdRng::seed_from_u64(42);
        let ids = gen_identifiers(&mut rng, 0, 1.0);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].scheme, "internal");
        assert_eq!(ids[1].scheme, "duns");
    }
}
