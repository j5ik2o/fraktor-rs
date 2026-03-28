## Why

`modules/actor/src/core` は現在、責務軸のモジュール群の中に `typed` だけが型付け軸として混在しており、最上位の分類軸が揃っていない。この状態では `references/pekko/actor` と `references/pekko/actor-typed` の責務境界に対応付けにくく、`core/typed` 直下も receptionist、pubsub、routing の語彙がフラットに露出している。

Pekko を参照実装として使い続けるためには、`modules/actor` の package 構造そのものを Pekko 由来の責務境界へ寄せる必要がある。正式リリース前のため、公開 import path の破壊的変更を許容してでも、この段階で構造を整える価値がある。

## What Changes

- `modules/actor/src/core` の最上位分類を `kernel` と `typed` に再編し、責務軸と型付け軸を分離する
- `core/typed` 直下の receptionist、pubsub、routing 関連の型を Pekko に寄せた package に再配置する
- `receptionist_command`、`service_key`、`listing` を `core/typed/receptionist/` に集約する
- `topic`、`topic_command`、`topic_stats` を `core/typed/pubsub/` に集約する
- `routers`、`resizer`、`*_router_builder` を `core/typed/routing/` に集約する
- `core/typed` 直下には typed primitive と typed runtime の基盤型だけを残し、発見・pubsub・routing の語彙を root から外す
- **BREAKING** `crate::core::typed::*` および関連 import path の一部を新しい package 経由へ変更する
- 実装時は file move / mod wiring ごとに `./scripts/ci-check.sh ai dylint` を実行し、最後に `./scripts/ci-check.sh ai all` で全体確認する

## Capabilities

### New Capabilities
- `actor-package-structure`: actor モジュールの package 構造を Pekko 対応の責務境界へ再編する

### Modified Capabilities

## Impact

- 影響対象コード: `modules/actor/src/core.rs`、`modules/actor/src/core/**`、`modules/actor/src/std/**`、関連 tests/examples
- 影響対象 API: `crate::core::typed` 配下の公開 import path、re-export 構成、module path
- 依存関係への影響: 依存 crate の追加は不要。`mod` 配線、`use` 文、test import の更新が中心
- 検証への影響: 構造変更のたびに `./scripts/ci-check.sh ai dylint` を実行し、最終的に `./scripts/ci-check.sh ai all` が必要
