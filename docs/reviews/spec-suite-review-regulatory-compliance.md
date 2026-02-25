# Expert Review: OMTS Specification Suite (Regulatory Compliance)

**Reviewer:** Regulatory Compliance Expert, Supply Chain Regulatory Compliance Advisor
**Specs Reviewed:** OMTS-SPEC-001 through OMTS-SPEC-006 (all Draft, Revision 1, 2026-02-18)
**Review Date:** 2026-02-18

---

## Assessment

From a regulatory compliance perspective, OMTS demonstrates a remarkably well-informed mapping to the current European and international due diligence regulatory landscape. The specification suite addresses the structural data requirements of the EU Corporate Sustainability Due Diligence Directive (CSDDD), the EU Deforestation Regulation (EUDR), the German Supply Chain Due Diligence Act (LkSG), CBAM, and AMLD 5/6 with a degree of specificity that is rare in open data format proposals. The graph model captures the core regulatory entities -- organizations with jurisdiction, facilities with geolocation, goods with commodity codes, persons as beneficial owners, and attestations covering certifications and due diligence statements -- and the edge types explicitly encode the relationship categories that trigger regulatory obligations. The inline regulatory relevance annotations (e.g., `supplies` = "direct business relationship" under CSDDD Article 3(e); `subcontracts` triggering LkSG Section 9 obligations) are valuable for implementers who need to understand why specific data elements exist.

The EUDR coverage is particularly strong. The `consignment` node type with `lot_id` and `origin_country`, combined with `facility` nodes carrying WGS 84 coordinates or GeoJSON polygon boundaries, directly addresses Article 9 due diligence statement requirements, including the geolocation-to-plot traceability that will apply to large operators from 30 December 2026 under the revised regulation (EU 2025/2650). The `attestation` node type with `attestation_type: "due_diligence_statement"` and `standard: "EUDR-DDS"` provides a clean mechanism for referencing DDS submissions. However, the spec should be aware that the EUDR's December 2025 revision introduces simplified requirements and a Commission simplification review due by 30 April 2026, which may alter geolocation precision requirements for certain commodity categories.

The most significant regulatory gap is the absence of a temporal audit trail mechanism at the file level. Regulations like CSDDD (even as narrowed by the December 2025 Omnibus I agreement to companies with 5,000+ employees and EUR 1.5 billion turnover) and LkSG require demonstrable ongoing monitoring, not point-in-time snapshots. The `snapshot_date` field captures a single moment, but there is no normative mechanism for linking successive snapshots into a compliance timeline that a supervisory authority (which EU member states must designate by July 2026 under CSDDD) could audit. Additionally, CBAM's definitive phase, which began 1 January 2026 with the first surrender deadline of 30 September 2027, requires embedded emissions data that the current `consignment` node type does not carry.

---

## Strengths

- **Regulatory-aware edge type design.** The distinction between `supplies` (CSDDD Article 3(e) "direct business relationship"), `subcontracts` (LkSG Section 9 indirect obligations), and `sells_to` (CSDDD Article 8(2) downstream due diligence) maps directly to the relationship categories that determine regulatory scope and obligation triggers. This is not merely semantic labeling; it determines which entities fall within mandatory due diligence perimeters.
- **EUDR geolocation support in `facility` nodes.** WGS 84 coordinates with GeoJSON polygon support directly addresses EUDR Article 9's requirement for plot-level geolocation, including the specification that plots over four hectares require polygon perimeters rather than single coordinate points.
- **Beneficial ownership modeling for AMLD 5/6.** The `person` node type linked via `beneficial_ownership` edges with `control_type` and `percentage` properties captures the UBO data required under the EU Anti-Money Laundering Directive, with the 25% threshold correctly identified as a tooling concern rather than a format constraint.
- **GDPR-compliant privacy defaults for person data.** SPEC-004's mandatory omission of person nodes from public files, combined with confidential default sensitivity, implements GDPR data minimization (Article 5(1)(c)) at the format level rather than leaving it to producer discretion.
- **Attestation node flexibility.** The `attestation` node type with enumerated `attestation_type` values (certification, audit, due_diligence_statement, self_declaration) and `outcome` status covers the full range of compliance documentation: SA8000 certifications, SMETA audits, EUDR due diligence statements, and LkSG risk assessments.
- **Cross-jurisdictional identifier model.** The composite identifier approach (SPEC-002) avoids the fatal trap of mandating a single identifier scheme. This is essential for cross-jurisdictional compliance: a German company reporting under LkSG uses Handelsregister numbers, while the same entity's US subsidiary under UFLPA may only carry an EIN. Both are representable without requiring LEI enrollment.

---

## Concerns

- **[Critical] No embedded emissions data model for CBAM.** CBAM entered its definitive (financial) phase on 1 January 2026. Importers must declare embedded emissions per installation and surrender certificates. SPEC-001's `consignment` node lacks fields for direct emissions, indirect emissions, emission factor source, or installation-specific production route. The `composed_of` edge notes CBAM relevance but provides no mechanism for carrying the actual emissions data. Without this, OMTS cannot serve as a data carrier for CBAM Article 7 declarations.

- **[Major] No temporal linkage between successive graph snapshots.** CSDDD (Article 11) and LkSG (Section 4(4)) require continuous monitoring, not one-time mapping. A supervisory authority reviewing compliance needs to see how a company's supply chain graph evolved over time: when a high-risk supplier was identified, what mitigation was taken, when it was re-assessed. The current `snapshot_date` captures a point in time, but there is no normative mechanism for versioning, delta encoding, or linking successive snapshots into an auditable timeline.

- **[Major] EUDR geolocation precision not specified.** SPEC-001 Section 4.2 defines `geo` as WGS 84 coordinates or GeoJSON geometry, but does not specify minimum coordinate precision. EUDR implementing guidance requires at least six decimal digits of latitude/longitude precision. Without a normative precision floor, producers may export coordinates rounded to two decimal places (approximately 1.1 km precision), which would be insufficient for EUDR plot verification.

- **[Major] No risk severity classification on attestation outcomes.** LkSG Section 5 and CSDDD Article 7 require prioritized risk analysis. The `attestation` node's `outcome` field (pass/conditional_pass/fail/pending/not_applicable) captures binary results but not the severity or likelihood dimensions that risk-based due diligence frameworks require. A conditional pass on a labor rights audit in a high-risk jurisdiction carries different regulatory weight than one in a low-risk jurisdiction, but the format cannot express this distinction.

- **[Minor] UFLPA entity list linkage absent.** The US Uyghur Forced Labor Prevention Act operates primarily through the UFLPA Entity List maintained by the Forced Labor Enforcement Task Force. SPEC-006 notes UFLPA coverage via jurisdiction and geo properties, but there is no mechanism for flagging entities that appear on the UFLPA Entity List or other sanctions/restricted party lists. This is a common compliance requirement for US importers.

- **[Minor] Conflict minerals regulation (EU 2017/821) not addressed in data model.** While the regulatory alignment table in SPEC-006 does not cover the EU Conflict Minerals Regulation, the `attested_by` edge with `scope: "conflict_minerals"` provides partial coverage. However, the regulation requires smelter/refiner identification and Responsible Minerals Initiative (RMI) conformant smelter list mapping, which is not represented.

---

## Recommendations

1. **[P0] Add embedded emissions properties to `consignment` or define a CBAM-specific extension.** At minimum, add optional fields for `direct_emissions_co2e` (number, tonnes CO2e), `indirect_emissions_co2e`, `emission_factor_source` (enum: `actual`, `default_eu`, `default_country`), and `installation_id` (reference to producing facility). CBAM's first surrender deadline is 30 September 2027; the format must carry this data before that date.

2. **[P0] Define a snapshot versioning mechanism.** Add optional `previous_snapshot_ref` (hash or URI of the prior snapshot) and `snapshot_sequence` fields to the file header. This enables regulatory auditors to reconstruct the temporal evolution of a supply chain graph. Consider a companion normative annex specifying how diff/delta between snapshots is computed.

3. **[P1] Specify minimum geolocation precision for EUDR compliance.** Add a validation rule (L2 level) that `geo` coordinates on `facility` nodes linked to EUDR-relevant commodities SHOULD have at least six decimal digits of precision, and that plots exceeding four hectares SHOULD use GeoJSON polygon geometry rather than a single point.

4. **[P1] Add risk classification properties to attestation nodes.** Extend the `attestation` node type with optional `risk_severity` (enum: `critical`, `high`, `medium`, `low`) and `risk_likelihood` (enum: `very_likely`, `likely`, `possible`, `unlikely`) fields. These enable risk-prioritized due diligence workflows as required by CSDDD Article 7 and LkSG Section 5.

5. **[P1] Add a restricted-party-list flag or extension mechanism.** Define an optional `regulatory_flags` array on `organization` nodes (or as a separate attestation subtype) for marking entities that appear on sanctions lists, UFLPA Entity Lists, or other restricted party databases. This is a cross-cutting compliance requirement.

6. **[P2] Track the CSDDD Omnibus I changes and update SPEC-006 accordingly.** The December 2025 Omnibus agreement raised CSDDD thresholds to 5,000 employees and EUR 1.5 billion turnover, postponed application to mid-2029, and deleted mandatory climate transition plans. SPEC-006's regulatory alignment table should reflect these revised scope parameters so implementers understand the current regulatory landscape.

7. **[P2] Monitor the EUDR simplification review.** The Commission must present its simplification report by 30 April 2026. If geolocation or due diligence statement requirements are materially amended, the data model may need to adapt.

---

## Cross-Expert Notes

- **Enterprise Integration Expert:** The CBAM emissions gap directly affects ERP integration. SAP S/4HANA's CBAM module (available since 2024) and Oracle's sustainability reporting tools already track installation-level embedded emissions. The ERP integration guide (SPEC-005) should include mappings from these CBAM-specific ERP data structures to OMTS.

- **Security & Privacy Expert:** The temporal audit trail recommendation (snapshot versioning) has integrity implications. A chain of snapshot references needs tamper-evident linking (e.g., each snapshot referencing the hash of its predecessor) to be credible as a regulatory audit trail. Without integrity, a company could retroactively alter earlier snapshots.

- **Standards Expert:** The ISO 6523 mapping in SPEC-006 should be extended to cover CBAM installation identifiers. CBAM uses installation-specific identifiers issued by competent authorities in exporting countries; these will need a scheme definition.

- **Graph Modeling Expert:** The temporal linkage recommendation has graph model implications. Successive snapshots form a meta-graph of graph versions. Consider whether this is modeled within the existing graph structure or as a separate metadata layer.

- **Data Format Expert:** The EUDR geolocation precision requirement should be expressed as a JSON Schema constraint (e.g., minimum string length or decimal precision for coordinate values) to enable automated validation.

---

*This review reflects the regulatory landscape as of 18 February 2026, including the CSDDD Omnibus I provisional agreement of December 2025, the revised EUDR (EU 2025/2650) published 23 December 2025, and CBAM's entry into its definitive phase on 1 January 2026.*
