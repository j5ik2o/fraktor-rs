# プロダクト概要
> 最終更新: 2025-11-08

cellactor-rs は Akka/Pekko および protoactor-go のライフサイクル設計を Rust の no_std 環境へ移植し、標準環境（Tokio など）とも同一 API で運用できるアクターランタイムです。DeathWatch を強化した監視 API、system mailbox による厳格なライフサイクル順序、EventStream/DeadLetter の可観測性を兼ね備え、埋め込みボードからホスト OS まで一貫したデプロイを実現します。

## コア機能
- **ライフサイクル指向の ActorSystem**: `SystemMessage::Create/Recreate/Failure` を先行処理し、SupervisorStrategy と組み合わせて deterministic な再起動/停止を保証します。
- **強化 DeathWatch**: `watch/unwatch` と `spawn_child_watched` を通じて監視登録と子生成を一括管理し、停止済み PID でも即時 `on_terminated` を配送して復旧を閉じ込めます。
- **EventStream & Telemetry**: ログ、DeadLetter、ライフサイクルイベントを低遅延バスで公開し、`LoggerSubscriber` や独自サブスクライバで観測できます。
- **Typed/Untyped 並存 API**: `TypedActor` が `into_untyped/as_untyped` で Classic API と相互運用し、型安全なビヘイビア切替と `reply_to` パターンを両立します。
- **Toolbox & Runtime 分離**: `cellactor-utils-core` の `RuntimeToolbox` 抽象で割り込み安全な同期原語を提供し、`actor-std` で Tokio 実行器やホストログへのバインディングを後付けできます。

## ターゲットユースケース
- Akka/Pekko/Proto.Actor のデザインを Rust へ移植しつつ、ミッションクリティカルな復旧ポリシーを維持したい分散アプリケーション。
- RP2040 などの `thumbv6/v8` 系マイコンや `embassy` ベースの no_std 環境で、同一コードパスのアクターシステムを走らせたいファームウェア/RTOS プロジェクト。
- EventStream と DeadLetter メトリクスを軸に、ホスト（Tokio）側でログ集約・監視を行う観測性重視の制御平面。

## 価値提案
- **一貫性**: `actor-core` と `actor-std` を分離し、同じアクター API を no_std / std のどちらでも再コンパイルのみで再利用可能。
- **復旧容易性**: DeathWatch 強化と `SystemMessage` 優先度により、監視通知と SupervisorStrategy をシンプルに合成できる。
- **観測性即応**: EventStream/DeadLetter と LoggerSubscriber により、RTT/UART からホストログまで最小構成で配信。
- **移行ガイド付き**: `docs/guides/actor-system.md` や `death_watch_migration.md` が Akka/Pekko からの移行パターンを明文化し、段階的な導入を支援。
- **設計参照の透明性**: `references/protoactor-go` / `references/pekko` を一次資料にしており、既知のパターンを Rust 流儀へ変換する指針が共有されている。

---
_AI ランタイムが意思決定するときに必要な目的と価値を記述し、詳細仕様は各 specs に委ねます。_
