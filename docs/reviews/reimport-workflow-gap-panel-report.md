# Expert Panel Report: Re-Import Workflow Gap in SPEC-003

**Date:** 2026-02-25
**Topic:** The update/re-import workflow gap in SPEC-003 (Merge Semantics) — when a user imports an Excel supplier list, then later modifies values and re-imports, nodes cannot be matched to the originals because `internal` scheme identifiers are excluded from cross-file merge.

---

## Panel Chair Summary

Six domain experts independently reviewed the re-import workflow gap in SPEC-003. The panel reached **unanimous consensus** on the core finding: re-import from the same source and cross-party merge are structurally different operations, and the specification's failure to distinguish them is a critical gap that will produce silent data corruption (duplicate nodes) in the most common early-adoption workflow.

All six panelists agree that the `internal` scheme exclusion from cross-file merge (SPEC-003, Section 2) is correct for the multi-party case and must be preserved. The fix is not to weaken the merge predicate but to define a **new named operation** — variously called "same-origin update," "revise," or "origin-scoped upsert" — that uses `internal` identifier matching scoped to a shared `authority` value. This operation has different preconditions (same origin), different conflict semantics (newer file wins), and different algebraic properties (directional, not commutative) than general merge.

The panel also reached strong consensus on two prerequisite fixes: (1) the hardcoded `authority: "supplier-list"` in the Excel import template must be replaced with a stable, user-specified authority value, and (2) SPEC-003 Section 5's algebraic properties (commutativity, associativity, idempotency) must be preserved for the existing `merge` operation by keeping the new operation formally separate. The Standards Expert and Graph Modeling Expert both provide formal arguments for why embedding same-origin semantics in the existing merge predicate would break associativity in multi-file scenarios.

The panel diverges on naming and placement: the Standards Expert argues for "revise" with a `revision_of` provenance pointer at the file level; the Graph Modeling Expert prefers "same-origin update" modeled after Neo4j's `MERGE`/`ON MATCH SET` pattern; the Entity Identification Expert and Enterprise Integration Expert both reference MDM prior art (Informatica XREF, SAP MDG inbound processing). These are terminological differences, not architectural disagreements. The recommended approach synthesized below accommodates all perspectives.

---

## Panel Composition

| Panelist | Role | Key Focus Area |
|----------|------|----------------|
| Graph Modeling Expert | Graph Data Modeling & Algorithm Specialist | Formal operation semantics, algebraic properties |
| Entity Identification Expert | Entity Identification & Corporate Hierarchy Specialist | Authority scoping, enrichment preservation, MDM prior art |
| Enterprise Integration Expert | Enterprise Systems Architect | ERP re-export workflows, SAP/Oracle/D365 delta patterns |
| Procurement Expert | Chief Procurement Officer | Practitioner usability, adoption risk, Coupa/Ariba comparison |
| Supply Chain Expert | Supply Chain Visibility & Risk Analyst | Regulatory compliance (CSDDD, LkSG), deep-tier visibility |
| Standards Expert | Standards Development & Interoperability Specialist | Specification rigor, GS1 EPCIS prior art, W3C PROV alignment |

---

## Consensus Findings

### 1. Re-import and merge are different operations (all 6 experts)

Every panelist independently concluded that same-origin re-import cannot be handled by the existing merge operation. The merge predicate is designed for files from different parties with potentially different identifier namespaces. Re-import is a same-party, same-namespace update. Conflating them forces either weakening the merge predicate (breaking safety) or accepting duplicates (breaking usability).

Prior art cited: Neo4j `MERGE` vs. `MATCH...SET` (Graph Modeling), Informatica MDM XREF (Entity ID, ERP Integration), SAP MDG inbound processing vs. consolidation (Entity ID, ERP Integration), GS1 EPCIS `eventID` correction semantics (Standards), Coupa/Ariba supplier upsert (Procurement), W3C RDF Diff/Patch (Graph Modeling, Standards).

### 2. The `internal` exclusion from merge is correct and must be preserved (all 6 experts)

No panelist recommends changing SPEC-003 Section 2. The exclusion prevents false-positive merges when unrelated organizations use the same internal identifiers.

### 3. The `authority: "supplier-list"` hardcoded value is a blocker (5 of 6 experts)

Graph Modeling, Entity ID, ERP Integration, Procurement, and Supply Chain experts all flag this. A generic constant authority value would cause false matches between different teams' supplier lists. It must be replaced with a stable, user-specified, source-specific authority.

### 4. Option C (encourage external identifiers) is necessary but insufficient (all 6 experts)

All panelists acknowledge that external identifiers improve merge quality. All also note that for mid-market procurement teams and deep-tier suppliers in developing markets, external identifiers may never be available. The re-import solution must work for internal-only files.

---

## Critical Issues

| # | Issue | Flagged By | Summary |
|---|-------|-----------|---------|
| C1 | No operation exists for same-origin reconciliation | All 6 | SPEC-003 defines merge for different-origin files. There is no operation for reconciling a new version against its own prior version. Every re-import of internal-only data produces duplicates. |
| C2 | Hardcoded `authority: "supplier-list"` breaks same-origin detection | Graph Modeling, Entity ID, ERP Integration, Procurement, Supply Chain | The Excel template uses a generic constant. Two different teams' lists would be indistinguishable, causing false matches in any same-origin mechanism. |
| C3 | Enrichment lifecycle assumption is unrealistic for automation | ERP Integration, Procurement | SPEC-005 Section 6 assumes human-assisted enrichment before re-import. Enterprise integration pipelines run as unattended batch jobs. |
| C4 | Regulatory reporting accuracy undermined | Supply Chain | Duplicate node proliferation means entity counts in CSDDD/LkSG reporting are inflated. A company reporting 500 Tier 2 suppliers after 4 quarterly re-imports actually has 125. |

---

## Major Issues

| # | Issue | Flagged By | Summary |
|---|-------|-----------|---------|
| M1 | Option A as originally described is underspecified and risks breaking algebraic properties | Graph Modeling, Entity ID, Standards | Embedding same-origin semantics in the existing merge predicate breaks associativity in multi-file scenarios. Must be a separate operation. |
| M2 | No guidance on enrichment preservation during re-import | Entity ID, ERP Integration | If a base graph was enriched with external identifiers after initial import, a same-origin update must preserve those identifiers. Currently unspecified. |
| M3 | No behavior specified for removed suppliers | Entity ID, Supply Chain | When a supplier disappears from the Excel between exports, no signal exists to retire the node. Needs an unmatched-node policy. |
| M4 | No diagnostic warning for internal-only nodes | ERP Integration, Procurement | No L2 warning tells users that their internal-only file will produce duplicates on re-import. Silent failure. |
| M5 | Option B (stable node IDs) creates brittle key dependencies | Entity ID, Procurement | Requiring deterministic node ID generation across re-imports adds fragile ETL-pipeline-style dependencies. |

---

## Minor Issues

| # | Issue | Flagged By | Summary |
|---|-------|-----------|---------|
| m1 | Option C not actionable for deep-tier suppliers | Supply Chain, Procurement, Entity ID | Tier 2/3 suppliers in developing markets may never have LEI/DUNS. |
| m2 | `supplier_id` documented as optional without re-import consequence warning | Procurement, Supply Chain | Users who omit it don't know they're permanently breaking update capability. |
| m3 | `merge_metadata` lacks same-origin match statistics | Graph Modeling, ERP Integration | No fields to record whether internal-scheme matching was used or how many nodes were matched/inserted/retained. |
| m4 | SPEC-003 Section 8 (Intra-File Dedup) doesn't address longitudinal re-import | ERP Integration | A reader looking for re-import guidance won't find it. |
| m5 | Three options presented as mutually exclusive when they are complementary | Entity ID | Options A + C should be pursued together. |

---

## Consolidated Recommendations

### P0 — Immediate (before next spec revision)

**R1. Define a "Same-Origin Update" operation in SPEC-003.**
Add a new normative section (e.g., Section 4a or Section 11) defining same-origin update as a named operation distinct from `merge`. Specification:

- **Precondition:** Both files carry `internal` identifiers with identical `authority` values on the nodes to be reconciled.
- **Identity predicate:** Nodes sharing `scheme: "internal"`, equal `authority`, and equal `value` are update candidates.
- **Property resolution:** New file wins for scalar properties (last-write-wins); old value recorded in `_conflicts`.
- **Identifier handling:** Union of identifier arrays, preserving externally-enriched identifiers from the base.
- **Algebraic properties:** Directional (not commutative). Idempotent: `update(base, base) = base`.
- **Unmatched-node policy:** Configurable — `retain` (default, safe), `flag` (annotate for review), `expire` (set `valid_to`).

*Originated by: all 6 experts. Naming varies ("same-origin update," "revise," "origin-scoped upsert") but semantics converge.*

**R2. Fix the Excel template authority value.**
Replace hardcoded `authority: "supplier-list"` with a user-specified, stable, source-specific authority. Recommend pattern: `{org-identifier}:{list-scope}` (e.g., `acme-corp:approved-suppliers`). Persist in template metadata so it auto-populates on re-export. Add a `--authority` CLI flag to the import command.

*Originated by: Entity ID, Graph Modeling, ERP Integration, Procurement, Supply Chain.*

**R3. Add a clarifying note in SPEC-003 Section 2.**
Insert: "The merge operation in this section combines files from potentially different origins using external identifiers. It is not designed for reconciling successive exports from the same source system. For that use case, see Section [same-origin update]. Applying general merge to two versions of the same internal-only file will produce duplicate nodes."

*Originated by: Graph Modeling, Entity ID, Standards.*

### P1 — Before v1.0

**R4. Make `authority` stability normative in SPEC-002.**
Add a normative requirement: `authority` values on `internal` identifiers in files intended for re-import MUST be stable across exports and unique to the producing source. Provide good/bad examples.

*Originated by: Entity ID, Graph Modeling, ERP Integration, Standards.*

**R5. Update SPEC-005 Section 6 enrichment lifecycle.**
Add to the enrichment level table: at "Internal-only" level, same-origin update is available for re-import. Add a Section 6.4 ("Recurring Export and Re-Import") documenting the workflow with a concrete SAP monthly-export example.

*Originated by: Entity ID, ERP Integration, Procurement, Supply Chain.*

**R6. Add L2 validation warning for re-import duplicate risk.**
When a file contains nodes with only `internal` identifiers, emit: "This file contains N nodes with only internal identifiers. Re-importing without same-origin update mode will produce duplicates." Cross-reference SPEC-002 L2-EID-01.

*Originated by: ERP Integration, Procurement.*

**R7. Preserve SPEC-003 Section 5 algebraic properties unchanged.**
Do not extend the existing `merge` identity predicate. The new same-origin update has its own properties section.

*Originated by: Graph Modeling, Standards, Entity ID.*

**R8. Add `revision_of` provenance pointer.**
Allow the file header or `merge_metadata` to declare: "This file is a revision of file X (identified by content hash)." Enables downstream consumers to detect stale copies and supports audit trails.

*Originated by: Standards (W3C PROV-DM `wasDerivedFrom` analogy).*

### P2 — Future

**R9. Document enrichment-preservation invariant.**
Specify that same-origin update MUST NOT discard external identifiers added by enrichment. The update is additive for identifiers.

*Originated by: Entity ID.*

**R10. Extend `merge_metadata` for same-origin statistics.**
Record: operation type (merge vs. same-origin update), authority matched on, counts of nodes updated/inserted/retained-without-match.

*Originated by: Graph Modeling, ERP Integration.*

**R11. Promote `supplier_id` to strongly-recommended and document re-import consequence of omitting it.**
Update template documentation: "If blank, re-importing will create duplicate records. Strongly recommended for any list updated over time."

*Originated by: Procurement, Supply Chain.*

**R12. Define composite match heuristic for identifier-poor entities.**
For Tier 2/3 suppliers with no stable ID at all, document a `name` + `jurisdiction` + `parent_supplier` matching heuristic for same-origin updates, with `same_as` edge emission at `confidence: "probable"`.

*Originated by: Supply Chain.*

---

## Cross-Domain Interactions

| Interaction | Domains | Description |
|-------------|---------|-------------|
| Authority stability is the linchpin | Entity ID + Graph Modeling + ERP Integration | The entire same-origin update mechanism depends on `authority` values being stable and unique per source. SPEC-002 must make this normative; the Excel template must enforce it; ERP export guidance must document it. |
| Enrichment preservation bridges update and merge | Entity ID + Supply Chain + ERP Integration | After same-origin update, enriched identifiers must survive. After enrichment, the file can participate in cross-party merge. The two operations form a pipeline that must be specified end-to-end. |
| Regulatory accuracy depends on update semantics | Supply Chain + Procurement + Standards | CSDDD/LkSG require demonstrable, repeatable supply chain mapping. A graph that inflates entity counts on every re-import cannot support regulatory reporting. The same-origin update operation is a compliance infrastructure requirement. |
| Algebraic safety constrains the solution space | Graph Modeling + Standards | The formal properties of merge (commutativity, associativity, idempotency) rule out embedding same-origin semantics in the existing merge predicate. The new operation must be separate. This is not a preference — it is a formal requirement. |
| UX determines adoption | Procurement + ERP Integration | The same-origin update must be the default path for re-import, not an advanced option. If users must understand the distinction between merge and update to avoid duplicates, adoption will stall. The CLI should auto-detect same-origin when authority values match. |

---

## Individual Expert Reports

### Graph Modeling Expert

*(Full report: `docs/reviews/merge-reimport-review-graph-modeling.md`)*

Core argument: The problem is a missing operation, not a flaw in the merge predicate. Recommends a "Same-Origin Update Operation" section in SPEC-003, modeled after Neo4j `MERGE`/`ON MATCH SET` and W3C RDF Diff/Patch. Strongly argues that the existing merge algebraic properties must be preserved by keeping the new operation separate. Flags that Option A as originally described breaks commutativity if embedded in the merge predicate.

### Entity Identification Expert

*(Full report: `docs/reviews/merge-reimport-review-entity-identification.md`)*

Core argument: Internal-only is the permanent state for most procurement teams, not a transitional phase. References Informatica MDM XREF and SAP MDG as prior art for distinguishing same-origin upsert from cross-origin consolidation. Uniquely raises the enrichment-preservation invariant (external identifiers added post-import must survive re-import) and the unmatched-node policy (what happens when a supplier is removed between exports). Flags the `authority: "supplier-list"` hardcoded value as a critical blocker.

### Enterprise Integration Expert

Core argument: The re-import gap is the dominant real-world workflow for enterprise procurement. At scale, 5,000 vendors become 10,000 nodes after two monthly cycles. References SAP MDG key mapping tables and Informatica MDM `PKEY_SRC_OBJECT` as direct prior art. Recommends same-origin merge mode as an importer-level behavior with a dedicated CLI flag, plus an L2 validation warning for internal-only nodes. Flags that the enrichment lifecycle assumption is unrealistic for automated batch pipelines.

### Procurement Expert

Core argument: This is not an edge case — it is the primary use pattern for 80% of procurement teams. Compares OMTS unfavorably to Coupa, Ariba, and Jaggaer, all of which use a stable buyer-assigned key as the update anchor. Emphasizes that mid-market companies will never enrich to DUNS/LEI level for most suppliers. Recommends framing same-origin update as an importer behavior (SPEC-005), not a core spec change (SPEC-003), to keep the spec simple. Uniquely flags that `supplier_id` being optional without re-import consequence warning is a documentation failure.

### Supply Chain Expert

Core argument: The problem is worst for Tier 2/3 suppliers — exactly the entities regulators want visibility on. Duplicate proliferation undermines CSDDD and LkSG reporting accuracy. A company reporting 500 Tier 2 suppliers when it actually has 125 is providing misleading data. Recommends same-origin update with a stable authority, plus a composite match heuristic (`name` + `jurisdiction` + `parent_supplier`) for identifier-poor deep-tier entities.

### Standards Expert

Core argument: The absence of a formally defined update operation is a specification design error, not a tooling gap. References GS1 EPCIS `eventID` correction semantics (and the community's experience untangling conflated update/creation intent) and W3C PROV-DM's `wasDerivedFrom` distinction. Argues Option A breaks associativity in multi-file scenarios and should be rejected on formal grounds. Recommends "revise" as a separate named operation with a `revision_of` provenance pointer. Notes that OMTS could make a differentiating standards contribution by specifying same-origin update semantics, which are underspecified in the broader standards landscape.
