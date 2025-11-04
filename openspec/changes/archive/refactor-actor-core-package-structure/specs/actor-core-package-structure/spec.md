# Capability: Actor Core Package Structure

## 概要
`modules/actor-core/src` のパッケージ構造を定義する。モジュールを機能別に階層化し、内部実装を隠蔽し、保守性と可読性を向上させる。

## MODIFIED Requirements

### Requirement: パッケージ階層構造
`modules/actor-core/src` SHALL be organized into 11 logical packages. モジュールは機能別に階層化され、内部実装が隠蔽されなければならない (MUST)。

#### Scenario: アクタープリミティブへのアクセス
**Given** ユーザーがアクターの基本機能を使用したい
**When** `use cellactor_core::actor_prim::actor::Actor;` でインポートする
**Then** `Actor` トレイトと関連する型（`ActorRef`, `ActorCell`, `ActorContext`等）にアクセスできる
**And** 内部実装の詳細（例: `actor_prim/actor_ref_internal.rs`）は公開されていない

#### Scenario: メッセージングパッケージの使用
**Given** ユーザーがメッセージング機能を使用したい
**When** `use cellactor_core::messaging::any_message::AnyMessage;` でインポートする
**Then** メッセージングに関連する型（`AnyMessage`, `AskResponse`, `MessageInvoker`等）にアクセスできる

#### Scenario: preludeによる便利なインポート
**Given** ユーザーが主要な型をまとめてインポートしたい
**When** `use cellactor_core::prelude::*;` でインポートする
**Then** よく使われる型（`Actor`, `ActorRef`, `ActorContext`, `Props`, `ActorSystem`等）がすべてインポートされる

### Requirement: 破壊的変更の許容
パッケージ構造の大規模な再編成に伴い、既存のフラットなインポートパスは廃止する (MUST)。`modules/actor-core/src` の公開 API は新しい階層構造に合わせて再設計し、crate ルートや中間モジュールでの再エクスポートは行わない (SHALL)。利用側は Module Wiring ガイドラインに従い、末端モジュールまたは `prelude` から明示的に型を取り込む必要がある。

#### Scenario: 既存コードの移行
**Given** 既存のコードが `use cellactor_core::Actor;` のように旧来のフラットパスでインポートしている
**When** パッケージ構造のリファクタリングが完了する
**Then** 旧来のインポートパスはコンパイルエラーになる
**And** 利用者は `use cellactor_core::actor_prim::actor::Actor;` や `use cellactor_core::prelude::*;` といった新しい階層構造へ明示的に移行する

#### Scenario: パッケージルートの再エクスポート禁止
**Given** `actor_prim.rs` のようなパッケージルートファイルが存在する
**When** ルートファイルの公開シンボルを確認する
**Then** 子モジュールは `pub mod actor;` のように公開される
**And** `pub use actor::Actor;` のような再エクスポート宣言は存在しない

### Requirement: 内部実装の隠蔽
公開APIと内部実装を明確に分離しなければならない (MUST)。内部実装は `*_internal.rs` へ切り出し、`pub(crate)` で可視性を制限する (SHALL)。

#### Scenario: 内部実装へのアクセス制限
**Given** ユーザーが`actor`パッケージを使用する
**When** 公開されていない内部実装（`actor_prim/actor_ref_internal.rs` など）にアクセスしようとする
**Then** コンパイルエラーが発生する
**And** 内部実装の詳細は外部から参照できない

### Requirement: パッケージ別ドキュメント
各パッケージには、モジュールレベルのドキュメントが含まれなければならない (MUST)。ドキュメントには、パッケージの責務と使用例を記載する (SHALL)。

#### Scenario: パッケージドキュメントの確認
**Given** ユーザーが `cargo doc` でドキュメントを生成する
**When** 生成されたドキュメントを確認する
**Then** 各パッケージ（`actor`, `messaging`, `mailbox`等）にモジュールレベルのドキュメントが存在する
**And** パッケージの責務と使用例が記載されている

### Requirement: パッケージ構造の定義
以下の13個の論理パッケージが存在しなければならない (MUST)：

1. `actor_prim` - アクタープリミティブ（`Actor`, `ActorRef`, `ActorCell`, `ActorContext`, `Pid`, `ChildRef`, `ReceiveState`）
2. `messaging` - メッセージング（`AnyMessage`, `AskResponse`, `MessageInvoker`, `SystemMessage`）
3. `mailbox` - メールボックス（`Mailbox`, `MailboxCapacity`, `MailboxPolicy`, `MailboxOverflowStrategy`, `MailboxMetricsEvent`）
4. `supervision` - スーパービジョン（`SupervisorStrategy`, `SupervisorDirective`, `RestartStatistics`）
5. `props` - アクター生成設定（`Props`, `ActorFactory`, 設定関連）
6. `spawn` - アクター生成処理（`SpawnError`, `NameRegistry`）
7. `system` - アクターシステム（`ActorSystemGeneric`, `ActorSystem`, `SystemState`, `Dispatcher`）
8. `eventstream` - イベントストリーム（`EventStream`, `EventStreamEvent`, `EventStreamSubscriber`等）
9. `lifecycle` - ライフサイクル（`LifecycleEvent`, `LifecycleStage`）
10. `deadletter` - デッドレター（`Deadletter`, `DeadletterEntry`, `DeadletterReason`）
11. `logging` - ロギング（`LogEvent`, `LogLevel`, `LoggerSubscriber`, `LoggerWriter`）
12. `futures` - Future統合（`ActorFuture`, `ActorFutureListener`）
13. `error` - エラー型（`ActorError`, `ActorErrorReason`, `SendError`）

#### Scenario: パッケージの存在確認
**Given** パッケージ構造のリファクタリングが完了している
**When** `modules/actor-core/src/` ディレクトリを確認する
**Then** 上記の11個のパッケージディレクトリが存在する
**And** 各パッケージにはルートファイル（例: `actor_prim.rs`）が存在する

### Requirement: テストの配置
各モジュールのテストは、対応するパッケージ内に配置されなければならない (MUST)。すべてのテストは正常にパスする (SHALL)。

#### Scenario: テストファイルの配置
**Given** `actor.rs` に関連するテストが存在する
**When** パッケージ構造のリファクタリングが完了した後
**Then** テストは `actor_prim/` 配下に配置されている
**And** すべてのテストが正常にパスする

## 非機能要件

### 保守性
- モジュール間の依存関係が明確であること
- 各パッケージの責務が明確に定義されていること

### ビルド時間
- パッケージ分割により、並列ビルドの最適化が可能であること
- インクリメンタルビルドが効率的に動作すること

### ドキュメント
- 各パッケージにモジュールレベルのドキュメントが存在すること
- 使用例が含まれていること
