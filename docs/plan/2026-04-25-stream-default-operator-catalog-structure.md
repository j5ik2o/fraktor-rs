# Stream DefaultOperatorCatalog 構造分割計画

## 目的

`DefaultOperatorCatalog` に集中している operator contract / coverage をカテゴリ別 internal module へ移動し、catalog 本体を dispatcher として薄く保つ。

## 対象

- `modules/stream-core/src/core/impl/default_operator_catalog.rs`
- `modules/stream-core/src/core/impl/default_operator_catalog_*.rs`
- `modules/stream-core/src/core/impl/default_operator_catalog/tests.rs`
- `docs/gap-analysis/stream-gap-analysis.md`

## 実装方針

1. `default_operator_catalog.rs` は `DefaultOperatorCatalog`、`new`、`Default`、`OperatorCatalog` 実装に絞る。
2. `module-wiring` の no-parent-reexport を避けるため、`default_operator_catalog.rs` は leaf module のまま維持し、`default_operator_catalog_source`、`default_operator_catalog_transform`、`default_operator_catalog_substream`、`default_operator_catalog_timing`、`default_operator_catalog_fan_in`、`default_operator_catalog_fan_out`、`default_operator_catalog_failure`、`default_operator_catalog_hub`、`default_operator_catalog_kill_switch` を sibling internal module として追加する。
3. 各カテゴリ module は `lookup(key) -> Option<OperatorContract>` と `coverage() -> &'static [OperatorCoverage]` を持つ internal function とする。
4. 既存の contract 文言、requirement id、unsupported operator error は変更せず、移動のみで意味を維持する。
5. `DefaultOperatorCatalog::coverage()` はカテゴリ coverage を結合した静的配列を返す。
6. 既存テストと追加済みカテゴリ分割テストで、網羅性、重複なし、lookup と coverage の requirement id 一致を確認する。

## スコープ外

- StreamRef remote integration
- TCP / TLS stream API
- public DSL API の追加
- `GraphInterpreter` drive state machine の追加分割

## 検証

- `rtk cargo clippy -p fraktor-stream-core-rs -- -D warnings`
- `rtk cargo test -p fraktor-stream-core-rs`
- `rtk ./scripts/ci-check.sh ai dylint`
