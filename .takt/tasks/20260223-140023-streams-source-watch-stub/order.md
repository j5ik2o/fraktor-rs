# Source::watch() stub 実装

## 目的

`modules/streams/src/core/stage/source.rs` の `Source::watch()` メソッドが `self` をそのまま返すだけの stub になっている。actor-watch 互換ステージとして実処理を追加し、stub を排除する。

## 現状

```rust
/// Adds an actor-watch compatibility stage.
#[must_use]
pub const fn watch(self) -> Self {
  self
}
```

対象箇所: `modules/streams/src/core/stage/source.rs:187-191`

## 要件

- [ ] `Source::watch()` に actor-watch 互換の実処理を実装する
- [ ] Apache Pekko の `Source.watch` / `WatchTermination` 相当の機能を参照実装として確認する（`references/pekko/`）
- [ ] 実装が不要と判断した場合はメソッド自体を削除し、呼び出し元があれば修正する
- [ ] 既存テストが全てパスすること

## 受け入れ基準

- `watch()` が `self` をそのまま返す stub 状態ではないこと（実装 or 削除）
- `cargo test -p fraktor-streams-rs` が全てパスすること
- `./scripts/ci-check.sh dylint -m streams` がパスすること

## 参考情報

- Pekko の `WatchTermination`: ストリームの完了を監視するステージ。マテリアライズ値として `Future[Done]` を提供する
- `references/pekko/` 内の該当実装を確認すること
