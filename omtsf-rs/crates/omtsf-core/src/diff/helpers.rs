use crate::enums::{EdgeTypeTag, NodeTypeTag};

/// Returns the `snake_case` string for a [`NodeTypeTag`].
///
/// Known variants resolve to a `&'static str` with no allocation;
/// extension variants return the stored `String` by reference.
pub(super) fn node_type_str(tag: &NodeTypeTag) -> &str {
    tag.as_str()
}

/// Returns the `snake_case` string for an [`EdgeTypeTag`].
///
/// Known variants resolve to a `&'static str` with no allocation;
/// extension variants return the stored `String` by reference.
pub(super) fn edge_type_str(tag: &EdgeTypeTag) -> &str {
    tag.as_str()
}
