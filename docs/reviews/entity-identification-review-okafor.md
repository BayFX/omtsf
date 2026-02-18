# Expert Review: OMTSF Entity Identification Specification (Revision 2)

**Reviewer:** Danielle Okafor, Open Source Strategy & Governance Lead
**Spec Reviewed:** OMTSF-SPEC-001 -- Entity Identification (Draft, Revision 2, 2026-02-17)
**Review Date:** 2026-02-18

---

## Assessment

Revision 2 directly addresses the three P0 findings I flagged in the initial panel review, and addresses them substantively rather than perfunctorily. The CC-BY-4.0 license header on the spec with Apache 2.0 noted for code (P0-12) resolves the licensing ambiguity that would have blocked any serious adopter's legal review. The scheme governance process in Section 4.3 (P0-11) -- TSC, lazy consensus, 30-day public review, explicit criteria for inclusion, and a deprecation pathway with a 90-day notice period -- is a well-structured governance model that draws on patterns I recognize from OASIS and W3C processes. The GLEIF RA list versioning in Section 4.4 (P0-10) -- quarterly versioned snapshots maintained in-repo with validator fallback to warning-not-rejection for unknown codes -- correctly decouples OMTSF validation from GLEIF's publication cadence.

This is materially better than Revision 1. The spec has moved from "no governance at all" to "credible governance scaffolding." That said, the governance model is still scaffolding, not a load-bearing structure. The TSC referenced in Section 4.3 does not exist yet. There is no charter, no membership criteria, no quorum definition, no conflict-of-interest policy, and no process for forming the TSC itself. This is not unusual for a specification at draft stage, but the scheme governance process normatively depends on a body that has no operating procedures. The risk is not that the process is wrong -- it is well-designed -- but that it is unexecutable until the organizational infrastructure catches up.

From an adoption strategy perspective, Revision 2 strengthens the spec considerably. The "minimum viable file" concept is implicitly validated by the decision (Section 16, resolved Open Question #2) to keep external identifiers at Level 2, which preserves the adoption ramp I advocated for. The enrichment lifecycle model in Section 14.2, while primarily a producer guidance concern, also serves adoption: it tells a mid-market company "you can start with just your ERP vendor codes and improve over time." That is the right message.

## Strengths

- **Licensing clarity.** CC-BY-4.0 for spec, Apache 2.0 for code is the correct and standard split. This removes the single largest barrier to enterprise adoption -- legal departments will not approve use of an unlicensed specification, regardless of technical merit.
- **Governance process design.** The three-gate model (written proposal with evidence, 30-day review, TSC approval via lazy consensus or majority vote) is lightweight enough to avoid bureaucratic ossification while formal enough to prevent unilateral changes. The deprecation pathway with a 90-day notice and two-version retention period protects existing implementations.
- **Inclusion criteria for core schemes.** Requiring a publicly available specification, no IP encumbrance on identifier values, demonstrated coverage (100,000+ entities or regulatory mandate), and an identifiable operational authority is a well-calibrated set of thresholds. These criteria would correctly admit schemes like EORI (EU customs) while correctly excluding proprietary internal numbering systems.
- **GLEIF RA list decoupling.** The versioned snapshot approach with quarterly updates and warning-not-rejection for unknown codes is operationally sound. The decision to exempt snapshot updates from TSC approval (standard PR workflow) avoids governance bottleneck on routine data maintenance.
- **Extension scheme namespace.** The reverse-domain notation for extension schemes (Section 4.2) is a proven pattern from Java packaging and DNS that avoids collision with future core schemes without requiring a central registry for extensions.

## Concerns

- **[High] TSC is referenced but undefined.** Section 4.3 normatively depends on a "OMTSF Technical Steering Committee (TSC)" for scheme approval and deprecation decisions. No TSC charter, formation process, membership criteria, or decision-making procedures exist anywhere in the repository. Until the TSC is constituted, the governance process is formally complete but practically unexecutable. Any scheme addition request received today would have no body to process it.

- **[High] No contributor process beyond scheme governance.** The spec now governs how identifier schemes are added, but the broader contribution model remains undefined. There is no CONTRIBUTING.md, no Developer Certificate of Origin (DCO) or Contributor License Agreement (CLA), and no process for contributing to the spec itself (as opposed to the scheme registry). An implementer who discovers an ambiguity in the merge semantics has no documented path to propose a fix. This is an adoption friction point: enterprises evaluating whether to invest engineering effort in OMTSF tooling need to see a contribution pathway before committing resources.

- **[Medium] No conformance test suite plan.** Revision 2 significantly expanded the validation rules (Section 11 now has 17 Level 1 rules, 8 Level 2 rules, and 7 Level 3 rules). Without a conformance test suite, each implementer will independently interpret these rules, leading to divergent validation behavior. A conformance test suite -- even a collection of valid and invalid `.omts` file fragments with expected validation outcomes -- is the most effective ecosystem enablement tool for a format specification. The spec should at minimum reference a planned test suite and its scope.

- **[Medium] Adoption complexity for small suppliers remains unaddressed.** While the Level 2 decision preserves the adoption ramp, the spec itself is now over 1,000 lines covering 16 sections. A small supplier asked to produce an `.omts` file faces a formidable document. The "minimum viable file" profile (P1-20 from the panel report) has not been created. A one-page "Quick Start: Producing Your First OMTSF File" guide that shows a 15-line file with one organization node, one internal identifier, and one supply edge would do more for adoption than any amount of governance refinement.

- **[Low] Extension scheme registry governance is implicit.** Section 4.2 defines the namespace convention for extension schemes but is silent on whether there is any registry of known extensions, any collision detection mechanism, or any community process for coordinating extension scheme codes. The four "known extension codes" in the table have no formal status. As adoption grows, the risk of two independent communities choosing overlapping reverse-domain prefixes is low but non-zero.

## Recommendations

1. **(P0) Draft a TSC charter.** Define membership criteria (e.g., active spec contributors, implementers, adopters), quorum, voting procedures, term limits, and the bootstrap process for the initial TSC. Without this, Section 4.3 is a dead letter. The charter does not need to be complex -- the Node.js TSC charter is a good lightweight model.

2. **(P1) Publish a CONTRIBUTING.md and adopt DCO.** Define how to contribute to the spec (not just the scheme registry): issue filing, pull request process, review expectations, and IP commitment. The Developer Certificate of Origin (DCO, developercertificate.org) is the lightest-weight IP mechanism and is compatible with Apache 2.0 and CC-BY-4.0.

3. **(P1) Publish a conformance test suite seed.** Start with 20-30 `.omts` file fragments covering the Level 1 validation rules (L1-ID-01 through L1-ID-17): one valid file per rule, one invalid file per rule, expected validation outcome. This is the minimum viable conformance suite. Expand to Level 2 and Level 3 over time.

4. **(P1) Create a "minimum viable file" quick-start guide.** A separate, short document showing the simplest possible valid file, the simplest file that passes Level 2, and the simplest enriched file. Target audience: a procurement analyst at a 200-person manufacturer who has never seen a graph data format.

5. **(P2) Establish a community extension scheme registry.** A YAML or CSV file in the repository where extension scheme authors can register their codes, preventing collision. No TSC approval required -- just a PR with the scheme code, a contact, and a one-line description. This is the lightest possible coordination mechanism.

## Cross-Expert Notes

- **For Dr. Nakamura (Standards):** The governance process in Section 4.3 aligns well with your recommendation for formal scheme registration. However, the TSC that would execute this process does not yet exist. I recommend we jointly draft a TSC charter that incorporates your ISO 6523 alignment concerns as a standing review criterion for new scheme proposals.

- **For Marcus Lindgren (Procurement):** The adoption complexity concern I raised is directly connected to your enrichment lifecycle model (Section 14.2). The progression from internal-only to enriched is well-described for implementers, but needs a companion document for the procurement professionals who will be the actual users. A quick-start guide co-authored with your perspective on what a CPO needs to see would be highly effective.

- **For Prof. Varga (Graph Modeling):** The scheme governance process (Section 4.3) handles vocabulary evolution but does not address evolution of the merge semantics or edge type taxonomy. If the formal algebraic properties of merge are changed in a future spec version, the governance implications are significant -- existing merged datasets may become inconsistent. The TSC charter should explicitly scope governance authority over merge semantics, not just the scheme registry.

- **For Dr. Tanaka (Security):** The GLEIF RA list versioning (Section 4.4) uses a standard PR workflow without TSC approval. This is correct for routine data updates, but the validator fallback behavior (warning-not-rejection for unknown codes) has security implications: a malicious file could use fabricated RA codes to bypass validation. The conformance test suite should include test cases for unknown RA code handling to ensure validators implement the fallback correctly.

- **For Dr. Kowalski (Data Format):** The scheme governance process and extension scheme namespace are well-defined textually, but the conformance test suite I am recommending would be the authoritative machine-readable expression of the validation rules. I would welcome your input on the test vector format, particularly for the boundary reference hash test vectors you advocated for in your review.
