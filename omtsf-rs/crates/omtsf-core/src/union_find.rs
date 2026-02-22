//! Union-Find (disjoint set) data structure for node identity resolution.
//!
//! Implements the algorithm described in merge.md Section 2.1.
//!
//! Path compression uses iterative path-halving: during [`UnionFind::find`] each
//! visited node is pointed directly at its grandparent, halving the path length
//! without needing a second pass or recursion. Union-by-rank keeps trees shallow;
//! when ranks are equal the **lower ordinal** becomes the root, ensuring that
//! [`UnionFind::find`] returns a deterministic representative regardless of
//! operation order (required for commutativity).

/// A union-find (disjoint set) structure with path-halving and union-by-rank.
///
/// Each element is identified by a `usize` ordinal in `[0, n)` where `n` is
/// the number of elements supplied at construction time.
///
/// # Determinism
///
/// When two sets of equal rank are merged, the lower ordinal is chosen as the
/// new root. This guarantees that `find` returns the same representative for
/// any given merge history, independent of the order in which `union` is called.
#[derive(Debug, Clone)]
pub struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    /// Creates a new `UnionFind` with `n` singleton sets.
    ///
    /// Each element `i` is initially its own representative (`parent[i] == i`,
    /// `rank[i] == 0`).
    pub fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0u8; n],
        }
    }

    /// Returns the representative of the set containing `x`.
    ///
    /// Uses iterative path-halving: each node visited during the traversal is
    /// linked directly to its grandparent. This achieves the inverse-Ackermann
    /// amortized bound without recursion.
    ///
    /// # Panics
    ///
    /// Does not panic. If `x >= n` the caller has a logic error; however this
    /// function will still terminate (with an out-of-bounds index panic from the
    /// Vec, which is acceptable for a logic error in the caller).
    pub fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            let grandparent = self.parent[self.parent[x]];
            self.parent[x] = grandparent;
            x = grandparent;
        }
        x
    }

    /// Merges the sets containing `a` and `b`.
    ///
    /// Uses union-by-rank. When ranks are equal, the **lower ordinal** becomes
    /// the new root, providing a deterministic tie-break for commutativity.
    pub fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);

        if ra == rb {
            return;
        }

        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => {
                self.parent[ra] = rb;
            }
            std::cmp::Ordering::Greater => {
                self.parent[rb] = ra;
            }
            std::cmp::Ordering::Equal => {
                if ra < rb {
                    self.parent[rb] = ra;
                    self.rank[ra] += 1;
                } else {
                    self.parent[ra] = rb;
                    self.rank[rb] += 1;
                }
            }
        }
    }

    /// Returns the number of elements in this `UnionFind`.
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Returns `true` if this `UnionFind` contains no elements.
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn new_creates_singletons() {
        let mut uf = UnionFind::new(5);
        for i in 0..5 {
            assert_eq!(
                uf.find(i),
                i,
                "element {i} should be its own representative"
            );
        }
    }

    #[test]
    fn find_self_is_identity() {
        let mut uf = UnionFind::new(3);
        assert_eq!(uf.find(0), 0);
        assert_eq!(uf.find(1), 1);
        assert_eq!(uf.find(2), 2);
    }

    #[test]
    fn union_two_elements_same_set() {
        let mut uf = UnionFind::new(4);
        uf.union(0, 1);
        assert_eq!(
            uf.find(0),
            uf.find(1),
            "after union, elements should share a representative"
        );
    }

    #[test]
    fn union_does_not_affect_others() {
        let mut uf = UnionFind::new(4);
        uf.union(0, 1);
        assert_ne!(uf.find(0), uf.find(2));
        assert_ne!(uf.find(0), uf.find(3));
        assert_ne!(uf.find(2), uf.find(3));
    }

    #[test]
    fn transitive_closure() {
        let mut uf = UnionFind::new(3);
        uf.union(0, 1);
        uf.union(1, 2);
        let r0 = uf.find(0);
        let r1 = uf.find(1);
        let r2 = uf.find(2);
        assert_eq!(r0, r1);
        assert_eq!(r1, r2);
    }

    #[test]
    fn deterministic_representative_lower_ordinal_wins_on_tie() {
        let mut uf = UnionFind::new(5);
        uf.union(3, 1);
        assert_eq!(uf.find(3), 1, "lower ordinal 1 should win over 3");
        assert_eq!(uf.find(1), 1);
    }

    #[test]
    fn union_commutativity_same_representative() {
        let mut uf_ab = UnionFind::new(2);
        uf_ab.union(0, 1);
        let rep_ab = uf_ab.find(0);

        let mut uf_ba = UnionFind::new(2);
        uf_ba.union(1, 0);
        let rep_ba = uf_ba.find(0);

        assert_eq!(rep_ab, rep_ba, "union must be commutative");
    }

    #[test]
    fn idempotent_union() {
        let mut uf = UnionFind::new(3);
        uf.union(0, 1);
        let rep_before = uf.find(0);
        uf.union(0, 1);
        let rep_after = uf.find(0);
        assert_eq!(rep_before, rep_after, "double-union must be idempotent");
    }

    #[test]
    fn path_halving_compresses_path() {
        let mut uf = UnionFind::new(5);
        uf.union(0, 1);
        uf.union(0, 2);
        uf.union(0, 3);
        uf.union(0, 4);
        let root = uf.find(0);
        for i in 0..5 {
            assert_eq!(
                uf.find(i),
                root,
                "all elements should share the same representative after path compression"
            );
        }
    }

    #[test]
    fn union_by_rank_higher_rank_wins() {
        let mut uf = UnionFind::new(3);
        uf.union(0, 1);
        uf.union(0, 2);
        assert_eq!(
            uf.find(2),
            0,
            "singleton should go under the higher-rank root"
        );
        assert_eq!(uf.find(1), 0);
    }

    #[test]
    fn len_and_is_empty() {
        let uf = UnionFind::new(0);
        assert!(uf.is_empty());
        assert_eq!(uf.len(), 0);

        let uf = UnionFind::new(3);
        assert!(!uf.is_empty());
        assert_eq!(uf.len(), 3);
    }

    #[test]
    fn large_component_merge() {
        const N: usize = 64;
        let mut uf = UnionFind::new(N);
        for i in 1..N {
            uf.union(0, i);
        }
        let root = uf.find(0);
        for i in 0..N {
            assert_eq!(
                uf.find(i),
                root,
                "element {i} should share the root after merging all into one component"
            );
        }
    }
}
