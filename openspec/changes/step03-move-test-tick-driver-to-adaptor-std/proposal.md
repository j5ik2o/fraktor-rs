## Why

`actor-core` の `test-support` feature が抱える「責務 B（ダウンストリーム統合テスト用 API 公開）」のうち最大のコンポーネントは `TestTickDriver` である。`actor-core` は no_std クレートであるため、`std::thread` / `std::time` を使う `TestTickDriver` は本質的に std 環境専用。これを `actor-core` 本体で提供していること自体が責務の漏洩。

`actor-adaptor-std` は既に std 環境専用のアダプタクレートとして存在し、`tokio-executor` feature / `test-support` feature を持ち、std 向けの周辺機能を集約する場所として位置づけられている。`TestTickDriver` はこちらに引っ越すのが構造的に自然。

本 change は `test-support` feature を最終的に退役する長期計画（Strategy B）の第 3 ステップ。責務 A（`critical-section/std` impl provider）は既に退役済み（PR #1607/#1608）、責務 B-1 として `TestTickDriver` 移設を行う。

## What Changes

- `modules/actor-core/src/` 配下で `TestTickDriver` 関連の実装（構造体定義、テストユーティリティ、`#[cfg(any(test, feature = "test-support"))]` ゲート内の公開シンボル）を抽出
- `modules/actor-adaptor-std/src/test_support/` 配下（または同等の場所、design で最終決定）に移設
- `actor-core` 側では no_std セーフな `TickDriver` trait のみを残す（既存実装を継続）
- ダウンストリーム（`showcases/std`、`actor-core` 自身の `[[test]]`、他 crate の test）の import path を更新
- `actor-core/test-support` feature からは `TestTickDriver` 関連が消えるため、残責務 B-2、C の範囲が縮小
- **BREAKING（workspace-internal）**: `fraktor_actor_core_rs::...::TestTickDriver` → `fraktor_actor_adaptor_std_rs::...::TestTickDriver` へのパス変更

**Non-Goals**:
- `TestTickDriver` 以外の test-support 公開 API（`new_empty` 等、responsibility B-2）の移設は step04 で行う
- `test-support` feature 自体の削除は step06 で行う
- `actor-adaptor-std` の既存 `test-support` feature 設計見直し（現状の構造を尊重）

## Capabilities

### New Capabilities
- なし

### Modified Capabilities
- なし（現時点では `TestTickDriver` の配置は spec 化されていない）

design / specs フェーズで以下のいずれかを判断:
- 案 A: 新規 capability `actor-test-driver-placement` を ADDED し、「std 依存のテストドライバは actor-adaptor-std 側に置く」ルールを明文化
- 案 B: 既存 `actor-lock-construction-governance` 等の spec に Scenario を追加（test-support 責務分離の原則として）
- 案 C: 本 change は実装のみ、spec 化は別 change に切り分け（OpenSpec validation 要件のため何らか最低限の delta が必要）

## Impact

- **Affected code**:
  - `modules/actor-core/src/`（`TestTickDriver` 定義・エクスポート削除）
  - `modules/actor-adaptor-std/src/`（新規受け入れ先、Cargo.toml の `test-support` feature 再定義）
  - `modules/actor-core/tests/*.rs`（import path 更新、該当があれば）
  - `showcases/std/*/main.rs`（import path 更新）
- **Affected APIs**: `TestTickDriver` のクレートパス変更（workspace-internal breaking）
- **Affected dependencies**: `actor-adaptor-std` が `actor-core` test-only API に依存しなくなる（現状の循環的な印象を解消）
- **Release impact**: pre-release phase につき外部影響は軽微
