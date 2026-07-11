# RFC pekko-0008: typed 層（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/`（`Behavior.scala`, `internal/BehaviorImpl.scala`, `internal/Supervision.scala`, `internal/adapter/ActorAdapter.scala`, `internal/receptionist/LocalReceptionist.scala`, `delivery/`） |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 対応 fraktor RFC | [0008](../0008-actor-typed-layer.md) |
| 最終照合日 | 2026-07-11 |

## 1. 規範仕様

### 1.1 Behavior の内部表現

- **PTY-1.** `Behavior[T]` は `_tag: Int` による 9 タグ判別: `ExtensibleBehavior` / `EmptyBehavior` / `IgnoreBehavior` / `UnhandledBehavior` / `DeferredBehavior` / `SameBehavior` / `FailedBehavior` / `StoppedBehavior` / `SuperviseBehavior`。`same` / `unhandled` / `stopped` / `empty` / `ignore` はシングルトンの unsafe cast。
- **PTY-2.** `canonicalize` は `Same` / `Unhandled` を現在の behavior に解決し、`Deferred` を再帰評価する。`empty` は「常に unhandled」に解釈される。
- **PTY-3.** `StoppedBehavior` は `PostStop` シグナルのときのみ post-stop コールバックを実行し、他のメッセージは無視する。停止シーケンスは `ActorAdapter.postStop` が `interpretSignal(PostStop)` を呼んでから behavior を stopped にリセットする。

### 1.2 typed ↔ classic 接続（ActorAdapter）

- **PTY-4.** classic からの `Any` メッセージは `msg.asInstanceOf[T]` で**無検査ダウンキャスト**される（型安全性は ActorRef 型付けによる構築時保証に委ねられ、実行時検査はない）。
- **PTY-5.** restart 時は `postRestart` が `Behavior.start`（Deferred の再帰評価）で behavior を初期化し直す（preStart 相当の再実行）。`preRestart` は全タイマーをキャンセルして `PreRestart` シグナルを配り、behavior を stopped にする。

### 1.3 death pact と unhandled

- **PTY-6.** `Behavior.interpretSignal` は、`Terminated` シグナルの解釈結果が `UnhandledBehavior` の場合に **`DeathPactException(ref)` を throw** する（supervision で捕捉可能にするための位置。`ActorAdapter.unhandled` の同種コードは到達しない二重防御と明記されている）。

### 1.4 supervise

- **PTY-7.** `Behaviors.supervise(...).onFailure(strategy)` は `BehaviorInterceptor` として `RestartSupervisor` / `ResumeSupervisor` / `StopSupervisor` のラッパーを合成する。Resume は例外を握って `same`、Stop は `failed(t)`、Restart は `maxRestarts`（既定 -1 = 無制限）+ `withinTimeRange` を判定し、Backoff は `ScheduledRestart` を自己スケジュールしつつ **StashBuffer**（容量 `stashCapacity` / 既定は system 設定 `RestartStashCapacity`）へ受信メッセージを退避する。

### 1.5 Receptionist / delivery

- **PTY-8.** Receptionist のプロトコルは `Register` / `Deregister` / `Subscribe` / `Find` と応答 `Registered` / `Deregistered` / `Listing`。登録者・購読者とも `watchWith` で監視され、終了時に自動 deregister / 購読解除される（「登録解除は参照先のライフサイクル終了によっても暗黙に起こる」と API doc に明記）。
- **PTY-9.** delivery は consumer 駆動の at-least-once: `Delivery(message, confirmTo)` を受けた consumer が `Confirmed` を返すまで producer は未確認メッセージを保持・再送する。喪失検出・再送・重複排除は consumer 側の demand が駆動する。`DurableProducerQueue` は confirmed seqNr マップと未確認列を永続化してクラッシュ後再送を支える。`WorkPullingProducerController` は Receptionist の ServiceKey でワーカーを動的発見し、**順序保証なし**でルーティングする。

## 2. 不変条件

- **INV-PTY-1**: `Terminated` シグナルの未処理は必ず `DeathPactException` になる（PTY-6）。
- **INV-PTY-2**: restart を跨いで古い behavior 状態が残ることはない（PTY-5 の Deferred 再評価）。
- **INV-PTY-3**: Receptionist の Listing に終了済み参照が残り続けることはない（watchWith 自動除去、PTY-8）。

## 3. fraktor-rs との差分

| 観点 | Pekko | fraktor-rs |
|------|-------|-----------|
| Behavior 表現 | タグ付きクラス階層 + canonicalize（interceptor 合成） | directive enum + ハンドラ 3 種の struct（`BehaviorRunner` が解釈） |
| ダウンキャスト | 無検査 `asInstanceOf[T]`（失敗は ClassCastException） | `downcast_ref` 失敗を **Recoverable な ActorError** に変換（supervision へ流す） |
| death pact | `interpretSignal` で throw（例外） | `DeathPactError` 入りの `ActorError::recoverable` を返す（同義。位置も runner 内で同等） |
| Restart 中のメッセージ | Backoff 時 StashBuffer に退避 | backoff supervisor が stash mailbox を要求（同趣旨） |
| supervise の既定 restart 上限 | -1（無制限） | fraktor untyped 既定は WithinWindow(10)/1s（typed は strategy 指定次第） |
| Unhandled の観測 | `UnhandledMessage` publish（untyped 経由） | `UnhandledMessage` publish（typed runner が発行） |
| delivery | at-least-once / consumer 駆動 / durable queue / work-pulling | 同等の対応物あり（fraktor RFC 0008 TY-14。詳細比較は将来の個別 RFC） |

## 4. 参照

- fraktor 側 RFC 0008
- `Behavior.scala:183-306`（canonicalize / interpret / interpretSignal）、`internal/BehaviorImpl.scala:29-172`、`internal/Supervision.scala:45-421`、`internal/adapter/ActorAdapter.scala:115-335`、`internal/receptionist/LocalReceptionist.scala`、`delivery/{ProducerController,ConsumerController,DurableProducerQueue,WorkPullingProducerController}.scala`
