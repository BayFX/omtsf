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

#[cfg(test)]
mod tests;

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
    ///
    /// The original patterns as supplied by the caller. Prefer reading
    /// [`names_lowered`][Self::names_lowered] in hot-path code; this field is
    /// kept for display and round-trip purposes.
    pub names: Vec<String>,
    /// Pre-lowercased name patterns, parallel to [`names`][Self::names].
    ///
    /// Populated automatically by [`from_selectors`][Self::from_selectors].
    /// If you construct `SelectorSet` by writing to `names` directly, also
    /// push the lowercased form here so that [`matches_node`][Self::matches_node]
    /// performs correct case-insensitive matching without redundant allocations.
    pub names_lowered: Vec<String>,
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
                Selector::Name(n) => {
                    set.names_lowered.push(n.to_lowercase());
                    set.names.push(n);
                }
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

        if !self.names_lowered.is_empty() {
            let has_match = node.name.as_ref().is_some_and(|name| {
                let name_lower = name.to_lowercase();
                self.names_lowered
                    .iter()
                    .any(|pat| name_lower.contains(pat.as_str()))
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
