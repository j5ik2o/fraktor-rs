## Why

`ActorSharedFactory` は dispatcher、executor、event stream、actor-cell runtime state まで一括で生成しており、変更理由の異なる生成責務を 1 trait に押し込めた God Factory になっている。これにより、特定の shared wrapper だけを差し替えたい場面でも無関係な factory 契約まで同時に実装・注入する必要があり、責務境界とテスト境界の両方が曖昧になっている。

## What Changes

- `ActorSharedFactory` を廃止し、shared wrapper / shared state ごとに責務を分離した個別 factory trait を導入する
- dispatcher / executor / actor-ref sender / event stream / actor-cell runtime state / mailbox bundle など、変更理由の異なる生成責務を独立 Port として定義する
- actor-core / actor-adaptor-std / cluster-core の wiring を、単一 God Factory 依存から必要な個別 Port 依存へ置き換える
- `shared_factory` 命名へ揃えた公開 API と module 名を、個別 Port 前提の形へ再整理する
- **BREAKING** `ActorSharedFactory` を削除し、それに依存する wiring / test double / override を個別 factory trait へ置き換える

## Capabilities

### New Capabilities
- `actor-shared-factory-ports`: actor runtime の shared wrapper / shared state 構築を 1責務1Port の factory trait 群として定義する

### Modified Capabilities
- `dispatcher-trait-provider-abstraction`: dispatcher / executor / shared queue の構築境界を単一 factory ではなく個別 Port に変更する
- `actor-system-default-config`: actor system default config が単一 `ActorSharedFactory` ではなく、必要な個別 factory trait を使う wiring へ変更する

## Impact

- 対象コード:
  - `modules/actor-core/src/core/kernel/system/shared_factory/`
  - `modules/actor-core/src/core/kernel/dispatch/`
  - `modules/actor-core/src/core/kernel/actor/`
  - `modules/actor-core/src/core/kernel/event/stream/`
  - `modules/actor-adaptor-std/src/std/system/shared_factory/`
  - `modules/cluster-core/src/core/`
- 影響内容:
  - 単一 `ActorSharedFactory` に依存している wiring を個別 Port へ置換する
  - test double が個別 Port 単位へ分割され、必要最小限の差し替えだけを実装できるようになる
- 非目標:
  - generic shared (`ActorFutureShared<T>` など) を一律に system-scoped factory へ含めること
  - actor-* 以外の crate へこの分割を広げること
  - 今回の change で implementation detail まで完全に最適化し切ること
