## 1. Current documentation audit

- [x] 1.1 Review `README.md` cluster references and identify wording that implies broad Pekko Cluster parity.
- [x] 1.2 Review `docs/gap-analysis/cluster-gap-analysis.md` for sections that read as direct implementation backlog instead of comparison context.
- [x] 1.3 Confirm `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` remains the authoritative priority document and does not need scope expansion.

## 2. Documentation alignment

- [x] 2.1 Update `README.md` cluster overview and links so `cluster-*` is introduced as Grain runtime infrastructure.
- [x] 2.2 Update `docs/gap-analysis/cluster-gap-analysis.md` summary and priority framing so Pekko is clearly a reference for operational concerns.
- [x] 2.3 Make deferred Pekko concepts explicit before detailed gap tables.
- [x] 2.4 Add or adjust links between README, cluster gap analysis, and the Grain runtime roadmap.

## 3. Scope guard

- [x] 3.1 Ensure the change does not modify Rust source, runtime behavior, provider implementations, or dependencies.
- [x] 3.2 Ensure no documentation claims Pekko Cluster / Cluster Sharding public API parity as the current roadmap.
- [x] 3.3 Preserve useful Pekko comparison evidence instead of deleting it wholesale.

## 4. Validation

- [x] 4.1 Run `openspec validate align-cluster-grain-runtime-docs --strict`.
- [x] 4.2 Run `git diff --check`.
- [x] 4.3 Review the final docs diff for conflicting priority statements.
