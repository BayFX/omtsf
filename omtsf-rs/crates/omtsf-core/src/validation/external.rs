//! External data source trait and associated record types for L3 validation rules.
//!
//! L3 rules cross-reference external data sources such as GLEIF (for LEI records)
//! and national business registries (for `nat-reg` identifiers). This module
//! defines the [`ExternalDataSource`] trait that L3 rules receive as an injected
//! dependency, keeping `omtsf-core` free of network or I/O dependencies.
//!
//! The CLI wires in a concrete implementation. WASM consumers may supply their own.
//!
//! See `omtsf-rs/docs/validation.md` Section 4.3 for the full specification.

/// A record returned by an LEI data source for a given LEI string.
///
/// Fields represent the subset of GLEIF LEVEL 1 data needed by L3 validation rules.
/// The record is consumed by L3-EID-01 (LEI status verification) and related rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeiRecord {
    /// The LEI string this record corresponds to.
    pub lei: String,
    /// The registration status reported by the LEI issuer
    /// (e.g. `"ISSUED"`, `"LAPSED"`, `"ANNULLED"`).
    pub registration_status: String,
    /// Whether the LEI is currently active (`"ISSUED"` and not retired).
    pub is_active: bool,
}

/// A record returned by a national business registry lookup.
///
/// Fields represent the minimum information needed by L3 validation rules
/// to verify that a `nat-reg` identifier resolves to a known legal entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatRegRecord {
    /// The registration authority code (GLEIF RA code or equivalent).
    pub authority: String,
    /// The registration number as filed with the authority.
    pub value: String,
    /// Whether the registration is currently active in the registry.
    pub is_active: bool,
}

/// An injected data source for L3 validation rules.
///
/// Implementations may call external APIs, query local caches, or return
/// fixed data (e.g. for testing). The trait is object-safe: the engine
/// stores the reference as `Option<&dyn ExternalDataSource>`.
///
/// # WASM safety
///
/// `omtsf-core` does not perform I/O. All network access is the responsibility
/// of the caller supplying the concrete implementation.
///
/// # Skipping gracefully
///
/// L3 rules receive `Option<&dyn ExternalDataSource>`. When the option is `None`,
/// each rule MUST skip its checks entirely without emitting any diagnostics.
pub trait ExternalDataSource {
    /// Look up LEI registration status for the given LEI string.
    ///
    /// Returns `None` if the LEI is not found in the data source or if the
    /// data source is unavailable for this particular lookup. Rules MUST treat
    /// `None` as "no data available" and skip the check silently.
    fn lei_status(&self, lei: &str) -> Option<LeiRecord>;

    /// Look up a national registry entry by authority and registration value.
    ///
    /// `authority` is the GLEIF registration authority code (e.g. `"RA000548"`
    /// for the German Handelsregister) or an equivalent identifier understood
    /// by the implementing data source.
    ///
    /// Returns `None` if the entry is not found or the data source is
    /// unavailable for this lookup. Rules MUST treat `None` as "no data
    /// available" and skip the check silently.
    fn nat_reg_lookup(&self, authority: &str, value: &str) -> Option<NatRegRecord>;
}
