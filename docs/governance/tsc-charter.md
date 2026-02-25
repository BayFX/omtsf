# OMTS Technical Steering Committee (TSC) Charter

**Status:** Draft
**Date:** 2026-02-18
**Addresses:** R2-C2, R2-P0-2

---

## 1. Purpose

The Technical Steering Committee (TSC) governs the technical direction of the OMTS project, including normative specifications, controlled vocabularies, validation rules, and merge semantics.

## 2. Scope of Authority

The TSC has decision-making authority over:

- **Identifier scheme vocabulary** (OMTS-SPEC-002, Section 5.3): additions, promotions, and deprecations of core schemes
- **Node and edge type registries** (OMTS-SPEC-001): additions of core node and edge types
- **Merge semantics** (OMTS-SPEC-003): changes to identity predicates, algebraic properties, or transitive closure behavior (stability-critical — requires major version increment)
- **Validation rules**: additions, modifications, or removals of L1/L2/L3 rules across all specs
- **GLEIF RA list snapshots** (OMTS-SPEC-002, Section 5.4): snapshot updates follow the standard PR workflow and do NOT require TSC approval
- **Specification versioning**: major and minor version number assignments

## 3. Membership

### 3.1 Composition

The TSC consists of 5--9 members representing a balance of:
- Specification authors and maintainers
- Implementors (tooling developers, validator authors)
- End users (companies producing or consuming `.omts` files)
- Domain experts (supply chain, regulatory, identity)

### 3.2 Selection

- Initial TSC members are appointed by the project founders during the bootstrap period (Section 7).
- Subsequent members are elected by existing TSC members via simple majority vote.
- Members serve 2-year terms, renewable without limit.
- Members may resign at any time by written notice to the TSC.

### 3.3 Removal

A TSC member may be removed for sustained inactivity (missing 3 consecutive meetings without notice) by majority vote of remaining members.

## 4. Decision-Making

### 4.1 Lazy Consensus

The default decision-making process is **lazy consensus**: a proposal is approved if no TSC member objects within the review period.

- Standard review period: **30 days** for scheme additions, edge/node type additions, and validation rule changes.
- Extended review period: **90 days** for scheme deprecations and changes to merge semantics (Section 9 of OMTS-SPEC-003).
- The review period begins when the proposal PR is opened and the TSC mailing list is notified.

### 4.2 Contested Decisions

If any TSC member objects during the review period:
1. The proposer and objector(s) attempt to resolve the objection through discussion on the PR.
2. If unresolved after 14 days, the TSC Chair calls a vote.
3. Approval requires a **simple majority** of all TSC members (not just those voting).

### 4.3 Quorum

A vote requires participation of at least **50% of TSC members** (rounded up) to be valid.

### 4.4 Chair

The TSC elects a Chair from among its members by simple majority. The Chair:
- Calls votes when lazy consensus fails
- Sets meeting agendas
- Serves as tiebreaker in the event of a tied vote
- Serves a 1-year term, renewable

## 5. Meetings

- The TSC meets at least quarterly, with additional meetings as needed.
- Meetings may be held via video conference or asynchronous discussion on the project's communication channels.
- Meeting minutes are published in the repository within 7 days.

## 6. Conflict of Interest

TSC members with a material conflict of interest on a specific proposal (e.g., employed by the issuing authority of an identifier scheme under consideration) MUST disclose the conflict and MAY recuse themselves from the vote. Failure to disclose a known conflict is grounds for removal.

## 7. Bootstrap Process

Until the TSC is formally constituted:

1. The project founders act as an interim TSC with full authority.
2. The interim TSC MUST constitute a permanent TSC within **6 months** of the first stable release (v1.0.0) of any normative specification.
3. During the interim period, all decisions follow the lazy consensus process with a 14-day review period (shortened from the standard 30 days to enable rapid iteration on draft specifications).
4. Decisions made during the bootstrap period remain valid after the permanent TSC is constituted unless explicitly revisited.

## 8. Intellectual Property Policy

### 8.1 Contribution Licensing

All contributions to the OMTS project are made under the **Developer Certificate of Origin (DCO)** v1.1. Contributors certify, by adding a `Signed-off-by` trailer to each commit, that they have the right to submit the contribution under the project's licenses:

- **Specifications** (`spec/`): Licensed under CC-BY-4.0.
- **Code, schemas, and tooling**: Licensed under Apache License 2.0.

No Contributor License Agreement (CLA) is required. The DCO is sufficient for this project's current stage and is the mechanism used by the Linux kernel, Git, and other major open source projects.

### 8.2 Patent Grant

Each Contributor, by submitting a Contribution to the project, hereby grants to all recipients of the Work a perpetual, worldwide, non-exclusive, no-charge, royalty-free, irrevocable patent license to make, have made, use, offer to sell, sell, import, and otherwise transfer the Work, where such license applies only to those patent claims licensable by such Contributor that are necessarily infringed by their Contribution(s) alone or by combination of their Contribution(s) with the Work to which such Contribution(s) was submitted.

This grant is consistent with the Apache License 2.0 patent grant (Section 3) and ensures that the OMTS specification and its implementations remain freely implementable without patent risk from contributors.

### 8.3 No CLA Required

The project does not require a Contributor License Agreement. The DCO sign-off combined with the Apache 2.0 patent grant provides sufficient IP protection for an open specification project at this stage. If the project moves to a standards body (e.g., OASIS, Linux Foundation), the IP policy may be updated to align with that body's requirements.

## 9. Amendments

This charter may be amended by a **two-thirds majority** vote of all TSC members, with a 30-day review period.
