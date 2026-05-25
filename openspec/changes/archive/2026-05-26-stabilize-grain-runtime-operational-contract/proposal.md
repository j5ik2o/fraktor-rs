## Why

`cluster-*` の主軸を Proto.Actor-Go 型 Virtual Actor / Grain runtime として再定義したため、直近の実装は Pekko parity ではなく Grain runtime の運用 contract を固定する必要がある。identity lookup、topology update、placement cache、join / leave / down の期待値が曖昧なまま failure detector、downing、rebalance に進むと、何を守るための機能なのかが不明確になる。

## What Changes

- Grain identity resolution の成功 / pending / no authority / cache hit の contract を仕様化する。
- topology update と member departure によって absent authority の activation / PID cache が無効化されることを仕様化する。
- join / leave / down / passivation と placement resolution の関係を contract test で固定する。
- rolling update 時に保証することと、rebalance / remembered entities へ先送りすることを明示する。
- Provider lifecycle、failure detector、downing は今回の contract の観測対象に留め、SBR や reachability matrix の本実装は含めない。

## Capabilities

### New Capabilities

- `cluster-grain-runtime-operational-contract`: Grain runtime の identity lookup、placement cache、topology update、member departure、passivation、rolling update 時の運用 contract を定義する。

### Modified Capabilities

- なし

## Impact

- `modules/cluster-core/src/identity/`
- `modules/cluster-core/src/placement/`
- `modules/cluster-core/src/grain/`
- `modules/cluster-core/src/membership/`
- `modules/cluster-core/tests/`
- `modules/cluster-adaptor-std/tests/`
- `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md`
