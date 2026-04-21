## Why

step03 で `TestTickDriver` を `actor-adaptor-std` に引っ越した後も、`actor-core` の `test-support` feature 配下には責務 B の残りコンポーネント（`new_empty` 系コンストラクタ、テスト用の `MockActorRef`、各種 fixture を構築するヘルパ、その他 `#[cfg(any(test, feature = "test-support"))]` で公開されているダウンストリーム向け API）が残る。

これらを `actor-core` 本体に同居させ続ける限り、「本体 feature flag 経由で内部 API を露出する」アンチパターンが温存される。解決策は **専用の test helpers クレート `fraktor-actor-test-rs` を切り出す** こと。ダウンストリーム（統合テスト、showcase、fraktor-cluster/stream/remote/persistence の test 層）は `[dev-dependencies]` でこの crate を取り込む。

本 change は Strategy B の第 4 ステップ（責務 B-2）。`test-support` feature の体積を大幅に減らし、step05（責務 C 処理）と step06（feature 削除）の地ならしになる。

## What Changes

- 新規 crate `modules/actor-test/`（名前: `fraktor-actor-test-rs`、no_std 選択肢は design で確定。おそらく std 要）を作成
- `actor-core/test-support` feature 配下で公開していた以下を移設:
  - `new_empty` 系コンストラクタ（`ActorSystem::new_empty`、`ActorRef::new_empty` 等、design で全数列挙）
  - 各種テスト用 mock / fixture（`MockActorRef`、`TestProbe` 等、存在するもの）
  - 統合テストで共有されているヘルパ関数（具体は design の調査で確定）
- 移設先で API を整理（不要になったものは削除、責務 C と重複する内部 API promotion は step05 に委ねる）
- ダウンストリームの `Cargo.toml` を更新: `actor-core = { features = ["test-support"] }` → `fraktor-actor-test-rs`（`[dev-dependencies]`）
- **BREAKING（workspace-internal）**: 公開テストヘルパの crate path が変わる

**Non-Goals**:
- 責務 C（内部 API の `pub(crate)` → `pub` 格上げ）の処理は step05 で行う
- `test-support` feature の完全削除は step06 で行う
- `actor-core` 自身の `[[test]]` が依存する純粋な内部 helper（`#[cfg(test)]` のみで `feature = "test-support"` ゲートされていないもの）は移設対象外

## Capabilities

### New Capabilities
- なし（新規 crate の API surface そのものは capability spec 化しない想定。design で再検討し、必要なら `actor-test-helpers-api` のような capability を ADDED として導入）

### Modified Capabilities
- なし

OpenSpec validation 要件を満たすため、design / specs フェーズで最低 1 件の delta を設計する。候補:
- 案 A: 新規 capability `actor-test-helpers-placement` を ADDED し、「ダウンストリーム向けテストヘルパは専用 crate に置く」ルールを明文化
- 案 B: `actor-lock-construction-governance` に Scenario を追加（test-support feature 経由で内部 API を露出しない原則として）

## Impact

- **Affected code**:
  - 新規: `modules/actor-test/src/**`、`modules/actor-test/Cargo.toml`
  - 既存: `modules/actor-core/src/` の `#[cfg(any(test, feature = "test-support"))]` ゲートの一部削除
  - ダウンストリーム: `modules/cluster-*`、`modules/remote-*`、`modules/stream-*`、`modules/persistence-*`、`showcases/std` の `[dev-dependencies]`
- **Affected APIs**: テストヘルパの crate path 変更（workspace-internal breaking）
- **Affected dependencies**: 新規 crate が `fraktor-actor-core-rs` に依存（逆方向の循環は発生しない）
- **Release impact**: pre-release phase につき外部影響は軽微。ただし新規 crate publish の検討が必要（docs.rs 向け、または publish = false で workspace-internal のみ）
