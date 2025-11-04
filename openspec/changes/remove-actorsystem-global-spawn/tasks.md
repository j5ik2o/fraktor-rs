## 実装タスクリスト

### フェーズ1: API削除

#### 準備
- [ ] 既存コードで `ActorSystem::{spawn,spawn_child,actor_ref,children,stop_actor}` の使用箇所を全て洗い出し
- [ ] 各使用箇所を ActorContext 経由のパターンに書き換える計画を立てる

#### API変更
- [ ] `modules/actor-core/src/system/base.rs` - `ActorSystemGeneric` の該当メソッドを `pub(crate)` に変更
  - [ ] `spawn` を `pub(crate)` に
  - [ ] `spawn_child` を `pub(crate)` に
  - [ ] `actor_ref` を `pub(crate)` に
  - [ ] `children` を `pub(crate)` に
  - [ ] `stop_actor` を `pub(crate)` に
- [ ] `modules/actor-std/src/system/base.rs` - 対応するメソッドを削除
- [ ] コンパイルエラーになる箇所を全て特定

#### テスト移行
- [ ] `modules/actor-core/tests` を ActorContext パターンに書き換え
  - [ ] `system_lifecycle.rs`
  - [ ] `event_stream.rs`
  - [ ] `ping_pong.rs`
  - [ ] `supervisor.rs`
  - [ ] `system_events.rs`
  - [ ] その他のテストファイル
- [ ] `modules/actor-std/tests` を ActorContext パターンに書き換え
  - [ ] `tokio_acceptance.rs`
  - [ ] その他のstdテスト

#### サンプル移行
- [ ] `modules/actor-std/examples` を ActorContext パターンに書き換え
  - [ ] `ping_pong_tokio`
  - [ ] `deadletter_std`
  - [ ] `named_actor_std`
  - [ ] その他のサンプル

### フェーズ2: ドキュメント整備

- [ ] CHANGELOG に BREAKING CHANGE を追加
  - [ ] 削除されたAPIのリスト
  - [ ] 移行ガイド（ActorContext経由のパターン）
  - [ ] 推奨パターンの例
- [ ] README のサンプルコードを更新
- [ ] API ドキュメントを更新

### フェーズ3: 検証

- [ ] 全テストスイートが成功することを確認
- [ ] `makers ci-check` が全てパス
- [ ] カバレッジが維持されていることを確認
- [ ] `openspec validate remove-actorsystem-global-spawn --strict` が成功

### オプション: テスト用ヘルパー

- [ ] `#[cfg(test)]` でのPIDベースAPI公開を検討
- [ ] テスト用ガーディアンパターンのテンプレート提供を検討
