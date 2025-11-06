## 1. 設計と調査
- [ ] Typed API で覆うべき Untyped API 一覧（ActorSystem/ActorRef/ChildRef/ActorContext/Props）を洗い出し、必要な委譲ポイントをドキュメント化する
- [ ] 既存のメッセージフロー（spawn → tell → ask → reply）のテストケースを確認し、Typed 版で流用または追加すべき観点を整理する

## 2. 実装
- [ ] `modules/actor-core/src/typed.rs` と `typed/` 配下のファイルを追加し、モジュールをエクスポートする
- [ ] `TypedActor` / `BehaviorGeneric` / `TypedActorSystemGeneric` を実装し、`BehaviorGeneric` から `PropsGeneric` への変換を提供する
- [ ] `TypedActorRefGeneric` / `TypedChildRefGeneric` / `TypedActorContextGeneric` を実装し、Untyped API への委譲と型安全な `tell/ask/reply/spawn_child` を提供する
- [ ] Typed API と Untyped API を相互に変換するための `into_untyped` / `from_untyped` などのヘルパーを追加する

## 3. テスト・ドキュメント・CI
- [ ] Typed API を利用したサンプル（例: カウンター actor）と単体テストを追加し、代表的なパス（spawn/tell/ask/reply）が動作することを確認する
- [ ] README や関連ドキュメントに Typed API の利用手順と移行ガイドラインを追記する
- [ ] `./scripts/ci-check.sh all` を実行し、全テストと lint が成功することを確認する
