## Context

`docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` defines the current direction for `cluster-*`: it is a Virtual Actor / Grain runtime, not an Apache Pekko Cluster parity project. Recent changes have added provider-boundary, downing, and placement operational contracts, but top-level documentation and cluster gap analysis still contain enough Pekko parity framing that future work can be mis-scoped.

This change is documentation-only. It does not change Rust APIs, runtime behavior, provider implementations, or OpenSpec runtime contracts.

## Goals / Non-Goals

**Goals:**

- Make `README.md` and cluster docs describe `cluster-*` primarily as Grain runtime infrastructure.
- Keep Pekko references, but label them as comparison and operational design input.
- Point readers from the root README and cluster gap analysis to the cluster Grain runtime roadmap as the decision record for priority.
- Ensure deferred Pekko concepts are explicitly presented as out of current scope unless a future OpenSpec change adopts them.

**Non-Goals:**

- Do not implement or change cluster runtime behavior.
- Do not delete cluster gap-analysis evidence simply because it mentions Pekko.
- Do not claim Pekko public API parity for cluster.
- Do not reframe actor, stream, remote, or persistence documentation outside the cluster-specific scope.

## Decisions

1. Treat the roadmap as the authoritative priority document.

   `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` already captures the decision that `cluster-*` is Grain runtime first. README and gap analysis should link to it instead of restating a competing priority model.

2. Preserve `cluster-gap-analysis.md` as comparison, not implementation backlog.

   The file contains useful references to Pekko failure cases and missing concepts. Removing that information would lose context. Instead, the implementation pass should relabel the analysis so raw API gaps and singleton/sharding/distributed-data entries do not imply immediate priority.

3. Keep documentation language narrow and operational.

   The docs should emphasize identity lookup, placement, activation/passivation, topology updates, provider boundaries, failure observation, and downing decisions. Deferred items should be named plainly, not hidden.

4. Avoid code-adjacent churn.

   Because this is a documentation alignment change, validation should focus on OpenSpec validation and Markdown diff hygiene. Rust tests are not required unless implementation unexpectedly touches code.

## Risks / Trade-offs

- Existing gap-analysis tables may still be long and Pekko-heavy -> Add summary and priority framing near the top so readers do not interpret the whole table as a current backlog.
- Over-correcting could erase useful Pekko comparison context -> Preserve comparison sections and explicitly state that Pekko is a reference source.
- README may become too detailed -> Keep root README concise and link to the roadmap / gap analysis for detail.
- Documentation-only changes can drift again -> Add a spec requirement that future cluster docs must preserve the Grain runtime framing and roadmap link.
