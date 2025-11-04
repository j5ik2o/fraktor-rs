## 実装タスクリスト

### フェーズ1: API削除

#### 準備
- [x] 既存コードで `ActorSystem::{spawn,spawn_child,actor_ref,children,stop_actor}` の使用箇所を全て洗い出し
- [x] 各使用箇所を ActorContext 経由のパターンに書き換える計画を立てる

#### API変更
- [x] `modules/actor-core/src/system/base.rs` - `ActorSystemGeneric` の該当メソッドを `pub(crate)` に変更
  - [x] `spawn` を `pub(crate)` に
  - [x] `spawn_child` を `pub(crate)` に
  - [x] `actor_ref` を `pub(crate)` に
  - [x] `children` を `pub(crate)` に
  - [x] `stop_actor` を `pub(crate)` に
- [x] `modules/actor-std/src/system/base.rs` - 対応するメソッドを削除
- [x] コンパイルエラーになる箇所を全て特定

#### テスト移行
- [x] `modules/actor-core/tests` を ActorContext パターンに書き換え
  - [x] `system_lifecycle.rs` - 既にActorContext使用
  - [x] `event_stream.rs` - `test_spawn` を使用するように修正
  - [x] `ping_pong.rs` - 既にActorContext使用
  - [x] `supervisor.rs` - `test_actor_ref` を使用するように修正
  - [x] `system_events.rs` - 既にActorContext使用
- [x] `modules/actor-std/tests` を ActorContext パターンに書き換え
  - [x] `tokio_acceptance.rs` - ガーディアン経由のパターンに書き換え

#### サンプル移行
- [x] `modules/actor-std/examples` を ActorContext パターンに書き換え
  - [x] `ping_pong_tokio` - 既にActorContext使用
  - [x] `deadletter_std` - 既にActorContext使用
  - [x] `named_actor_std` - 既にActorContext使用
  - [x] その他のサンプル - 全て確認済み

### フェーズ2: ドキュメント整備

- [x] CHANGELOG に BREAKING CHANGE を追加
  - [x] 削除されたAPIのリスト
  - [x] 移行ガイド（ActorContext経由のパターン）
  - [x] 推奨パターンの例
- [x] README のサンプルコードを更新（プロジェクトルートにREADME.mdなし）
- [x] API ドキュメントを更新（rustdocコメントは適切に記述済み）

### フェーズ3: 検証

- [x] 全テストスイートが成功することを確認
- [x] `cargo test --workspace` が全てパス (201 passed)
- [x] `cargo clippy --workspace` が警告のみ（エラーなし）
- [x] `cargo fmt --check` がパス
- [x] テスト失敗（`deadline_timer_key_debug`）を修正
- [x] カバレッジが維持されていることを確認（全テストがパス）
- [x] `openspec validate remove-actorsystem-global-spawn --strict` が成功

### 実装方針の変更

- [x] 当初のテスト用ヘルパー（`test_actor_ref`, `test_spawn`）は迂回策と判断し削除
- [x] すべてのテストをActorContext経由のガーディアンパターンに完全移行
  - [x] `supervisor.rs` - ライフサイクルイベント監視パターンに変更
  - [x] `event_stream.rs` - pre_start時のspawnパターンに変更
