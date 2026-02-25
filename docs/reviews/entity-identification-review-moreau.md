# Expert Review: OMTS Entity Identification Specification (Revision 2)

**Reviewer:** Regulatory Compliance Expert, Supply Chain Regulatory Compliance Advisor
**Spec Reviewed:** OMTS-SPEC-001 -- Entity Identification (Draft, Revision 2, 2026-02-17)
**Date:** 2026-02-18
**Review Type:** Post-panel follow-up (assessing P0 remediation)

---

## Assessment

Revision 2 of the Entity Identification Specification represents a decisive response to the two critical findings I raised during the initial panel review: the absence of beneficial ownership representation and the lack of an attestation/certification model. Both gaps have been addressed with purpose-built constructs that demonstrate a clear understanding of the regulatory requirements driving them.

The `person` node type (Section 5.4) with default `confidential` sensitivity, mandatory omission under `disclosure_scope: "public"`, and the requirement for producers to assess data protection compliance before generation -- this is a thoughtful implementation that respects the tension between CSDDD/AMLD transparency obligations and GDPR data minimization. The `beneficial_ownership` edge type (Section 6.5) correctly captures percentage, control type (voting rights, capital, other means, senior management), and temporal validity. Importantly, the spec leaves the 25% UBO threshold determination to tooling rather than baking a jurisdiction-specific rule into the data model. This is the right call: the threshold varies across Member States' transpositions.

The attestation model (Section 8) is fit for purpose. The `attestation` node type with typed categories (certification, audit, due diligence statement, self-declaration) and the `attested_by` edge linking entities, facilities, or goods to attestation nodes provides the structural foundation needed for EUDR due diligence statements, SA8000/SMETA audit records, and LkSG risk analysis documentation. The inclusion of `outcome` (pass, conditional_pass, fail, pending) and `reference` fields addresses the audit trail requirements that regulators expect.

The supply relationship edge taxonomy (Section 7) now formally defines `supplies`, `subcontracts`, `tolls`, `distributes`, `brokers`, `operates`, and `produces` with regulatory relevance annotations. The explicit note that a `supplies` edge constitutes a "direct business relationship" under CSDDD Article 3(e) and a "direct supplier" under LkSG Section 2(7) is exactly the kind of regulatory mapping that compliance teams need. The `subcontracts` edge correctly notes that LkSG Section 9 triggers due diligence obligations upon substantiated knowledge of violations at a subcontractor -- this distinction between direct and delegated production is load-bearing for regulatory compliance.

The regulatory alignment table (Section 12.3) has been expanded to include AMLD 5/6 coverage via `person` nodes and `beneficial_ownership` edges, and attestation coverage for EUDR and LkSG. This table is now a credible quick-reference for compliance officers evaluating OMTS adoption.

## Strengths

- **GDPR-aware beneficial ownership model.** The layered privacy design -- `person` nodes default to `confidential`, `beneficial_ownership` edges inherit that sensitivity, both are stripped from public-scope files -- demonstrates that the spec authors engaged seriously with the GDPR/AMLD tension rather than treating it as someone else's problem.
- **Attestation type taxonomy.** The four-type enum (`certification`, `audit`, `due_diligence_statement`, `self_declaration`) maps cleanly to the documentary evidence types that regulators actually request. The `other` escape valve preserves extensibility without undermining the controlled vocabulary.
- **Regulatory relevance annotations on supply edges.** Annotating each edge type with its regulatory significance (CSDDD article, LkSG section) bridges the gap between the data model and the legal obligations it serves. This is rare in technical specifications and highly valuable.
- **Attestation scope field on `attested_by` edges.** The free-text `scope` property (e.g., "working conditions", "deforestation-free") allows a single facility to carry multiple attestations covering different compliance domains without type proliferation.

## Concerns

- **[Major] Attestation model lacks chain-of-custody linkage.** The `attested_by` edge links an entity or facility to an attestation, but there is no mechanism to link an attestation to a specific consignment, shipment, or batch of goods flowing through the network. EUDR Regulation (EU) 2023/1115 requires due diligence statements to be linked to specific products placed on the EU market, with traceability to the production plot. The current model can say "this facility has an EUDR DDS" but not "this specific shipment of cocoa is covered by DDS-2026-00142." This requires either allowing `attested_by` edges from `good` nodes (which is syntactically permitted but not illustrated) or introducing a consignment-level construct. Without this, the attestation model covers facility-level compliance but falls short of transaction-level chain of custody.

- **[Major] No attestation revocation or supersession model.** Certifications are revoked. Audit results are superseded by follow-up audits. Due diligence statements are amended. The attestation model has `valid_to` for expiration but no mechanism for recording that an attestation was revoked before its expiry, or that a newer attestation supersedes an older one. In practice, a revoked SA8000 certificate is materially different from an expired one -- the former may indicate discovered violations. A `status` field (active, revoked, superseded, withdrawn) or a `superseded_by` edge between attestation nodes would address this.

- **[Moderate] `beneficial_ownership` percentage is optional.** While I understand the pragmatic reason -- UBO registries in many jurisdictions do not publish precise percentages -- the absence of even a threshold band (e.g., "25-50%", "50-75%", ">75%") weakens the regulatory utility. Under AMLD, the 25% threshold is a hard legal line. If the percentage is omitted entirely, downstream tooling cannot determine UBO status programmatically. A `percentage_band` enum as an alternative to the precise `percentage` field would preserve usability when exact figures are unavailable.

- **[Moderate] No regulatory jurisdiction field on attestation nodes.** An SA8000 certification is globally recognized, but an EUDR due diligence statement is jurisdiction-specific (EU). A LkSG risk analysis report is relevant to German regulatory authorities. The attestation node has no field indicating which regulatory regime it serves. This matters when a multinational must generate jurisdiction-specific compliance reports from a single merged graph.

- **[Minor] `former_identity` edge does not capture successor liability.** When Company A merges into Company B, regulatory liability (pending CSDDD complaints, LkSG remediation obligations) may transfer to the successor entity. The `former_identity` edge captures the corporate event but not the liability transfer implications. This is a P2 concern that should be acknowledged.

## Recommendations

1. **(P1) Illustrate and formalize consignment-level attestation linkage.** Add an example showing an `attested_by` edge from a `good` node to an EUDR DDS attestation node. Clarify in Section 8.2 that `attested_by` edges MAY originate from any node type (organization, facility, or good). Consider whether a `consignment` or `lot` construct (per Dr. Supply Chain Expert's batch-level recommendation) is needed to complete the chain-of-custody model.

2. **(P1) Add attestation lifecycle status.** Extend the `attestation` node with a `status` field: `active`, `revoked`, `superseded`, `withdrawn`. Optionally add a `superseded_by` property containing the graph-local ID of the replacement attestation node. This is essential for audit trail integrity -- regulators need to know not just what certifications exist, but whether they remain valid.

3. **(P1) Add `percentage_band` as an alternative to `percentage` on `beneficial_ownership` edges.** Enum values: `below_25`, `25_to_50`, `50_to_75`, `above_75`. This enables programmatic UBO threshold determination even when exact percentages are unavailable, which is the common case for beneficial ownership data sourced from public registries.

4. **(P2) Add `regulatory_jurisdiction` field to attestation nodes.** Optional ISO 3166-1 alpha-2 field indicating the regulatory regime the attestation serves. This enables jurisdiction-filtered compliance reporting from merged multi-jurisdiction graphs.

5. **(P2) Acknowledge successor liability gap in `former_identity` documentation.** Add a note to Section 6.4 that regulatory liability transfer in merger/acquisition events is a tooling and legal analysis concern beyond the scope of the data model, but that the `former_identity` edge provides the structural foundation for such analysis.

## Cross-Expert Notes

- **For Dr. Supply Chain Expert (Supply Chain Visibility):** The consignment-level attestation gap I identify above directly connects to your recommendation for batch/lot-level `good` node support (P2-11 in the panel report). If the spec introduces a lot-level construct, attestation linkage at that granularity becomes straightforward. I would support elevating your lot-level recommendation to P1 given the EUDR enforcement timeline (December 2025 for large operators, already in effect).

- **For Dr. Security & Privacy Expert (Security & Privacy):** The `person` node privacy constraints are well-designed. One edge case worth examining: when `beneficial_ownership` edges are stripped from public-scope files, the resulting graph may still reveal UBO-adjacent information through `ownership` edge chains that terminate at entities with suspiciously high ownership percentages. A determined adversary could infer the existence of redacted `person` nodes. This is likely acceptable given the GDPR balancing test, but worth documenting.

- **For Entity Identification Expert (Entity Identification):** The `beneficial_ownership` edge type correctly handles the UBO data model, but in practice, UBO data quality is poor -- national registers are often outdated, nominee structures obscure true ownership, and thresholds vary across jurisdictions. The `confidence` field you recommended for identifier records (P1-25/31 in the panel report) would be equally valuable on `beneficial_ownership` edges. I would support a generalized confidence/verification metadata model applicable to both identifiers and edges.

- **For Open Source Strategy Expert (Open Source Strategy):** The attestation model creates an opportunity for community-maintained mappings between attestation `standard` values and regulatory requirements. A registry of recognized `standard` codes (similar to the identifier scheme vocabulary) would prevent fragmentation -- one producer writing `SA8000:2014` and another writing `SA-8000` for the same certification standard.
