## Context

`cluster-grain-runtime-operational-contract` は identity resolution、topology invalidation、passivation、rolling update の最小 contract を持つ。
一方で `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` の Task slice 5 は、rebalance 実装前に Rendezvous hashing のまま伸ばす範囲と、join / leave / rolling update 時の movement と cache invalidation の期待値を固定することを求めている。

現行実装は `RendezvousHasher`、`PartitionIdentityLookup`、`PlacementCoordinatorCore` を中心に、authority topology から owner を選び、activation / PID cache を再利用する。
この change は新しい placement algorithm を導入せず、現行 algorithm の contract を明確化し、将来の rebalance / remembered entities 実装の境界を残す。

## Goals / Non-Goals

**Goals:**

- Rendezvous hashing が同一 topology / same key で deterministic placement を返すことを contract test として強める。
- node join が既存 active activation を即時移動しないことを固定する。
- node leave / down が departed authority の activation / PID cache を invalidation することを固定する。
- rolling update 時の期待値を「stale placement prevention と latest topology re-resolution」に限定する。

**Non-Goals:**

- least-shard rebalance、minimum movement guarantee、coordinator protocol は実装しない。
- remembered entities、activation recovery、persistence integration は実装しない。
- in-flight request drain や graceful handoff protocol は実装しない。
- Pekko Cluster Sharding public API parity は扱わない。

## Decisions

1. Placement scalability は algorithm 変更ではなく contract hardening として扱う。

   現時点の主軸は Grain runtime であり、Rendezvous hashing は coordinator-less な placement と no_std core に適している。rebalance algorithm を先に入れると、remembered entities や draining と境界が混ざるため、本 change では既存 algorithm の保証範囲を固定する。

2. Join は existing activation movement を発生させない。

   新 node が join した瞬間に既存 activation を移すと、minimum movement、state handoff、in-flight drain の要件が発生する。これらは Task slice 5 の deferred scope に残し、本 change では新規 resolution だけが latest topology を使うことを明確にする。

3. Leave / down は stale authority invalidation のみを強制する。

   departed authority を参照する activation / PID cache は再利用してはならない。これは provider / downing の種類に依存しない Grain runtime contract であり、rebalance なしでも安全性に直結する。

## Risks / Trade-offs

- Join 後に既存 activation が偏る可能性がある → 本 change では許容し、rebalance capability で扱う。
- Rolling update 中の request drain は保証されない → spec に non-goal として明記し、運用上は leave / down 後の re-resolution だけを保証する。
- Tests が現行 implementation detail に寄りすぎる可能性がある → public-ish contract surface (`IdentityLookup`, topology update, cache events) を中心に検証する。
