/// Selector types and `SelectorSet` for property-based node and edge filtering.
///
/// Implements Section 2 of the query technical specification (`query.md`).
///
/// # Composition Rules
///
/// Selectors compose with **OR within a group** and **AND across groups**:
/// - Multiple `NodeType` selectors → node must match *any* of them (OR).
/// - A `NodeType` group combined with a `Jurisdiction` group → node must match
///   both groups (AND).
///
/// This is the standard "disjunctive normal form within conjunctive groups"
/// pattern used by tools such as `kubectl` label selectors.
///
/// # Node-only vs Edge-only Selectors
///
/// - Node-only selectors (`NodeType`, `IdentifierScheme`, `IdentifierSchemeValue`,
///   `Jurisdiction`, `Name`) are silently skipped when evaluating edges.
/// - Edge-only selectors (`EdgeType`) are silently skipped when evaluating nodes.
/// - `LabelKey` and `LabelKeyValue` apply to both nodes and edges.
use crate::enums::{EdgeTypeTag, NodeTypeTag};
use crate::newtypes::CountryCode;
use crate::structures::{Edge, Node};

/// A single predicate that matches nodes, edges, or both.
///
/// See `query.md` Section 2.1 for the full semantics of each variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// Match nodes whose `node_type` equals the given tag.
    NodeType(NodeTypeTag),
    /// Match edges whose `edge_type` equals the given tag.
    EdgeType(EdgeTypeTag),
    /// Match elements that have a label with the given key (any value or no value).
    LabelKey(String),
    /// Match elements that have a label with the given key and exact value.
    LabelKeyValue(String, String),
    /// Match nodes that have at least one identifier with the given scheme.
    IdentifierScheme(String),
    /// Match nodes that have an identifier with the given scheme and exact value.
    IdentifierSchemeValue(String, String),
    /// Match nodes whose `jurisdiction` equals the given ISO 3166-1 alpha-2 code.
    Jurisdiction(CountryCode),
    /// Case-insensitive substring match on the node `name` field.
    Name(String),
}

/// Grouped selectors with AND-across-groups / OR-within-group composition.
///
/// Each field holds the accumulated values for one selector group. An empty
/// `Vec` for a group means that group is unconstrained (skipped during
/// evaluation). A non-empty `Vec` means the element must satisfy at least
/// one entry in that group.
///
/// Construct with [`SelectorSet::from_selectors`] or by populating the public
/// fields directly and then calling [`SelectorSet::matches_node`] /
/// [`SelectorSet::matches_edge`].
#[derive(Debug, Clone, Default)]
pub struct SelectorSet {
    /// Node type filter values (OR within group, node-only).
    pub node_types: Vec<NodeTypeTag>,
    /// Edge type filter values (OR within group, edge-only).
    pub edge_types: Vec<EdgeTypeTag>,
    /// Label key filter values (OR within group, applies to both nodes and edges).
    pub label_keys: Vec<String>,
    /// Label key=value filter pairs (OR within group, applies to both nodes and edges).
    pub label_key_values: Vec<(String, String)>,
    /// Identifier scheme filter values (OR within group, node-only).
    pub identifier_schemes: Vec<String>,
    /// Identifier scheme:value filter pairs (OR within group, node-only).
    pub identifier_scheme_values: Vec<(String, String)>,
    /// Jurisdiction filter values (OR within group, node-only).
    pub jurisdictions: Vec<CountryCode>,
    /// Name substring filter values (OR within group, case-insensitive, node-only).
    pub names: Vec<String>,
}

impl SelectorSet {
    /// Returns `true` if no selectors have been added.
    ///
    /// An empty `SelectorSet` passed to [`matches_node`][Self::matches_node] or
    /// [`matches_edge`][Self::matches_edge] returns `true` for every element
    /// (no constraints → everything matches).
    pub fn is_empty(&self) -> bool {
        self.node_types.is_empty()
            && self.edge_types.is_empty()
            && self.label_keys.is_empty()
            && self.label_key_values.is_empty()
            && self.identifier_schemes.is_empty()
            && self.identifier_scheme_values.is_empty()
            && self.jurisdictions.is_empty()
            && self.names.is_empty()
    }

    /// Returns `true` if at least one node-applicable selector is set.
    ///
    /// Node-applicable selectors are: `node_types`, `label_keys`, `label_key_values`,
    /// `identifier_schemes`, `identifier_scheme_values`, `jurisdictions`, `names`.
    ///
    /// Used by [`crate::graph::selector_match`] to decide whether to evaluate
    /// nodes at all. When `false`, the caller should treat nodes as not matched by
    /// this `SelectorSet`.
    pub fn has_node_selectors(&self) -> bool {
        !self.node_types.is_empty()
            || !self.label_keys.is_empty()
            || !self.label_key_values.is_empty()
            || !self.identifier_schemes.is_empty()
            || !self.identifier_scheme_values.is_empty()
            || !self.jurisdictions.is_empty()
            || !self.names.is_empty()
    }

    /// Returns `true` if at least one edge-applicable selector is set.
    ///
    /// Edge-applicable selectors are: `edge_types`, `label_keys`, `label_key_values`.
    ///
    /// Used by [`crate::graph::selector_match`] to decide whether to evaluate
    /// edges at all. When `false`, the caller should treat edges as not matched by
    /// this `SelectorSet`.
    pub fn has_edge_selectors(&self) -> bool {
        !self.edge_types.is_empty()
            || !self.label_keys.is_empty()
            || !self.label_key_values.is_empty()
    }

    /// Builds a `SelectorSet` from a flat list of [`Selector`] values.
    ///
    /// Each selector is distributed into the appropriate group field.
    pub fn from_selectors(selectors: Vec<Selector>) -> Self {
        let mut set = Self::default();
        for selector in selectors {
            match selector {
                Selector::NodeType(t) => set.node_types.push(t),
                Selector::EdgeType(t) => set.edge_types.push(t),
                Selector::LabelKey(k) => set.label_keys.push(k),
                Selector::LabelKeyValue(k, v) => set.label_key_values.push((k, v)),
                Selector::IdentifierScheme(s) => set.identifier_schemes.push(s),
                Selector::IdentifierSchemeValue(s, v) => {
                    set.identifier_scheme_values.push((s, v));
                }
                Selector::Jurisdiction(c) => set.jurisdictions.push(c),
                Selector::Name(n) => set.names.push(n),
            }
        }
        set
    }

    /// Returns `true` if `node` matches all non-empty selector groups.
    ///
    /// Evaluation follows AND-across-groups / OR-within-group semantics.
    /// Edge-only selectors (`edge_types`) are ignored.
    ///
    /// # Group evaluation order
    ///
    /// 1. `node_types` — node's type must match at least one entry.
    /// 2. `label_keys` — node must have a label whose key matches any entry.
    /// 3. `label_key_values` — node must have a label matching any (key, value) pair.
    /// 4. `identifier_schemes` — node must have an identifier whose scheme matches any entry.
    /// 5. `identifier_scheme_values` — node must have an identifier matching any (scheme, value) pair.
    /// 6. `jurisdictions` — node's jurisdiction must match at least one entry.
    /// 7. `names` — node's name must contain at least one entry as a case-insensitive substring.
    pub fn matches_node(&self, node: &Node) -> bool {
        if !self.node_types.is_empty() && !self.node_types.contains(&node.node_type) {
            return false;
        }

        if !self.label_keys.is_empty() {
            let has_match = node.labels.as_ref().is_some_and(|labels| {
                labels
                    .iter()
                    .any(|lbl| self.label_keys.iter().any(|k| k == &lbl.key))
            });
            if !has_match {
                return false;
            }
        }

        if !self.label_key_values.is_empty() {
            let has_match = node.labels.as_ref().is_some_and(|labels| {
                labels.iter().any(|lbl| {
                    self.label_key_values
                        .iter()
                        .any(|(k, v)| k == &lbl.key && lbl.value.as_deref() == Some(v.as_str()))
                })
            });
            if !has_match {
                return false;
            }
        }

        if !self.identifier_schemes.is_empty() {
            let has_match = node.identifiers.as_ref().is_some_and(|ids| {
                ids.iter()
                    .any(|id| self.identifier_schemes.iter().any(|s| s == &id.scheme))
            });
            if !has_match {
                return false;
            }
        }

        if !self.identifier_scheme_values.is_empty() {
            let has_match = node.identifiers.as_ref().is_some_and(|ids| {
                ids.iter().any(|id| {
                    self.identifier_scheme_values
                        .iter()
                        .any(|(s, v)| s == &id.scheme && v == &id.value)
                })
            });
            if !has_match {
                return false;
            }
        }

        if !self.jurisdictions.is_empty() {
            let has_match = node
                .jurisdiction
                .as_ref()
                .is_some_and(|jur| self.jurisdictions.iter().any(|j| j == jur));
            if !has_match {
                return false;
            }
        }

        if !self.names.is_empty() {
            let has_match = node.name.as_ref().is_some_and(|name| {
                let name_lower = name.to_lowercase();
                self.names
                    .iter()
                    .any(|pat| name_lower.contains(pat.to_lowercase().as_str()))
            });
            if !has_match {
                return false;
            }
        }

        true
    }

    /// Returns `true` if `edge` matches all non-empty edge selector groups.
    ///
    /// Node-only selectors (`node_types`, `identifier_schemes`,
    /// `identifier_scheme_values`, `jurisdictions`, `names`) are ignored.
    ///
    /// # Group evaluation order
    ///
    /// 1. `edge_types` — edge's type must match at least one entry.
    /// 2. `label_keys` — edge must have a label whose key matches any entry.
    /// 3. `label_key_values` — edge must have a label matching any (key, value) pair.
    pub fn matches_edge(&self, edge: &Edge) -> bool {
        if !self.edge_types.is_empty() && !self.edge_types.contains(&edge.edge_type) {
            return false;
        }

        if !self.label_keys.is_empty() {
            let has_match = edge.properties.labels.as_ref().is_some_and(|labels| {
                labels
                    .iter()
                    .any(|lbl| self.label_keys.iter().any(|k| k == &lbl.key))
            });
            if !has_match {
                return false;
            }
        }

        if !self.label_key_values.is_empty() {
            let has_match = edge.properties.labels.as_ref().is_some_and(|labels| {
                labels.iter().any(|lbl| {
                    self.label_key_values
                        .iter()
                        .any(|(k, v)| k == &lbl.key && lbl.value.as_deref() == Some(v.as_str()))
                })
            });
            if !has_match {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::enums::{EdgeType, NodeType};
    use crate::newtypes::{EdgeId, NodeId};
    use crate::structures::{Edge, EdgeProperties, Node};
    use crate::types::{Identifier, Label};

    fn node_id(s: &str) -> NodeId {
        NodeId::try_from(s).expect("valid NodeId")
    }

    fn edge_id(s: &str) -> EdgeId {
        NodeId::try_from(s).expect("valid EdgeId")
    }

    fn country_code(s: &str) -> CountryCode {
        CountryCode::try_from(s).expect("valid CountryCode")
    }

    fn bare_node(id: &str, node_type: NodeTypeTag) -> Node {
        Node {
            id: node_id(id),
            node_type,
            identifiers: None,
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
            extra: serde_json::Map::new(),
        }
    }

    fn org_node(id: &str) -> Node {
        bare_node(id, NodeTypeTag::Known(NodeType::Organization))
    }

    fn facility_node(id: &str) -> Node {
        bare_node(id, NodeTypeTag::Known(NodeType::Facility))
    }

    fn supplies_edge(id: &str, source: &str, target: &str) -> Edge {
        Edge {
            id: edge_id(id),
            edge_type: EdgeTypeTag::Known(EdgeType::Supplies),
            source: node_id(source),
            target: node_id(target),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    fn ownership_edge(id: &str, source: &str, target: &str) -> Edge {
        Edge {
            id: edge_id(id),
            edge_type: EdgeTypeTag::Known(EdgeType::Ownership),
            source: node_id(source),
            target: node_id(target),
            identifiers: None,
            properties: EdgeProperties::default(),
            extra: serde_json::Map::new(),
        }
    }

    fn label(key: &str, value: Option<&str>) -> Label {
        Label {
            key: key.to_owned(),
            value: value.map(str::to_owned),
            extra: serde_json::Map::new(),
        }
    }

    fn identifier(scheme: &str, value: &str) -> Identifier {
        Identifier {
            scheme: scheme.to_owned(),
            value: value.to_owned(),
            authority: None,
            valid_from: None,
            valid_to: None,
            sensitivity: None,
            verification_status: None,
            verification_date: None,
            extra: serde_json::Map::new(),
        }
    }

    /// A default `SelectorSet` is empty.
    #[test]
    fn test_selector_set_default_is_empty() {
        let ss = SelectorSet::default();
        assert!(ss.is_empty());
    }

    /// A `SelectorSet` with at least one selector is not empty.
    #[test]
    fn test_selector_set_with_one_selector_is_not_empty() {
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);
        assert!(!ss.is_empty());
    }

    /// `from_selectors` distributes each variant into the correct field.
    #[test]
    fn test_from_selectors_distributes_correctly() {
        let selectors = vec![
            Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
            Selector::EdgeType(EdgeTypeTag::Known(EdgeType::Supplies)),
            Selector::LabelKey("certified".to_owned()),
            Selector::LabelKeyValue("tier".to_owned(), "1".to_owned()),
            Selector::IdentifierScheme("lei".to_owned()),
            Selector::IdentifierSchemeValue("duns".to_owned(), "123456789".to_owned()),
            Selector::Jurisdiction(country_code("DE")),
            Selector::Name("Acme".to_owned()),
        ];
        let ss = SelectorSet::from_selectors(selectors);
        assert_eq!(ss.node_types.len(), 1);
        assert_eq!(ss.edge_types.len(), 1);
        assert_eq!(ss.label_keys.len(), 1);
        assert_eq!(ss.label_key_values.len(), 1);
        assert_eq!(ss.identifier_schemes.len(), 1);
        assert_eq!(ss.identifier_scheme_values.len(), 1);
        assert_eq!(ss.jurisdictions.len(), 1);
        assert_eq!(ss.names.len(), 1);
    }

    /// A matching node type passes the `NodeType` selector.
    #[test]
    fn test_matches_node_node_type_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);
        let node = org_node("n1");
        assert!(ss.matches_node(&node));
    }

    /// A non-matching node type fails the `NodeType` selector.
    #[test]
    fn test_matches_node_node_type_no_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Facility,
        ))]);
        let node = org_node("n1");
        assert!(!ss.matches_node(&node));
    }

    /// Multiple `NodeType` values compose with OR: node matches if it equals any.
    #[test]
    fn test_matches_node_node_type_or_composition() {
        let ss = SelectorSet::from_selectors(vec![
            Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
            Selector::NodeType(NodeTypeTag::Known(NodeType::Facility)),
        ]);
        assert!(ss.matches_node(&org_node("n1")));
        assert!(ss.matches_node(&facility_node("n2")));
        assert!(!ss.matches_node(&bare_node("n3", NodeTypeTag::Known(NodeType::Good))));
    }

    /// Node with matching label key passes `LabelKey` selector.
    #[test]
    fn test_matches_node_label_key_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
        let mut node = org_node("n1");
        node.labels = Some(vec![label("certified", None)]);
        assert!(ss.matches_node(&node));
    }

    /// Node without labels fails `LabelKey` selector.
    #[test]
    fn test_matches_node_label_key_no_labels() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
        let node = org_node("n1");
        assert!(!ss.matches_node(&node));
    }

    /// Node with labels but wrong key fails `LabelKey` selector.
    #[test]
    fn test_matches_node_label_key_wrong_key() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
        let mut node = org_node("n1");
        node.labels = Some(vec![label("tier", Some("1"))]);
        assert!(!ss.matches_node(&node));
    }

    /// Multiple `LabelKey` values compose with OR.
    #[test]
    fn test_matches_node_label_key_or_composition() {
        let ss = SelectorSet::from_selectors(vec![
            Selector::LabelKey("certified".to_owned()),
            Selector::LabelKey("audited".to_owned()),
        ]);
        let mut node = org_node("n1");
        node.labels = Some(vec![label("audited", None)]);
        assert!(ss.matches_node(&node));
    }

    /// Node with matching (key, value) label pair passes `LabelKeyValue` selector.
    #[test]
    fn test_matches_node_label_key_value_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
            "tier".to_owned(),
            "1".to_owned(),
        )]);
        let mut node = org_node("n1");
        node.labels = Some(vec![label("tier", Some("1"))]);
        assert!(ss.matches_node(&node));
    }

    /// Node with matching key but wrong value fails `LabelKeyValue` selector.
    #[test]
    fn test_matches_node_label_key_value_wrong_value() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
            "tier".to_owned(),
            "1".to_owned(),
        )]);
        let mut node = org_node("n1");
        node.labels = Some(vec![label("tier", Some("2"))]);
        assert!(!ss.matches_node(&node));
    }

    /// Node with matching key but no value (key-only label) fails `LabelKeyValue`.
    #[test]
    fn test_matches_node_label_key_value_key_only_label_no_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
            "certified".to_owned(),
            "yes".to_owned(),
        )]);
        let mut node = org_node("n1");
        node.labels = Some(vec![label("certified", None)]);
        assert!(!ss.matches_node(&node));
    }

    /// Node with matching identifier scheme passes `IdentifierScheme` selector.
    #[test]
    fn test_matches_node_identifier_scheme_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::IdentifierScheme("lei".to_owned())]);
        let mut node = org_node("n1");
        node.identifiers = Some(vec![identifier("lei", "529900T8BM49AURSDO55")]);
        assert!(ss.matches_node(&node));
    }

    /// Node without identifiers fails `IdentifierScheme` selector.
    #[test]
    fn test_matches_node_identifier_scheme_no_identifiers() {
        let ss = SelectorSet::from_selectors(vec![Selector::IdentifierScheme("lei".to_owned())]);
        let node = org_node("n1");
        assert!(!ss.matches_node(&node));
    }

    /// Multiple `IdentifierScheme` values compose with OR.
    #[test]
    fn test_matches_node_identifier_scheme_or_composition() {
        let ss = SelectorSet::from_selectors(vec![
            Selector::IdentifierScheme("lei".to_owned()),
            Selector::IdentifierScheme("duns".to_owned()),
        ]);
        let mut node = org_node("n1");
        node.identifiers = Some(vec![identifier("duns", "123456789")]);
        assert!(ss.matches_node(&node));
    }

    /// Node with matching (scheme, value) identifier passes `IdentifierSchemeValue`.
    #[test]
    fn test_matches_node_identifier_scheme_value_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::IdentifierSchemeValue(
            "duns".to_owned(),
            "123456789".to_owned(),
        )]);
        let mut node = org_node("n1");
        node.identifiers = Some(vec![identifier("duns", "123456789")]);
        assert!(ss.matches_node(&node));
    }

    /// Node with matching scheme but different value fails `IdentifierSchemeValue`.
    #[test]
    fn test_matches_node_identifier_scheme_value_wrong_value() {
        let ss = SelectorSet::from_selectors(vec![Selector::IdentifierSchemeValue(
            "duns".to_owned(),
            "123456789".to_owned(),
        )]);
        let mut node = org_node("n1");
        node.identifiers = Some(vec![identifier("duns", "999999999")]);
        assert!(!ss.matches_node(&node));
    }

    /// Node with matching jurisdiction passes `Jurisdiction` selector.
    #[test]
    fn test_matches_node_jurisdiction_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
        let mut node = org_node("n1");
        node.jurisdiction = Some(country_code("DE"));
        assert!(ss.matches_node(&node));
    }

    /// Node with different jurisdiction fails `Jurisdiction` selector.
    #[test]
    fn test_matches_node_jurisdiction_no_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
        let mut node = org_node("n1");
        node.jurisdiction = Some(country_code("US"));
        assert!(!ss.matches_node(&node));
    }

    /// Node without jurisdiction fails `Jurisdiction` selector.
    #[test]
    fn test_matches_node_jurisdiction_absent() {
        let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
        let node = org_node("n1");
        assert!(!ss.matches_node(&node));
    }

    /// Multiple `Jurisdiction` values compose with OR.
    #[test]
    fn test_matches_node_jurisdiction_or_composition() {
        let ss = SelectorSet::from_selectors(vec![
            Selector::Jurisdiction(country_code("DE")),
            Selector::Jurisdiction(country_code("FR")),
        ]);
        let mut node = org_node("n1");
        node.jurisdiction = Some(country_code("FR"));
        assert!(ss.matches_node(&node));
    }

    /// Node with name containing the pattern (case-insensitive) passes `Name` selector.
    #[test]
    fn test_matches_node_name_substring_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
        let mut node = org_node("n1");
        node.name = Some("Acme Corp".to_owned());
        assert!(ss.matches_node(&node));
    }

    /// Case-insensitive: uppercase pattern matches lowercase name.
    #[test]
    fn test_matches_node_name_case_insensitive() {
        let ss = SelectorSet::from_selectors(vec![Selector::Name("ACME".to_owned())]);
        let mut node = org_node("n1");
        node.name = Some("acme gmbh".to_owned());
        assert!(ss.matches_node(&node));
    }

    /// Node with name that does not contain the pattern fails `Name` selector.
    #[test]
    fn test_matches_node_name_no_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
        let mut node = org_node("n1");
        node.name = Some("Global Logistics Ltd".to_owned());
        assert!(!ss.matches_node(&node));
    }

    /// Node without name fails `Name` selector.
    #[test]
    fn test_matches_node_name_absent() {
        let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
        let node = org_node("n1");
        assert!(!ss.matches_node(&node));
    }

    /// `NodeType` AND `Jurisdiction`: both groups must pass.
    #[test]
    fn test_matches_node_and_across_groups_both_pass() {
        let ss = SelectorSet::from_selectors(vec![
            Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
            Selector::Jurisdiction(country_code("DE")),
        ]);
        let mut node = org_node("n1");
        node.jurisdiction = Some(country_code("DE"));
        assert!(ss.matches_node(&node));
    }

    /// `NodeType` passes but `Jurisdiction` fails → overall false.
    #[test]
    fn test_matches_node_and_across_groups_one_fails() {
        let ss = SelectorSet::from_selectors(vec![
            Selector::NodeType(NodeTypeTag::Known(NodeType::Organization)),
            Selector::Jurisdiction(country_code("DE")),
        ]);
        let mut node = org_node("n1");
        node.jurisdiction = Some(country_code("US"));
        assert!(!ss.matches_node(&node));
    }

    /// An empty `SelectorSet` matches every node (no constraints).
    #[test]
    fn test_matches_node_empty_selector_set_matches_all() {
        let ss = SelectorSet::default();
        assert!(ss.matches_node(&org_node("n1")));
        assert!(ss.matches_node(&facility_node("n2")));
    }

    /// Matching edge type passes `EdgeType` selector.
    #[test]
    fn test_matches_edge_edge_type_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::EdgeType(EdgeTypeTag::Known(
            EdgeType::Supplies,
        ))]);
        let edge = supplies_edge("e1", "a", "b");
        assert!(ss.matches_edge(&edge));
    }

    /// Non-matching edge type fails `EdgeType` selector.
    #[test]
    fn test_matches_edge_edge_type_no_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::EdgeType(EdgeTypeTag::Known(
            EdgeType::Ownership,
        ))]);
        let edge = supplies_edge("e1", "a", "b");
        assert!(!ss.matches_edge(&edge));
    }

    /// Multiple `EdgeType` values compose with OR.
    #[test]
    fn test_matches_edge_edge_type_or_composition() {
        let ss = SelectorSet::from_selectors(vec![
            Selector::EdgeType(EdgeTypeTag::Known(EdgeType::Supplies)),
            Selector::EdgeType(EdgeTypeTag::Known(EdgeType::Ownership)),
        ]);
        assert!(ss.matches_edge(&supplies_edge("e1", "a", "b")));
        assert!(ss.matches_edge(&ownership_edge("e2", "a", "b")));
    }

    /// `NodeType` selector is ignored when evaluating an edge.
    #[test]
    fn test_matches_edge_node_type_selector_is_skipped() {
        let ss = SelectorSet::from_selectors(vec![Selector::NodeType(NodeTypeTag::Known(
            NodeType::Organization,
        ))]);
        let edge = supplies_edge("e1", "a", "b");
        assert!(ss.matches_edge(&edge));
    }

    /// Jurisdiction selector is ignored when evaluating an edge.
    #[test]
    fn test_matches_edge_jurisdiction_selector_is_skipped() {
        let ss = SelectorSet::from_selectors(vec![Selector::Jurisdiction(country_code("DE"))]);
        let edge = supplies_edge("e1", "a", "b");
        assert!(ss.matches_edge(&edge));
    }

    /// `IdentifierScheme` selector is ignored when evaluating an edge.
    #[test]
    fn test_matches_edge_identifier_scheme_selector_is_skipped() {
        let ss = SelectorSet::from_selectors(vec![Selector::IdentifierScheme("lei".to_owned())]);
        let edge = supplies_edge("e1", "a", "b");
        assert!(ss.matches_edge(&edge));
    }

    /// Name selector is ignored when evaluating an edge.
    #[test]
    fn test_matches_edge_name_selector_is_skipped() {
        let ss = SelectorSet::from_selectors(vec![Selector::Name("acme".to_owned())]);
        let edge = supplies_edge("e1", "a", "b");
        assert!(ss.matches_edge(&edge));
    }

    /// Edge with matching label key in properties passes `LabelKey` selector.
    #[test]
    fn test_matches_edge_label_key_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
        let mut edge = supplies_edge("e1", "a", "b");
        edge.properties.labels = Some(vec![label("certified", None)]);
        assert!(ss.matches_edge(&edge));
    }

    /// Edge without labels in properties fails `LabelKey` selector.
    #[test]
    fn test_matches_edge_label_key_no_labels() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKey("certified".to_owned())]);
        let edge = supplies_edge("e1", "a", "b");
        assert!(!ss.matches_edge(&edge));
    }

    /// Edge with matching (key, value) label in properties passes `LabelKeyValue`.
    #[test]
    fn test_matches_edge_label_key_value_match() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
            "tier".to_owned(),
            "1".to_owned(),
        )]);
        let mut edge = supplies_edge("e1", "a", "b");
        edge.properties.labels = Some(vec![label("tier", Some("1"))]);
        assert!(ss.matches_edge(&edge));
    }

    /// Edge with wrong value fails `LabelKeyValue` selector.
    #[test]
    fn test_matches_edge_label_key_value_wrong_value() {
        let ss = SelectorSet::from_selectors(vec![Selector::LabelKeyValue(
            "tier".to_owned(),
            "1".to_owned(),
        )]);
        let mut edge = supplies_edge("e1", "a", "b");
        edge.properties.labels = Some(vec![label("tier", Some("2"))]);
        assert!(!ss.matches_edge(&edge));
    }

    /// An empty `SelectorSet` matches every edge (no constraints).
    #[test]
    fn test_matches_edge_empty_selector_set_matches_all() {
        let ss = SelectorSet::default();
        assert!(ss.matches_edge(&supplies_edge("e1", "a", "b")));
        assert!(ss.matches_edge(&ownership_edge("e2", "a", "b")));
    }
}
