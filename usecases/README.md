# OMTSF Use Cases

Six reference scenarios showing how OMTSF concepts combine to address real-world supply chain transparency problems.

---

## 1. EUDR Due Diligence

A European importer of cocoa must file a Due Diligence Statement (DDS) proving that commodities do not originate from deforested land. The importer models each origin cooperative as an `organization` node and each plantation as a `facility` node carrying `geo` coordinates at the precision required by the regulation. `supplies` edges with `commodity` properties (HS heading 1801) connect cooperatives to the importer. An `attestation` node represents the DDS itself, linked to the relevant facilities and organizations via `attested_by` edges.

**OMTSF concepts used:**

- `organization` nodes (SPEC-001, Section 4.1)
- `facility` nodes with `geo` coordinates (SPEC-001, Section 4.2)
- `supplies` edges with `commodity` property (SPEC-001, Section 6.1)
- `attestation` nodes and `attested_by` edges (SPEC-001, Sections 4.5, 7.1)
- EUDR regulatory alignment (SPEC-006, Section 3 and 3.1)

---

## 2. LkSG / CSDDD Multi-Tier Supplier Mapping

A German manufacturer subject to the LkSG must document risk analysis across its upstream supply chain. The graph contains `organization` nodes for direct suppliers (tier 1) and their sub-suppliers (tier 2+), connected by `supplies` and `subcontracts` edges carrying the `tier` property relative to the `reporting_entity`. Risk assessments are captured as `attestation` nodes with `attested_by` edges linking them to the assessed organizations.

**OMTSF concepts used:**

- `organization` nodes with external identifiers (SPEC-001, Section 4.1; SPEC-002, Section 3)
- `supplies` and `subcontracts` edges with `tier` property (SPEC-001, Sections 6.1, 6.2)
- `reporting_entity` field for perspective-anchored tier values (SPEC-001, Section 2)
- `attestation` nodes for risk documentation (SPEC-001, Sections 4.5, 7.1)
- LkSG and CSDDD regulatory alignment (SPEC-006, Section 3)

---

## 3. Multi-ERP Supplier Master Consolidation

A conglomerate runs SAP S/4HANA, Oracle SCM Cloud, and Microsoft Dynamics 365 across its divisions. Each ERP exports an `.omts` file following the mapping guidance in SPEC-005. The merge engine uses composite external identifiers (LEI, DUNS, VAT numbers) to detect overlapping supplier records across the three files. Where automated identity resolution is uncertain, `same_as` edges record probable matches for human review.

**OMTSF concepts used:**

- ERP-specific export mappings (SPEC-005, Sections 2, 3, 4)
- Composite identifier model with multiple schemes (SPEC-002, Sections 3, 5)
- Merge procedure and identity predicates (SPEC-003, Sections 2, 4)
- `same_as` edges for uncertain matches (SPEC-003, Section 7)
- Label mapping across ERPs (SPEC-005, Section 5)

---

## 4. Beneficial Ownership Transparency

A compliance team maps the corporate structure behind a key supplier to identify ultimate beneficial owners (UBOs). `organization` nodes represent legal entities in the chain. `legal_parentage` edges capture the statutory parent-subsidiary hierarchy. `ownership` edges carry `share_percent` for equity stakes. `beneficial_ownership` edges connect `person` nodes (the UBOs) to the entities they ultimately control. Person node properties are governed by the privacy rules in SPEC-004.

**OMTSF concepts used:**

- `organization` nodes (SPEC-001, Section 4.1)
- `person` nodes (SPEC-001, Section 4.4)
- `legal_parentage` edges (SPEC-001, Section 5.3)
- `ownership` edges with `share_percent` (SPEC-001, Section 5.1)
- `beneficial_ownership` edges (SPEC-001, Section 5.5)
- Person node privacy rules (SPEC-004, Section 5)
- EU AMLD 5/6 alignment (SPEC-006, Section 3)

---

## 5. CBAM Embedded Emissions Tracking

An EU importer subject to the Carbon Border Adjustment Mechanism must report embedded emissions for goods originating outside the EU. `facility` nodes represent non-EU installations where goods are produced. `organization` nodes represent the operators of those installations. `operates` edges link operators to facilities. `produces` edges connect facilities to `consignment` nodes carrying emissions-related properties. `attestation` nodes capture third-party verification of declared emission values.

**OMTSF concepts used:**

- `facility` nodes as installations (SPEC-001, Section 4.2)
- `organization` nodes as operators (SPEC-001, Section 4.1)
- `consignment` nodes (SPEC-001, Section 4.6)
- `operates` edges (SPEC-001, Section 6.6)
- `produces` edges (SPEC-001, Section 6.7)
- `attestation` nodes for verification (SPEC-001, Sections 4.5, 7.1)
- EU CBAM regulatory alignment (SPEC-006, Section 3)

---

## 6. Selective Disclosure for Partner Sharing

A brand owner wants to share a subset of its supply chain graph with an auditor without exposing commercially sensitive supplier identities. The file-level `disclosure_scope` is set to `partner`. Nodes whose identifiers are classified at a higher sensitivity level than the disclosure scope are replaced with `boundary_ref` nodes, which carry only a salted hash. The auditor receives a structurally valid subgraph showing the shape of the supply chain and attestation coverage without revealing protected identities.

**OMTSF concepts used:**

- `disclosure_scope` file-level field (SPEC-001, Section 2; SPEC-004, Section 3)
- `boundary_ref` nodes (SPEC-001, Section 4.7; SPEC-004, Section 4)
- Identifier sensitivity levels (SPEC-004, Section 2)
- `file_salt` for boundary reference hashing (SPEC-001, Section 2)
- Edge property sensitivity (SPEC-004, Section 2.1)
