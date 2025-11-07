# 実装タスク: typed Behaviors::supervise

1. **調査**
   - [ ] Pekko `Behaviors.supervise` の仕様要点を整理し、Rust 版で実現する範囲を記述する。
2. **基盤の拡張**
   - [ ] `modules/actor-core/src/typed/actor_prim/actor.rs` に `supervisor_strategy` を追加し、デフォルト実装を用意する。
   - [ ] `modules/actor-core/src/typed/typed_actor_adapter.rs` で `Actor` トレイトの `supervisor_strategy` 呼び出しを typed 側へ委譲する。
   - [ ] `modules/actor-core/src/typed/behavior.rs` に監督戦略オプションを保持できるフィールド／アクセサを追加する。
3. **DSL 実装**
   - [ ] `modules/actor-core/src/typed/behaviors.rs` と新規モジュールで `Behaviors::supervise` と `Supervise` ビルダーを実装する。
   - [ ] `BehaviorRunner` が `Behavior` から戦略を引き継ぎ、`TypedActor` の `supervisor_strategy` で返すようにする。
4. **サンプルとテスト**
   - [ ] `modules/actor-core/src/typed/tests.rs` に restart/stop/escalate を検証するテストを追加する。
   - [ ] `modules/actor-std/examples` の typed サンプルから少なくとも 1 つ `Behaviors::supervise` を使用する例を追加/更新する。
5. **ドキュメント**
   - [ ] `Behaviors::supervise` の RustDoc を追加し、使い方と注意点を記述する。
   - [ ] `CHANGELOG.md` もしくは `docs` に破壊的変更（typed actor trait にメソッド追加）を明記する。
6. **検証**
   - [ ] 影響範囲の `cargo test`/`cargo fmt` を実行し、最終的に `./scripts/ci-check.sh all` を成功させる。
