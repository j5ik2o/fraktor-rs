# RFC pekko-0008: typed 層（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/`（`Behavior.scala`, `MessageAndSignals.scala`, `Props.scala`, `BehaviorInterceptor.scala`, `SpawnProtocol.scala`, `ActorRefResolver.scala`, `Extensions.scala`, `scaladsl/{ActorContext,AskPattern,Routers,StashBuffer}.scala`, `pubsub/Topic.scala`, `eventstream/EventStream.scala`, `internal/`, `internal/adapter/`, `internal/routing/`, `internal/receptionist/LocalReceptionist.scala`, `delivery/`） |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 最終照合日 | 2026-07-12 |

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

### 1.5 Signal の全列挙

- **PTY-10.** typed の `Signal` は 5 種: `PreRestart`（restart 時、新 behavior への置換**前**に旧 behavior へ）/ `PostStop`（自身と子が推移的に終了した**後**。watcher への `Terminated` はこの処理後に送られる）/ `Terminated(ref)` / `ChildFailed(ref, cause)`（`Terminated` のサブクラス。子が未捕捉例外で失敗した場合のみ）/ `MessageAdaptionFailure(exception)`（メッセージアダプタの変換中例外。既定のシグナルハンドラは再 throw して supervision へ委ねる）。`DeathPactException` はシグナルではなく例外である（PTY-6）。

### 1.6 ActorContext の契約

- **PTY-11.** `ActorContext` の公開 API は spawn / spawnAnonymous / stop / watch / watchWith / unwatch / setReceiveTimeout / cancelReceiveTimeout / scheduleOnce / messageAdapter / ask / askWithStatus / pipeToSelf / log / setLoggerName / children / child / delegate である。`stop` は**直接の子のみ**停止でき、自分自身を渡すと `IllegalArgumentException`（自停止は `Behaviors.stopped` を使う。MUST）。
- **PTY-12.** ほぼすべての API に「actor のメッセージ処理スレッド以外（Future コールバック等）から呼んではならない」と明記されている。thread-safe と明記されるのは `self` / `system` / `scheduleOnce` / `executionContext` / `ask` / `pipeToSelf` のみである（MUST）。
- **PTY-13.** `messageAdapter` はメッセージクラスごとに 1 つのみ登録でき（再登録は置換）、照合は登録の逆順で行われる。

### 1.7 Props / DispatcherSelector / MailboxSelector

- **PTY-14.** typed `Props` は内部リンクリストであり、同一種別の設定は**最初の出現が勝つ**（`with*` の追加が既存設定を上書きする）。`DispatcherSelector` は default / blocking（`pekko.actor.default-blocking-io-dispatcher`）/ fromConfig / sameAsParent、`MailboxSelector` は default（`pekko.actor.typed.default-mailbox` = `SingleConsumerOnlyUnboundedMailbox`）/ bounded(capacity) / fromConfig を提供する。
- **PTY-15.** `PropsAdapter` は classic `Props` への変換時に必ず `Deploy.local` を付与し、**typed actor のリモートデプロイを禁止**する（MUST NOT）。

### 1.8 BehaviorInterceptor

- **PTY-16.** `BehaviorInterceptor` は `aroundStart` / `aroundReceive` / `aroundSignal` を持ち、`interceptMessageClass` に一致しないメッセージは interceptor を**バイパス**して内側の behavior へ直接届く。`isSame`（既定は参照同一）が真の interceptor がスタックに既にあれば新しい層を作らない（behavior スタックの無限成長防止）。`withMdc` は常に既存を置換、`transformMessages` は同一 matcher 以外の重ね掛けを例外で拒否する。`Behaviors.logMessages(LogOptions)` も interceptor として実装され、MDC はメッセージ処理ごとに `finally` で必ずクリアされる。

### 1.9 StashBuffer

- **PTY-17.** typed `StashBuffer` は生成時に capacity を要求し、`isFull` での `stash` は `StashOverflowException` になる。unstash 中に behavior が停止した場合、残りのメッセージは DeadLetter へ転送される。
- **PTY-18.** unstash 中の例外は `UnstashException(cause, behavior)` にラップされ、`ActorAdapter` が「実際に例外を投げた behavior」を復元してから元の例外を再 throw する。これにより supervision は正しい behavior へ `PreRestart` / `PostStop` を配れる。

### 1.10 ask と SpawnProtocol

- **PTY-19.** typed `ask` は対象が終了済みなら即時に `TimeoutException`、timeout ≤ 0 は `IllegalArgumentException` で失敗する。`askWithStatus` は `StatusReply` を平坦化する（`Error` は例外へ変換）。actor 内では context 外の ask より `ActorContext.ask` が推奨と doc に明記されている。
- **PTY-20.** `SpawnProtocol.Spawn(behavior, name, props, replyTo)` は name が空なら匿名 spawn し、名前衝突時は `name-1`, `name-2`, … の連番で空き名を探す（サフィックス形式は実装詳細と宣言）。生成した ref を `replyTo` へ返信し、behavior 自体はステートレスである。

### 1.11 pubsub Topic

- **PTY-21.** `Topic` のコマンドは `Publish` / `Subscribe` / `Unsubscribe` / `GetTopicStats` の 4 種。トピック毎に `ServiceKey(topicName)` を生成し、**ローカル購読者が 0 → 1 になったときだけ** Receptionist へ Register、最後の購読者が消えたら Deregister する（購読者のいないノードへは配信されない）。
- **PTY-22.** 配送は「他の Topic インスタンスが既知なら `MessagePublished` を全インスタンスへ転送し、各インスタンスが自ノードのローカル購読者へ再配信する」二段方式で、ノード内の重複配信を排除する。購読者もインスタンスも無い publish は `Dropped` として deadLetters へ送られる。購読者は `watchWith` で監視され、終了時に自動除去される。

### 1.12 Routers（typed）

- **PTY-23.** `PoolRouter` は poolSize 体の子を spawn し、既定は round-robin。子は watch され、停止した子は pool から除去、**全滅で pool 自身が停止**する。broadcast 述語が真のメッセージは全子へ配送される。ConsistentHashing 選択時は `virtualNodesFactor > 0` が必須である。
- **PTY-24.** `GroupRouter` は Receptionist の `ServiceKey` 購読で routee 集合を維持し（監視は Receptionist に委譲して自前 watch しない）、初回 `Listing` 到着まで内部 stash（容量 10000、超過は `Dropped`）する。routee ゼロでのメッセージは `Dropped` として publish される。`preferLocalRoutees = true` はローカル routee が存在する場合のみローカルへ限定する。

### 1.13 システム接続ユーティリティ

- **PTY-25.** `ActorRefResolver`（typed Extension）は `toSerializationFormat` / `resolveActorRef` を提供し、別 ActorSystem 由来の ref を渡すと `IllegalArgumentException` になる。typed の `Extensions` は `ExtensionId`（インスタンス同一性で同定）+ `ExtensionSetup` による差し替え + `pekko.actor.typed.extensions` の起動時ロードを持つ。`eventstream` パッケージは `EventStream.Publish` / `Subscribe` / `Unsubscribe` コマンドを classic EventStream へ委譲する。
- **PTY-26.** typed timers（`TimerSchedulerImpl`）は classic と同一の意味論で、key の存在・owner 一致（restart 跨ぎ排除）・generation 一致の 3 条件照合により stale timer メッセージを排除する（pekko-0006 PSCH-10 と同じ）。

### 1.14 typed ↔ classic adapter 群

- **PTY-27.** `ActorSystemAdapter` は classic system を typed `ActorSystem` に見せる（tell は user guardian へ委譲、`systemActorOf` は `SupervisorStrategy.stop` の supervise でラップ）。同一 classic system への adapter は `AdapterExtension` でキャッシュされ一意である。`ActorRefAdapter` は typed 内部 `SystemMessage` を classic `sysmsg.*` へ変換し、`ActorContextAdapter` は spawn / stop / watch を classic へ委譲する（`messageAdapter` は `ActorCell.addFunctionRef` で実装）。
- **PTY-28.** classic `Terminated` → typed シグナル変換は `ActorAdapter.aroundReceive` で行われる: supervisorStrategy が watch 中の子の失敗原因を `recordChildFailure` で記録しておき、`Terminated(ref)` 到着時に記録があれば `ChildFailed(ref, cause)`、なければ `Terminated(ref)` を生成して signal ハンドラへ渡す。
- **PTY-29.** user guardian は `GuardianStartupBehavior` が `Start` 到達までメッセージを stash（容量 1000、超過は `StashOverflowException`）し、guardian の停止時は `GuardianStopInterceptor` が `system.terminate()` を呼んで Coordinated Shutdown を発火する。

### 1.15 Receptionist / delivery

- **PTY-8.** Receptionist のプロトコルは `Register` / `Deregister` / `Subscribe` / `Find` と応答 `Registered` / `Deregistered` / `Listing`。登録者・購読者とも `watchWith` で監視され、終了時に自動 deregister / 購読解除される（「登録解除は参照先のライフサイクル終了によっても暗黙に起こる」と API doc に明記）。
- **PTY-9.** delivery は consumer 駆動の at-least-once: `Delivery(message, confirmTo)` を受けた consumer が `Confirmed` を返すまで producer は未確認メッセージを保持・再送する。喪失検出・再送・重複排除は consumer 側の demand が駆動する。`DurableProducerQueue` は confirmed seqNr マップと未確認列を永続化してクラッシュ後再送を支える。`WorkPullingProducerController` は Receptionist の ServiceKey でワーカーを動的発見し、**順序保証なし**でルーティングする。
- **PTY-30.** delivery の既定値（`pekko.reliable-delivery`）: consumer 側は `flow-control-window = 50`（**半分消費した時点**で次ウィンドウを要求）/ `resend-interval-min = 2s`・`max = 30s`（1.5 倍ずつの指数バックオフ、正常受信でリセット）/ `only-flow-control = false`（true にすると喪失再送を放棄しフロー制御のみになる）。producer 側は `durable-queue.request-timeout = 3s` / `retry-attempts = 10`（超過で actor 停止）/ `resend-first-interval = 1s`（未確認先頭の周期再送。先頭確認でキャンセル）。ConsumerController は期待 seqNr との一致で配送し、小さい seqNr（再送重複）は受理、飛んだ seqNr は無視して `Resend` を要求する。

## 2. 不変条件

- **INV-PTY-1**: `Terminated` シグナルの未処理は必ず `DeathPactException` になる（PTY-6）。
- **INV-PTY-2**: restart を跨いで古い behavior 状態が残ることはない（PTY-5 の Deferred 再評価）。
- **INV-PTY-3**: Receptionist の Listing に終了済み参照が残り続けることはない（watchWith 自動除去、PTY-8）。
- **INV-PTY-4**: typed actor が classic の意味でリモートデプロイされることはない（`Deploy.local` 強制、PTY-15）。
- **INV-PTY-5**: 同一とみなせる interceptor によって behavior スタックが無限に成長することはない（`isSame` 重複除去、PTY-16）。
- **INV-PTY-6**: Topic のノード間転送によって同一ノードの購読者へ二重配信されることはない（インスタンス経由の再配信、PTY-22）。
- **INV-PTY-7**: stale なタイマーメッセージが現在の behavior に作用することはない（generation / owner 照合、PTY-26）。

## 3. 参照

- `Behavior.scala:183-306`（canonicalize / interpret / interpretSignal）、`internal/BehaviorImpl.scala:29-172`、`internal/Supervision.scala:45-421`、`internal/adapter/ActorAdapter.scala:93-335`（Terminated → ChildFailed 変換: 93-101 / UnstashException 処理: 152-162）、`internal/receptionist/LocalReceptionist.scala`、`delivery/{ProducerController,ConsumerController,DurableProducerQueue,WorkPullingProducerController}.scala`
- `MessageAndSignals.scala:20-125`（Signal 階層）、`scaladsl/ActorContext.scala:70-383`（API とスレッド契約）、`Props.scala:35-286`、`internal/adapter/PropsAdapter.scala:31-58`（Deploy.local）、`BehaviorInterceptor.scala:34-179` + `internal/InterceptorImpl.scala:106-236`（isSame 重複除去）
- `scaladsl/StashBuffer.scala:45-173` + `internal/StashBufferImpl.scala:40-290`（UnstashException）、`scaladsl/AskPattern.scala:85-176`、`SpawnProtocol.scala:41-86`、`pubsub/Topic.scala:35-126` + `internal/pubsub/TopicImpl.scala:41-154`
- `scaladsl/Routers.scala:35-190` + `internal/routing/{RoutingLogic,PoolRouterImpl,GroupRouterImpl}.scala`、`ActorRefResolver.scala:20-108`、`Extensions.scala:27-187`、`eventstream/EventStream.scala:36-74`、`internal/TimerSchedulerImpl.scala:94-189`、`internal/adapter/{ActorSystemAdapter,ActorRefAdapter,ActorContextAdapter,GuardianStartupBehavior}.scala`、`actor-typed/src/main/resources/reference.conf:70-131`（reliable-delivery 既定値）
