# ActorSystem グローバル spawn 廃止設計

## 目的
- `ActorSystem` の責務をガーディアン起動とインフラ監視に限定し、アクター階層外からの生成・停止・探索を禁止する。
- Pekko Typed の `SpawnProtocol` や protoactor-go の `RootContext` 相当の流れを取り込み、生成要求をメッセージ駆動に揃える。
- API の破壊的変更を安全に進めるためのモジュール分割・テスト方針を明確にする。

## 要件整理
1. `ActorSystemGeneric`/`ActorSystem` の公開 API から `spawn`/`spawn_child`/`actor_ref`/`children`/`stop_actor` を排除する。
2. 既存コードは `ActorContext` もしくはガーディアンへの専用メッセージでのみ子アクターを生成できるようにする。
3. PID 直指定の停止・探索 API は `ActorContext` からアクセスできる範囲に閉じ込め、外部へは露出しないよう可視性を絞る。
4. `no_std` と `StdToolbox` の両方で同じ API サーフェスを保ち、`#[cfg(feature = "std")]` をランタイム本体に追加しない。
5. 変更後も `./scripts/ci-check.sh all` が通ることを前提に、該当テストを全面的に更新する。

## 現状課題
- `modules/actor-core/src/system/base.rs` で `ActorSystemGeneric` が `pub fn spawn`, `spawn_child`, `actor_ref`, `children`, `stop_actor` を公開しており、任意コードがアクター階層をバイパスできる。
- `modules/actor-std/src/system/base.rs` も上記 API をそのまま re-export しており、標準ランタイム利用者が `system.spawn(&Props)` を直接呼べてしまう。
- `modules/actor-core/tests/*.rs` や `modules/actor-std/tests/*.rs` ではシナリオ開始のたびに `system.spawn` を使っており、ガーディアン経由のドリルが成立していない。
- `ActorContext` は `ctx.spawn_child` を提供しているが、`ActorSystem` 側の公開 API が温存されているため境界が曖昧。

## 設計方針
### 1. ActorSystem API 縮退
- `ActorSystemGeneric` の `spawn`, `spawn_child`, `actor_ref`, `children`, `stop_actor` を `pub(crate)` へ変更し、クレート外公開を止める。`state`, `event_stream`, `terminate` などの管理系 API は現状維持。
- `modules/actor-std/src/system/base.rs` でも同名メソッドを削除し、利用者は `ActorSystem::user_guardian_ref`・`terminate`・イベント/メトリクス取得のみが可能になるようにする。
- 内部で直接呼ぶ必要がある箇所（`ActorContext`, `ChildRef`, `SystemState` テスト等）は同一クレート内なので影響なし。クレート外テストは後述の新しいメッセージ駆動に書き換える。

### 2. ActorContext/Cell まわりの整理
- `ActorContext` から `ActorSystemGeneric` へ依存している箇所はそのまま維持しつつ、生成・停止・子列挙は Context API からのみ行えることを rustdoc で明記する。
- `ActorContext::stop_self`/`children` などが内部で使う `system.*` 呼び出しが crate-private 化の影響を受けないよう、`ActorContext` 内に `fn system_state(&self) -> ArcShared<SystemState<TB>>` のような補助メソッドを追加する案も検討（必要なら追加）。
- 将来的な Typed 化を見据え、Context 側に `spawn_guardian_child` などの糖衣は追加せず、Props/ChildRef ベースの API を維持する。

### 3. 既存コードの移行
- `modules/actor-core/tests` と `modules/actor-std/tests` で `system.spawn(...)` を直接呼んでいる箇所を、`TestGuardian` 経由のメッセージまたは `SpawnClient` に置き換える。テスト毎に匿名 guardian を差し込むのではなく、`tests/common/test_guardian.rs` のような共有ヘルパーを新設して重複を避ける。
- ドキュメントとサンプル（`modules/actor-std/examples/*`）は `system.user_guardian_ref().tell(AnyMessage::new(SpawnProtocolCommand::Spawn { .. }))` もしくは `SpawnClient` の利用例に更新する。

## データフロー概要
1. アプリが `ActorSystem::new(&Props::from_fn(|| SpawnProtocolActor::new(...)))` で起動。
2. トップレベル生成要求は `SpawnClient::spawn` が `SpawnProtocolCommand::Spawn` を guardian に `ask` 送信。
3. ガーディアン（`SpawnProtocolActor`）は `ctx.spawn_child` を実行し、`SpawnProtocolResult` を `reply_to` へ送付。
4. 呼び出し側は `SpawnProtocolResult` の `Result` を確認し、成功時は `ChildRef` を受け取る。失敗時は `SpawnError` をそのまま伝播。

## API 変更一覧
| 種別 | 旧 API | 新 API |
| --- | --- | --- |
| 削除 | `ActorSystemGeneric::spawn`, `spawn_child`, `actor_ref`, `children`, `stop_actor` | crate-private に縮退（外部からは不可） |
| 削除 | `ActorSystem::spawn`, `spawn_child`, `actor_ref`, `children`, `stop_actor` | メソッドごと削除 |
| 追加 | – | `SpawnProtocolCommand`, `SpawnProtocolResult`, `SpawnProtocolActor`, `SpawnClient` |
| 追加 | – | `StdSpawnClient` (type alias) と `spawn_async` 補助関数 |

## 実装ステップ
1. `ActorSystemGeneric`/`ActorSystem` の該当メソッドを crate-private に落とし、影響を受ける内部テストを修正。
2. `spawn` モジュールに SpawnProtocol 系の新ファイルを追加し、`spawn.rs` から `pub use` を提供。
3. `SpawnClient` を実装し、`modules/actor-std` 側に std 特化の利便関数を配置。
4. guardian 初期化フローを点検し、`ActorSystem::new` で `SpawnProtocolActor` を使う手順をドキュメント化。
5. 既存テスト/サンプルを `SpawnProtocol` ベースに全面更新。
6. `openspec/changes/remove-actorsystem-global-spawn/tasks.md` のチェックリストを埋めつつ、`openspec validate remove-actorsystem-global-spawn --strict` と `./scripts/ci-check.sh all` を実行。

## テスト戦略
- `modules/actor-core/tests`: `system_lifecycle.rs`, `event_stream.rs`, `ping_pong.rs`, `supervisor.rs` などで `SpawnProtocolActor` を guardian に使ったシナリオテストを追加し、直接 spawn を使っていたケースは廃止。
- `modules/actor-std/tests/tokio_acceptance.rs`: `SpawnClient` を用いたトップレベル spawn テストを追加し、Tokio executor との連携が維持されることを確認。
- `modules/actor-std/examples`: `ping_pong_tokio`, `deadletter_std`, `named_actor_std` すべてを `SpawnProtocol` 利用例に書き換え、README/SPEC と同期。
- `makers ci-check`/`./scripts/ci-check.sh all`: 全リンタと doctest, dylint を最終確認として実行。

## リスクとオープン課題
- `SpawnClient` が `AskResponse` の完了待ちに依存するため、`no_std` 環境での busy loop をどう隠蔽するか（現状は `ActorFuture::poll_immediate` で凌ぐ予定）。
- 既存ユーザが `ActorSystem::state()` から直接 `SystemState::send_system_message` を呼ぶ抜け道は残る。必要に応じて今後 `state()` の露出レベルを下げる追加提案が必要。
- `SpawnProtocolActor` のエラーハンドリングをどうロギングするか（Pekko では `StatusReply` を使う）。初期実装では `SpawnProtocolResult` の `Err(SpawnError)` 返却のみとし、詳細ログは `SpawnClient` 呼び出し側で行う方針。
