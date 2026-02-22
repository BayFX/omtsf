/// `CanonicalId` newtype and identifier index construction for the merge engine.
///
/// This module implements SPEC-002 Section 4 canonical identifier strings and
/// the identifier index described in merge.md Section 2.2.
///
/// The canonical form is:
/// - `{scheme}:{value}` for schemes that do not require authority
/// - `{scheme}:{authority}:{value}` for authority-required schemes (`nat-reg`, `vat`)
///
/// All three components are percent-encoded before joining: colons become `%3A`,
/// percent signs become `%25`, newlines become `%0A`, carriage returns become `%0D`.
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

use crate::structures::Node;
use crate::types::Identifier;

/// Returns `true` if `scheme` requires an authority component in its canonical
/// form.
///
/// Per SPEC-002, `nat-reg` and `vat` require authority. All other schemes may
/// optionally carry authority but do not require it in the canonical form.
fn requires_authority(scheme: &str) -> bool {
    scheme == "nat-reg" || scheme == "vat"
}

/// Percent-encodes a single component string per SPEC-002 Section 4.
///
/// The following characters are encoded:
/// - `%` → `%25`
/// - `:` → `%3A`
/// - `\n` (0x0A) → `%0A`
/// - `\r` (0x0D) → `%0D`
///
/// All other bytes are preserved as-is.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '%' => out.push_str("%25"),
            ':' => out.push_str("%3A"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            other => out.push(other),
        }
    }
    out
}

/// A canonical identifier string used as the key in the identifier index.
///
/// Constructed from an [`Identifier`] via [`CanonicalId::from_identifier`].
/// The inner string has the form `{scheme}:{value}` or
/// `{scheme}:{authority}:{value}` for authority-required schemes, with colons,
/// percent signs, newlines, and carriage returns in each component
/// percent-encoded before joining.
///
/// Implements `Deref<Target = str>` for zero-copy access to the inner string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalId(String);

impl CanonicalId {
    /// Constructs a `CanonicalId` from an [`Identifier`].
    ///
    /// The canonical form encodes each component (scheme, authority if
    /// applicable, value) with `percent_encode`, then joins them with `:`.
    ///
    /// Authority-required schemes (`nat-reg`, `vat`) include the authority as
    /// the middle segment. For other schemes the authority component is omitted
    /// from the canonical string even if the identifier carries one.
    ///
    /// # Example
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use omtsf_core::types::Identifier;
    /// use omtsf_core::canonical::CanonicalId;
    ///
    /// let id = Identifier {
    ///     scheme: "lei".to_owned(),
    ///     value: "529900T8BM49AURSDO55".to_owned(),
    ///     authority: None,
    ///     valid_from: None,
    ///     valid_to: None,
    ///     sensitivity: None,
    ///     verification_status: None,
    ///     verification_date: None,
    ///     extra: BTreeMap::new(),
    /// };
    /// let cid = CanonicalId::from_identifier(&id);
    /// assert_eq!(cid.as_str(), "lei:529900T8BM49AURSDO55");
    /// ```
    pub fn from_identifier(id: &Identifier) -> Self {
        let enc_scheme = percent_encode(&id.scheme);
        let enc_value = percent_encode(&id.value);

        let inner = if requires_authority(&id.scheme) {
            let authority = id.authority.as_deref().unwrap_or("");
            let enc_authority = percent_encode(authority);
            format!("{enc_scheme}:{enc_authority}:{enc_value}")
        } else {
            format!("{enc_scheme}:{enc_value}")
        };

        Self(inner)
    }

    /// Returns the canonical string as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes this `CanonicalId` and returns the inner `String`.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl Deref for CanonicalId {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CanonicalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for CanonicalId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Builds a `HashMap` from canonical identifier to the list of node indices
/// that carry that identifier.
///
/// Each element of `nodes` that has an `identifiers` field is iterated. For
/// every identifier whose scheme is **not** `internal`, the canonical form is
/// computed and the node index is appended to the corresponding entry in the
/// map.
///
/// `internal` scheme identifiers are excluded entirely per merge.md Section 2.2.
///
/// Construction is O(total identifiers) with a single pass over all nodes.
///
/// # Parameters
///
/// - `nodes`: slice of [`Node`] values, typically the concatenated set of
///   nodes from all input files. The returned indices are positions in this
///   slice.
///
/// # Returns
///
/// A `HashMap<CanonicalId, Vec<usize>>` where each key maps to one or more
/// node indices sharing that canonical identifier.
pub fn build_identifier_index(nodes: &[Node]) -> HashMap<CanonicalId, Vec<usize>> {
    let mut index: HashMap<CanonicalId, Vec<usize>> = HashMap::new();

    for (node_idx, node) in nodes.iter().enumerate() {
        let Some(identifiers) = node.identifiers.as_ref() else {
            continue;
        };
        for id in identifiers {
            if id.scheme == "internal" {
                continue;
            }
            let canonical = CanonicalId::from_identifier(id);
            index.entry(canonical).or_default().push(node_idx);
        }
    }

    index
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeMap;

    use super::*;
    use crate::enums::{NodeType, NodeTypeTag};
    use crate::newtypes::NodeId;
    use crate::types::Identifier;

    fn make_identifier(scheme: &str, value: &str, authority: Option<&str>) -> Identifier {
        Identifier {
            scheme: scheme.to_owned(),
            value: value.to_owned(),
            authority: authority.map(str::to_owned),
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: BTreeMap::new(),
        }
    }

    fn make_node(id: &str, identifiers: Option<Vec<Identifier>>) -> Node {
        Node {
            id: NodeId::try_from(id).expect("valid NodeId"),
            node_type: NodeTypeTag::Known(NodeType::Organization),
            identifiers,
            data_quality: None,
            labels: None,
            name: None,
            jurisdiction: None,
            status: None,
            governance_structure: None,
            operator: None,
            address: None,
            geo: None,
            commodity_code: None,
            unit: None,
            role: None,
            attestation_type: None,
            standard: None,
            issuer: None,
            valid_from: None,
            valid_to: None,
            outcome: None,
            attestation_status: None,
            reference: None,
            risk_severity: None,
            risk_likelihood: None,
            lot_id: None,
            quantity: None,
            production_date: None,
            origin_country: None,
            direct_emissions_co2e: None,
            indirect_emissions_co2e: None,
            emission_factor_source: None,
            installation_id: None,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn canonical_id_basic_lei() {
        let id = make_identifier("lei", "529900T8BM49AURSDO55", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "lei:529900T8BM49AURSDO55");
    }

    #[test]
    fn canonical_id_duns() {
        let id = make_identifier("duns", "081466849", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "duns:081466849");
    }

    #[test]
    fn canonical_id_gln() {
        let id = make_identifier("gln", "1234567890123", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "gln:1234567890123");
    }

    #[test]
    fn canonical_id_nat_reg_with_authority() {
        let id = make_identifier("nat-reg", "HRB12345", Some("DE"));
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "nat-reg:DE:HRB12345");
    }

    #[test]
    fn canonical_id_vat_with_authority() {
        let id = make_identifier("vat", "DE123456789", Some("DE"));
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "vat:DE:DE123456789");
    }

    #[test]
    fn canonical_id_nat_reg_missing_authority_uses_empty() {
        let id = make_identifier("nat-reg", "HRB12345", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "nat-reg::HRB12345");
    }

    #[test]
    fn canonical_id_non_authority_scheme_ignores_authority() {
        let id = make_identifier("lei", "529900T8BM49AURSDO55", Some("GLEIF"));
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "lei:529900T8BM49AURSDO55");
    }

    #[test]
    fn canonical_id_percent_encodes_colon_in_value() {
        let id = make_identifier("internal", "foo:bar", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "internal:foo%3Abar");
    }

    #[test]
    fn canonical_id_percent_encodes_percent_in_value() {
        let id = make_identifier("lei", "50%", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "lei:50%25");
    }

    #[test]
    fn canonical_id_percent_encodes_percent_then_percent() {
        let id = make_identifier("lei", "50%25", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "lei:50%2525");
    }

    #[test]
    fn canonical_id_percent_encodes_newline_in_value() {
        let id = make_identifier("lei", "abc\ndef", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "lei:abc%0Adef");
    }

    #[test]
    fn canonical_id_percent_encodes_carriage_return_in_value() {
        let id = make_identifier("lei", "abc\rdef", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "lei:abc%0Ddef");
    }

    #[test]
    fn canonical_id_percent_encodes_colon_in_scheme() {
        let id = make_identifier("com.example:ext", "value", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "com.example%3Aext:value");
    }

    #[test]
    fn canonical_id_percent_encodes_colon_in_authority() {
        let id = make_identifier("nat-reg", "HRB12345", Some("D:E"));
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "nat-reg:D%3AE:HRB12345");
    }

    #[test]
    fn canonical_id_percent_encodes_crlf_in_value() {
        let id = make_identifier("lei", "foo\r\nbar", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.as_str(), "lei:foo%0D%0Abar");
    }

    #[test]
    fn canonical_id_display() {
        let id = make_identifier("duns", "123456789", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.to_string(), "duns:123456789");
    }

    #[test]
    fn canonical_id_deref() {
        let id = make_identifier("lei", "TEST", None);
        let cid = CanonicalId::from_identifier(&id);
        assert!(cid.starts_with("lei:"));
        assert_eq!(cid.len(), "lei:TEST".len());
    }

    #[test]
    fn canonical_id_as_ref_str() {
        let id = make_identifier("lei", "X", None);
        let cid = CanonicalId::from_identifier(&id);
        let s: &str = cid.as_ref();
        assert_eq!(s, "lei:X");
    }

    #[test]
    fn canonical_id_into_string() {
        let id = make_identifier("duns", "999", None);
        let cid = CanonicalId::from_identifier(&id);
        assert_eq!(cid.into_string(), "duns:999");
    }

    #[test]
    fn canonical_id_clone_eq() {
        let id = make_identifier("lei", "ABC", None);
        let cid = CanonicalId::from_identifier(&id);
        let cloned = cid.clone();
        assert_eq!(cid, cloned);
    }

    #[test]
    fn canonical_id_hash_in_hashmap() {
        let id = make_identifier("lei", "UNIQUE", None);
        let cid = CanonicalId::from_identifier(&id);
        let mut map: HashMap<CanonicalId, u32> = HashMap::new();
        map.insert(cid.clone(), 42);
        assert_eq!(map.get(&cid), Some(&42));
    }

    #[test]
    fn index_empty_nodes() {
        let nodes: Vec<Node> = vec![];
        let index = build_identifier_index(&nodes);
        assert!(index.is_empty());
    }

    #[test]
    fn index_node_without_identifiers() {
        let nodes = vec![make_node("org-1", None)];
        let index = build_identifier_index(&nodes);
        assert!(index.is_empty());
    }

    #[test]
    fn index_node_with_empty_identifiers() {
        let nodes = vec![make_node("org-1", Some(vec![]))];
        let index = build_identifier_index(&nodes);
        assert!(index.is_empty());
    }

    #[test]
    fn index_internal_scheme_excluded() {
        let id = make_identifier("internal", "sap-prod:V-100234", None);
        let nodes = vec![make_node("org-1", Some(vec![id]))];
        let index = build_identifier_index(&nodes);
        assert!(index.is_empty());
    }

    #[test]
    fn index_single_node_single_identifier() {
        let id = make_identifier("lei", "529900T8BM49AURSDO55", None);
        let nodes = vec![make_node("org-1", Some(vec![id]))];
        let index = build_identifier_index(&nodes);
        assert_eq!(index.len(), 1);

        let key = CanonicalId(String::from("lei:529900T8BM49AURSDO55"));
        let indices = index.get(&key).expect("key should be present");
        assert_eq!(indices, &[0usize]);
    }

    #[test]
    fn index_single_node_multiple_identifiers() {
        let ids = vec![
            make_identifier("lei", "529900T8BM49AURSDO55", None),
            make_identifier("duns", "081466849", None),
        ];
        let nodes = vec![make_node("org-1", Some(ids))];
        let index = build_identifier_index(&nodes);
        assert_eq!(index.len(), 2);

        let lei_key = CanonicalId(String::from("lei:529900T8BM49AURSDO55"));
        let duns_key = CanonicalId(String::from("duns:081466849"));
        assert_eq!(index.get(&lei_key).expect("lei present"), &[0usize]);
        assert_eq!(index.get(&duns_key).expect("duns present"), &[0usize]);
    }

    #[test]
    fn index_two_nodes_no_overlap() {
        let nodes = vec![
            make_node("org-1", Some(vec![make_identifier("lei", "LEI_A", None)])),
            make_node("org-2", Some(vec![make_identifier("lei", "LEI_B", None)])),
        ];
        let index = build_identifier_index(&nodes);
        assert_eq!(index.len(), 2);

        let key_a = CanonicalId(String::from("lei:LEI_A"));
        let key_b = CanonicalId(String::from("lei:LEI_B"));
        assert_eq!(index.get(&key_a).expect("LEI_A"), &[0usize]);
        assert_eq!(index.get(&key_b).expect("LEI_B"), &[1usize]);
    }

    #[test]
    fn index_overlapping_identifier_maps_to_both_nodes() {
        let shared_lei = "529900T8BM49AURSDO55";
        let nodes = vec![
            make_node(
                "org-1",
                Some(vec![make_identifier("lei", shared_lei, None)]),
            ),
            make_node(
                "org-2",
                Some(vec![make_identifier("lei", shared_lei, None)]),
            ),
        ];
        let index = build_identifier_index(&nodes);
        assert_eq!(index.len(), 1);

        let key = CanonicalId(format!("lei:{shared_lei}"));
        let mut indices = index.get(&key).expect("shared key present").clone();
        indices.sort_unstable();
        assert_eq!(indices, vec![0usize, 1]);
    }

    #[test]
    fn index_three_nodes_transitive_overlap() {
        let nodes = vec![
            make_node("org-1", Some(vec![make_identifier("lei", "LEI_AB", None)])),
            make_node(
                "org-2",
                Some(vec![
                    make_identifier("lei", "LEI_AB", None),
                    make_identifier("duns", "DUNS_BC", None),
                ]),
            ),
            make_node(
                "org-3",
                Some(vec![make_identifier("duns", "DUNS_BC", None)]),
            ),
        ];
        let index = build_identifier_index(&nodes);
        assert_eq!(index.len(), 2);

        let lei_key = CanonicalId(String::from("lei:LEI_AB"));
        let duns_key = CanonicalId(String::from("duns:DUNS_BC"));

        let mut lei_indices = index.get(&lei_key).expect("lei key").clone();
        lei_indices.sort_unstable();
        assert_eq!(lei_indices, vec![0usize, 1]);

        let mut duns_indices = index.get(&duns_key).expect("duns key").clone();
        duns_indices.sort_unstable();
        assert_eq!(duns_indices, vec![1usize, 2]);
    }

    #[test]
    fn index_mixed_internal_and_external_identifiers() {
        let ids = vec![
            make_identifier("internal", "sap:1234", None),
            make_identifier("lei", "PUBLIC_LEI", None),
        ];
        let nodes = vec![make_node("org-1", Some(ids))];
        let index = build_identifier_index(&nodes);
        assert_eq!(index.len(), 1, "only the lei should be indexed");

        let lei_key = CanonicalId(String::from("lei:PUBLIC_LEI"));
        assert!(index.contains_key(&lei_key));
    }

    #[test]
    fn index_nat_reg_with_authority() {
        let id = make_identifier("nat-reg", "HRB12345", Some("DE"));
        let nodes = vec![make_node("org-1", Some(vec![id]))];
        let index = build_identifier_index(&nodes);
        let key = CanonicalId(String::from("nat-reg:DE:HRB12345"));
        assert!(index.contains_key(&key));
    }

    #[test]
    fn index_value_with_colon_percent_encoded() {
        let id_with_colon = make_identifier("duns", "12:34", None);
        let id_plain = make_identifier("duns", "1234", None);
        let nodes = vec![
            make_node("org-1", Some(vec![id_with_colon])),
            make_node("org-2", Some(vec![id_plain])),
        ];
        let index = build_identifier_index(&nodes);
        assert_eq!(index.len(), 2, "colon-encoded and plain are distinct keys");

        let encoded_key = CanonicalId(String::from("duns:12%3A34"));
        let plain_key = CanonicalId(String::from("duns:1234"));
        assert_eq!(index.get(&encoded_key).expect("colon key"), &[0usize]);
        assert_eq!(index.get(&plain_key).expect("plain key"), &[1usize]);
    }
}
