# 提案: typed `Behaviors::supervise` の移植

**Change ID**: `add-typed-behavior-supervision`
**作成日**: 2025-11-07
**ステータス**: 提案中

## 概要

typed API に Pekko 互換の `Behaviors::supervise` DSL を追加し、typed actor が自身の監督戦略を宣言的に指定できるようにする。`BehaviorRunner` と `TypedActorAdapter` を拡張し、`SupervisorStrategy` の決定責務を typed レイヤから untyped `Actor` トレイトへ確実に受け渡す。

## 動機

1. **監督戦略の指定漏れ**: 直近の変更で `Actor` トレイトに `supervisor_strategy` が移設されたが、typed 側では常にデフォルト戦略が適用され、DSL から制御できない。
2. **Pekko 互換性**: 既存ユーザが Pekko typed DSL (`Behaviors.supervise(...).onFailure(...)`) を参照して移行するケースが多い。現状の API には等価機能がなく、サンプルコードも書き換えられない。
3. **テスト容易性**: typed 向けに監督戦略を差し替えられないため、再起動・停止・エスカレーションの分岐テストが書けない。

## 変更内容

- `Behaviors::supervise` を追加し、`Behavior` に `SupervisorStrategy` 上書き情報を付与できるビルダー (`Supervise<M, TB>`) を提供。
- `Behavior` 構造体に監督戦略オプションを保持するフィールドとアクセサを追加。再帰的に生成される `Behavior` でも上書き情報を持ち越せるようにする。
- `BehaviorRunner` が最新の `Behavior` から監督戦略を読み取り、`TypedActor` 実装として保持。
- `TypedActor` トレイトに `supervisor_strategy` を追加し、`TypedActorAdapter` が untyped `Actor` トレイトから呼ばれた際に委譲する。
- typed テスト・サンプルで `Behaviors::supervise` を使用したシナリオを追加し、再起動/停止/エスカレートを検証。

## 影響範囲

- `modules/actor-core/src/typed` 配下（`behavior.rs`, `behavior_runner.rs`, `behaviors.rs`, `typed_actor_adapter.rs`, `actor_prim/actor.rs`, `tests.rs`）。
- `modules/actor-std/examples` の typed サンプルの一部。
- 既存 API への破壊的変更: `TypedActor` トレイトにメソッドが追加されるため、typed actors を実装しているコードは `supervisor_strategy` のデフォルト実装を受け取る。既存ユーザへの影響は最小。

## オープンな課題

- `SupervisorStrategy` のデコレータが 1 件のみで十分か（Pekko では複数例外をチェイン可能）。初回は単一戦略のみで実装し、将来拡張でチェインを検討。
- `no_std` でのメモリフットプリントを計測し、`SupervisorStrategy` フィールド追加によるサイズ増を把握する。

## 承認基準

- typed API から監督戦略を指定でき、サンプル・テストで restart/stop/escalate を確認できること。
- `TypedActor` を実装する既存コードは追加の手当てなくビルドが通る（デフォルト実装）。
- `openspec validate add-typed-behavior-supervision --strict` が成功すること。
