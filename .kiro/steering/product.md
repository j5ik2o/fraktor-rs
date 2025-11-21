# プロダクト概要
> 最終更新: 2025-11-17

fraktor-rs は Akka/Pekko および protoactor-go のライフサイクル設計を Rust の no_std 環境へ移植し、標準環境（Tokio など）とも同一 API で運用できるアクターランタイムです。ワークスペースは `fraktor-utils-rs`（`modules/utils`）、`fraktor-actor-rs`（`modules/actor`）、`fraktor-remote-rs`（`modules/remote`）の 3 クレートで構成され、各クレートが `core`（no_std）/`std` モジュールを feature で切り替えることで、DeathWatch を強化した監視 API、system mailbox によるライフサイクル制御、EventStream/DeadLetter の可観測性、Remoting 拡張を埋め込みボードからホスト OS まで一貫した体験で提供します。

## コア機能
- **ライフサイクル制御**: `SystemMessage::Create/Recreate/Failure` を system mailbox で優先処理し、SupervisorStrategy／再起動ポリシーを deterministic に適用して actor の生成・停止シーケンスを安定化します。
- **監視と復旧閉じ込め**: `watch/unwatch`、`spawn_child_watched`、停止済み PID への即時 `on_terminated` 送達により、DeathWatch と子生成を 1 つのフローで扱い、復旧の境界を明示します。
- **観測・テレメトリ**: EventStream/DeadLetter/LoggerSubscriber を介してライフサイクル、リモート、TickDriver のイベントを低遅延で配信し、監視パイプラインへ直接流し込めます。
- **API サーフェスの二層化**: `TypedActor` と `into_untyped/as_untyped` 変換が型付き/非型付き API を橋渡しし、`reply_to` 前提のプロトコルで Classic `sender()` 依存を排除します。
- **アドレッシング & Remoting**: `ActorPathParts`/`ActorPathFormatter` が Pekko 互換 URI を生成し、`RemoteAuthorityManager` が `Unresolved/Connected/Quarantine` と遅延キューを管理してリモート隔離・復旧を統制します。
- **スケジューラ / Tick Driver**: `TickDriverBootstrap`・`SchedulerTickExecutor` と `StdTickDriverConfig::tokio_quickstart*` がハードウェア/手動/Tokio driver をテンプレ化し、`docs/guides/tick-driver-quickstart.md` でブート手順を統合します。
- **Toolbox & Runtime 分離**: `fraktor-utils-rs` の `RuntimeToolbox` が割り込み安全な同期原語・タイマを提供し、`fraktor-actor-rs` の `core`（no_std）と `std`（Tokio/ログ連携）が同一 API を別実装で差し替えます。

### モジュール別要約（`modules/actor/src/core`）
- **`actor_prim/`**: `Pid`、`ActorRef`、`ActorPathParts`、`ActorSelectionResolver` などアクター識別・アドレッシング・Typed/Untyped の橋渡しを司る基本語彙を提供。
- **`config/`**: `ActorSystemConfig`・`SchedulerConfig`・`RemotingConfig` を定義し、no_std/std 共通でライフサイクル・スケジューラ・DeathWatch 設定を束ねる。
- **`dead_letter/`**: 投入不能メッセージの保持 (`DeadLetterEntry`) と EventStream への通知を実装し、監視用 API と統合。
- **`dispatcher/`**: メールボックスとスレッド/実行器の橋渡し、`DispatchExecutor`・`DispatchShared` などの抽象をまとめて ActorRef 送達パスを標準化。
- **`event_stream/`**: `EventStreamGeneric` と `EventStreamEvent`（Lifecycle/DeadLetter/RemoteAuthority/TickDriver 等）を管理し、subscriber API を提供。
- **`extension/`**: ActorSystem への拡張ポイント登録機構を実装し、Toolbox 依存のプラグインを遅延初期化。
- **`futures/`**: `ActorFuture` と ask/reply フローのポーリング補助を no_std 向けに実装し、std 側では `ActorFuture` を `ArcShared` で保持。
- **`lifecycle/`**: SystemMessage 処理、DeathWatch、Terminated 通知などライフサイクル制御を司る。
- **`logging/`**: `LogEvent`/`LogLevel` と EventStream 発火 API を提供し、std では `tracing` 連携が可能。
- **`mailbox/`**: `Mailbox`, `MailboxScheduler` と `SystemMessage`/ユーザメッセージの優先処理ルールを実装。
- **`messaging/`**: `AnyMessageGeneric`, `MessageEnvelope`, `reply_to` など送受信ペイロードを定義し、Typed/Untyped 共用のビュー層を提供。
- **`props/`**: `PropsGeneric` とビルダー API を提供し、`spawn_child_watched` などのラッパーと連携。
- **`scheduler/`**: `Scheduler`, `TickDriverBootstrap`, `TickDriverRuntime`, `SchedulerTickExecutor` を含み、ハードウェア・手動・Tokio driver の抽象を一元化。
- **`serialization/`**: `MessageSerializer`, `SerializationRegistry` などの pluggable 仕組みを管理し、`Serde`/`postcard`/`prost` などを統合。
- **`spawn/`**: `ActorSpawner`, `SpawnError`, `NameRegistry` を実装し、Guardian 経由のアクター生成・命名ルールを提供。
- **`supervision/`**: `SupervisorStrategy`, `Decider`, `RestartPolicy` を定義し、SystemMessage 先行処理と連携して復旧挙動を制御。
- **`system/`**: `ActorSystemGeneric`, `SystemStateGeneric`, `RemoteAuthorityManagerGeneric` を実装し、全体の状態管理と EventStream 連携を集中化。
- **`typed/`**: `Behavior`, `TypedActorContext`, `TypedActorRef` を提供し、Untyped API との safe bridge (`into_untyped/as_untyped`) を担う。

## ターゲットユースケース
- Akka/Pekko/Proto.Actor のデザインを Rust へ移植しつつ、ミッションクリティカルな復旧ポリシーを維持したい分散アプリケーション。
- RP2040 などの `thumbv6/v8` 系マイコンや `embassy` ベースの no_std 環境で、同一コードパスのアクターシステムを走らせたいファームウェア/RTOS プロジェクト。
- EventStream と DeadLetter メトリクスを軸に、ホスト（Tokio）側でログ集約・監視を行う観測性重視の制御平面。

## 価値提案
- **一貫性**: `fraktor-actor-rs` が単一クレート内で `core`（default `#![no_std]`）と `std` モジュールを持ち、feature 切替だけで同じ API を no_std / std のどちらでも再利用可能。
- **復旧容易性**: DeathWatch 強化と `SystemMessage` 優先度により、監視通知と SupervisorStrategy をシンプルに合成できる。
- **リモート互換性**: Pekko/Proto.Actor と同じ actor path 体系（`fraktor` / `fraktor.tcp` スキーム、guardian 自動挿入、UID suffix）と quarantine ルールを Rust/no_std 上で再現し、異種ホストと埋め込み環境間での remoting を遮断なく延長できる。
- **観測性即応**: EventStream/DeadLetter と LoggerSubscriber により、RTT/UART からホストログまで最小構成で配信。
- **移行ガイド付き**: `docs/guides/actor-system.md`、`death_watch_migration.md`、`tick-driver-quickstart.md` が Akka/Pekko からの移行や TickDriver ブートストラップを明文化し、段階的な導入を支援。
- **設計参照の透明性**: `references/protoactor-go` / `references/pekko` を一次資料にしており、既知のパターンを Rust 流儀へ変換する指針が共有されている。

---
_AI ランタイムが意思決定するときに必要な目的と価値を記述し、詳細仕様は各 specs に委ねます。_
