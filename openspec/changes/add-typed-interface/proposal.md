## Why
利用者が `ActorSystemGeneric` や `ActorRefGeneric` を直接扱う場合、メッセージが `AnyMessageGeneric` で表現されるため型安全性がなく、誤った payload を送ってもコンパイル時に検出できない。また、`protoactor-go` や `pekko` の最新実装では Typed API が併設されており、アクター同士の契約を enum などで明示できるようになっている。cellactor-rs でもユーザーフェacing API を Typed 化すれば、破壊的変更を避けつつ安全性と DX を改善できる。

## What Changes
- `modules/actor-core/src/typed.rs` を起点に Typed API をまとめる新サブモジュールを追加する。
- `TypedActorSystemGeneric<TB, M>` を導入し、既存 `ActorSystemGeneric<TB>` を内包させて API を移譲する。ユーザーガーディアンには `BehaviorGeneric<TB, M>` で受け取った Typed な Props を渡す。
- `TypedActor<TB, M>`・`BehaviorGeneric<TB, M>` を新設し、`BehaviorGeneric` から `PropsGeneric<TB>` へ変換するアダプタ（AnyMessage へのアップキャスト）を提供する。
- `TypedActorRefGeneric<TB, M>` / `TypedChildRefGeneric<TB, M>` / `TypedActorContextGeneric<'a, TB, M>` を追加し、それぞれ既存 Untyped 型を包む薄い newtype で `tell/ask/reply/spawn_child` 等を型付き API へ張り替える。
- Untyped API と Typed API を並存させ、型変換ヘルパー（`into_untyped`, `from_untyped` 等）を用意して段階的移行を支援する。
- ドキュメントとサンプルを更新し、Typed API の使い方（enum メッセージ、Behavior の作り方、Typed/Untyped 併用方法）を記載する。

## Impact
- ユーザーは従来の Untyped API をそのまま利用できる一方、Typed API を通じてメッセージ型をコンパイル時に保証できる。
- ランタイム内部の構造体は一切変更せず、公開表層のみ薄いラッパーを追加するためリスクが限定的。
- 将来的な Typed 専用機能（Typed supervisor 戦略、Typed ask など）を追加しやすくなる。

## Scope
### Goals
1. Actor System / Actor / ActorRef / ChildRef / ActorContext それぞれに Typed 版を提供し、Untyped 版の API を完全に覆う。
2. `BehaviorGeneric<TB, M>` の作成と Props 化を容易にするためのビルダー API を実装する。
3. Typed API から Untyped API へエスケープする変換関数を用意し、既存拡張との互換性を確保する。
4. ドキュメント・サンプル・テストを更新し、Typed API の代表的なフロー（spawn、tell、ask、reply）を検証する。

### Non-Goals
- Untyped API の削除や大幅なシグネチャ変更。
- Typed ask 応答の完全型安全化（応答型パラメータ化）は別提案とする。
- ガーディアン以外のシステムアクターを Typed 化すること。

## Rollout Plan
1. `modules/actor-core` に `typed` サブモジュールを追加し、ファイルレイアウト（system.rs, behavior.rs, actor_prim/...）を準備する。
2. `TypedActor` / `BehaviorGeneric` / `TypedActorSystemGeneric` を実装し、`BehaviorGeneric` から `PropsGeneric` へ変換するアダプタを提供する。
3. `TypedActorRef` / `TypedChildRef` / `TypedActorContext` を段階的に導入し、Untyped API へ委譲する実装を整える。
4. サンプルとドキュメントを更新し、Typed API でカウンター actor を spawn してメッセージをやり取りするチュートリアルを追加する。
5. すべてのテストと `./scripts/ci-check.sh all` を実行し、回帰がないことを確認する。

## Risks & Mitigations
- **ジェネリクス増加によるコンパイル時間悪化**: Typed API は薄い newtype で内包するだけに留め、内部実装には影響させない。
- **Downcast 失敗時の取り扱い**: Typed ラッパー経由でのみ `AnyMessage` を生成するため、`TypedActor` 側で検査する必要がない設計とし、Untyped に戻る場合は明示的に `AnyMessage` を作るようドキュメントに記載する。
- **API 乱立による混乱**: ガイドラインを proposal/task/spec とドキュメントで明示し、Typed API は「ユーザー向け推奨、Untyped は内部/拡張用」と役割分担する。

## Impacted APIs / Modules
- `modules/actor-core/src/system.rs` および新設 `modules/actor-core/src/typed/system.rs`
- `modules/actor-core/src/actor_prim/*`（Typed ラッパー追加）
- `modules/actor-core/src/props.rs`（Behavior 変換導線）
- ドキュメント／サンプル（README, docs 配下）

## References
- protoactor-go Typed API (`actor/props`, `typed/actor_context`) の構成
- pekko Typed API の `Behaviors.setup` と薄いアダプション層
- 現状の Untyped API (`ActorSystemGeneric`, `ActorRefGeneric`, `ActorContextGeneric`)
