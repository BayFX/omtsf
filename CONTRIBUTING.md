# Contributing to OMTS

Thank you for your interest in contributing to the Open Multi-Tier Supply-chain Format.

## Developer Certificate of Origin (DCO)

All contributions to this project MUST be signed off under the [Developer Certificate of Origin](https://developercertificate.org/) (DCO v1.1). By signing off, you certify that you have the right to submit the contribution under the project's licenses.

Add a `Signed-off-by` trailer to every commit:

```
Signed-off-by: Your Name <your.email@example.com>
```

You can do this automatically with `git commit -s`.

## Contribution Types

### Specification Changes (Normative)

Normative changes to specifications in `spec/` follow a formal review process:

1. Open an issue describing the proposed change and its rationale.
2. Discuss the proposal in the issue. For non-trivial changes, the TSC may request a 30-day public review period (see the [TSC Charter](docs/governance/tsc-charter.md)).
3. Submit a pull request referencing the issue. The PR must include:
   - The spec text change itself.
   - Updates to the JSON Schema (`schema/`) if the file format changes.
   - Updates to test fixtures (`tests/fixtures/`) if validation rules change.
4. At least one TSC member must approve the PR.
5. Lazy consensus applies: if no TSC member objects within the review period, the PR is merged.

### Code Changes

Changes to tooling, validators, schema files, and test fixtures follow a lighter process:

1. Submit a pull request with a clear description of the change.
2. Ensure all existing tests continue to pass.
3. One approving review is required for merge.

### Documentation and Editorial

Typo fixes, clarifications, and documentation improvements can be submitted as pull requests without a prior issue. These do not require TSC review.

## Code of Conduct

Contributors are expected to be respectful and constructive in all project interactions. Harassment, discrimination, and personal attacks are not tolerated. The project maintainers reserve the right to remove contributions and ban contributors who violate these expectations.

## Licensing

- Specifications in `spec/` are licensed under [CC-BY-4.0](spec/LICENSE).
- Code, schemas, and tooling are licensed under [Apache-2.0](LICENSE).
- By submitting a contribution, you agree that your contribution will be licensed under the applicable license for the files you modify or create.

## Getting Started

1. Fork the repository and create a feature branch.
2. Make your changes, ensuring each commit is signed off (`git commit -s`).
3. Validate your changes against the JSON Schema if modifying spec content.
4. Submit a pull request against `main`.

## Questions?

Open an issue with the `question` label for any questions about contributing.
