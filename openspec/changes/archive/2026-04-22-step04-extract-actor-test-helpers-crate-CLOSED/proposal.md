> **CLOSED (2026-04-22, 未実装)**: 調査の結果、本 change のスコープが実質的に消滅していることが判明したため、実装せずに close する。
>
> **判断根拠**:
> - proposal で想定されていた移設対象 (`MockActorRef`、`TestProbe` 等) は workspace 内に **実在しない** (`grep` で確認、0 件)
> - `feature = "test-support"` で公開されている responsibility B-2 残（`ActorRef::new_with_builtin_lock`、`SchedulerRunner::manual`、`state::booting_state`/`running_state` モジュール宣言）の **caller はすべて actor-core 内部の inline test のみ**。外部 crate (cluster/stream/persistence/showcases) からの参照は 0 件
> - したがって専用 crate `fraktor-actor-test-rs` を新設する必要がなく、これらの要素は単に `pub(crate)` 化（or `#[cfg(test)]` 化）するだけで足りる。これは責務 C と本質的に同じ問題なので step05 で統合処理する
>
> **後続**: step05 (`step05-hide-actor-core-internal-test-api`) の proposal を更新し、責務 B-2 残 + 責務 C を統合スコープとして取り扱うように再定義済み。本 change ディレクトリは記録目的でアーカイブする。

## Why

step03 で `TestTickDriver` と `new_empty*` を `actor-adaptor-std` に引っ越した後も、`actor-core` の `test-support` feature 配下には責務 B の残りコンポーネント（テスト用の `MockActorRef`、各種 fixture を構築するヘルパ、その他 `#[cfg(any(test, feature = "test-support"))]` で公開されているダウンストリーム向け API）が残る。

> **step03 からの引き継ぎ**: `new_empty*` は step03 で `actor-adaptor-std::std::system::{new_empty_actor_system,_with,_typed}` 自由関数として移設済み。本 change の対象から除外する。
>
> **dev-cycle 制約（重要）**: step03 実装で判明した通り、`actor-core` の `[dev-dependencies]` 経由で actor-test crate を参照しても、**inline test では同一クレートが二バージョンとして compiler に見え型不一致になる**（Cargo の根本的仕様、回避不能）。このため、`actor-core` 自身の inline test で必要なテストヘルパは:
> - **A 案**: `actor-core` 内部に `pub(crate)` 限定の `#[cfg(test)]` 専用版を残す（step03 の TestTickDriver / new_empty\* と同じ二段構成）
> - **B 案**: 該当 inline test を統合テスト（`tests/*.rs`）へ移行し、外部 crate と同様に actor-test を `[dev-dependencies]` 経由で利用
>
> いずれを採るかは design で確定する。step03 では A 案を採用したが、対象が増えるとメンテ負荷が上がるため、step04 では B 案も検討候補に含める。

これらを `actor-core` 本体に同居させ続ける限り、「本体 feature flag 経由で内部 API を露出する」アンチパターンが温存される。解決策は **専用の test helpers クレート `fraktor-actor-test-rs` を切り出す** こと。ダウンストリーム（統合テスト、showcase、fraktor-cluster/stream/remote/persistence の test 層）は `[dev-dependencies]` でこの crate を取り込む。

本 change は Strategy B の第 4 ステップ（責務 B-2）。`test-support` feature の体積を大幅に減らし、step05（責務 C 処理）と step06（feature 削除）の地ならしになる。

## What Changes

- 新規 crate `modules/actor-test/`（名前: `fraktor-actor-test-rs`、no_std 選択肢は design で確定。おそらく std 要）を作成
- `actor-core/test-support` feature 配下で公開していた以下を移設:
  - 各種テスト用 mock / fixture（`MockActorRef`、`TestProbe` 等、存在するもの）
  - 統合テストで共有されているヘルパ関数（具体は design の調査で確定）
  - （`new_empty*` および `TestTickDriver` は step03 で `actor-adaptor-std` 側に移設済みのため対象外）
- 移設先で API を整理（不要になったものは削除、責務 C と重複する内部 API promotion は step05 に委ねる）
- ダウンストリームの `Cargo.toml` を更新: `actor-core = { features = ["test-support"] }` → `fraktor-actor-test-rs`（`[dev-dependencies]`）
- **BREAKING（workspace-internal）**: 公開テストヘルパの crate path が変わる

**Non-Goals**:
- 責務 C（内部 API の `pub(crate)` → `pub` 格上げ）の処理は step05 で行う
- `test-support` feature の完全削除は step06 で行う
- `actor-core` 自身の `[[test]]` が依存する純粋な内部 helper（`#[cfg(test)]` のみで `feature = "test-support"` ゲートされていないもの）は移設対象外
- `new_empty*` / `TestTickDriver` の再移設は不要（step03 で `actor-adaptor-std` に移設済み）。step04 は actor-test crate 新設を行うが、これらの公開 API を adaptor-std から actor-test に再移管するかは別問題（design で判断、デフォルトは現状維持）

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
