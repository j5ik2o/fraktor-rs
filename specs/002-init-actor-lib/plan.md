# 実装計画: Cellactor Actor Core 初期実装

**ブランチ**: `[002-init-actor-lib]` | **作成日**: 2025-10-28 | **参照スペック**: specs/002-init-actor-lib/spec.md  
**入力資料**: `specs/002-init-actor-lib/spec.md` に定義された機能仕様

## 概要

- ActorSystem にスコープ制約を課し、Rust の所有権で `ActorRef` を外部へ漏らさない仕組みを確立する。protoactor-go の `RootContext` と Apache Pekko Typed の `ActorSystem` を比較し、Rust では RAII とライフタイム境界で再構成する。
- Props/Behavior ビルダをチェーン API として設計し、状態初期化、メールボックス種別、監視ポリシー、メトリクス設定を組み合わせられるようにする。`Handle`/`Untyped` 命名は禁止し、新名称（例: `ScopeRef`, `ErasedMessageEnvelope`）を提示する。
- Mailbox/Dispatcher は `modules/utils-core` の queue 抽象を基盤にし、バウンデッド/アンバウンデッド、バックプレッシャーポリシー（保留・最新破棄・最古破棄・拡張・ブロック）を切り替え可能にする。`OverflowPolicy::Block` は HostAsync モードと `AsyncQueue` を組み合わせた場合のみ許可し、CoreSync では構成時に拒否・代替案提示を行う。Mailbox は Suspend/Resume をサポートし、SystemMessageQueue/UserMessageQueue を分離した上でシステムメッセージを常に優先する。公平性メトリクスを exposing し、no_std 環境で portable-atomic ベースのスケジューラを組む。
- DispatcherRuntime と MessageInvoker の責務を切り分け、Dispatcher がワーカー割当とスケジューリング、MessageInvoker が mailbox からの取り出しとハンドラ実行・backpressure 伝搬を担う構成にする。DispatcherRuntime は常に複数ワーカー（少なくとも 2 スレッド）を前提としたスレッドプールで動作させ、単一スレッド実装を想定しない。
- ReadyQueueCoordinator と Throughput ヒントの往復を明確化し、Mailbox Middleware チェインおよび Stash 制御を組み込んだテレメトリ連携（投入件数、Suspend Duration、予約枠消費）を最小限のオーバーヘッドで提供する。
- ExecutionRuntime とそのレジストリを導入し、CoreSync 実装をデフォルト提供しつつ HostAsync など追加ランタイムを差し替え可能にする。利用者がイベントループや DispatcherRuntime を直接管理する必要がない設計を保証する。
- ActorError と Supervision 戦略のデータモデルを定義し、再試行ポリシー・時間窓・重篤度を保持。判定器 API で Restart/Stop/Resume/Escalate を外部設定できるようにし、観測チャンネルへ通知する。
- EventStream を購読・解除・バックプレッシャーヒント付きで提供し、Dispatcher/メールボックスと整合した観測指標を quickstart とテストで示す。

## 技術コンテキスト

**言語/バージョン**: Rust nightly (`rust-toolchain.toml` 固定)。`core` と `alloc` を既定で使用し、`std` は検証用クレート・テストに限定。  
**主要依存関係**: `alloc`, `portable-atomic`, `heapless`, `modules/utils-core` の `Shared`/`AsyncMutexLike` 抽象、`modules/actor-core` 内の既存テストヘルパー。  
**準標準ライブラリ利用方針**: `core::future`, `core::task`, `alloc::collections::{VecDeque,BinaryHeap}`、`core::sync::atomic`（+ `portable-atomic`）。`std` の `Mutex` や `Arc` は利用せず `Shared` 系で代替。  
**ストレージ**: 組込み向け揮発メモリを前提としたヒープ（`alloc` 管理）。持続ストレージは扱わない。  
**テスト**: `./scripts/ci-check.sh all`（Cargo fmt/build/test + カスタムLint）と `makers ci-check -- dylint` を各フェーズで実行。統合テストは `modules/actor-core/tests.rs` に配置。  
**対象プラットフォーム**: RP2040/RP235x（no_std）、Linux/macOS ホスト（std 検証）。  
**プロジェクト種別**: マルチクレート。コア実装は `modules/actor-core`、周辺抽象は `modules/utils-core` に依存。  
**性能目標**: スペック SC-001〜SC-005 を満たし、メッセージ 95% を 5ms 以内で処理。バックプレッシャー時のドロップ率 0%。  
**制約**: コアは `#![no_std]`、`tokio` や `embassy` は別クレートへ隔離。`Shared` 命名規約順守、`Untyped`/`Handle` 禁止。循環参照を避け、プラガブルな抽象を用意。  
**スケール/スコープ**: スペック P1 ユーザーストーリー（ActorSystem 起動、メールボックス制御、Supervision）を第1スプリントで実装し、EventStream/観測は同バッチで最小可動を提供。

**既存調査対象**:  
- protoactor-go: `actor/root_context.go`, `actor/props.go`, `mailbox/*`, `actor/supervision.go`, `eventstream/event_stream.go`
- Apache Pekko Typed: `akka.actor.typed.ActorSystem`, `PropsAdapter`, `MailboxSelector`, `Supervision`, `EventStream`
- `modules/utils-core`: `collections::queue`, `collections::stack`, `sync::Shared` 実装

**未確定事項**: *(研究フェーズで解消済み。詳細は `research.md` 参照)*

## 憲章チェック

- **ゲートP1（原則1）**: `modules/actor-core` は `#![no_std]` を維持し、共有参照は `Shared` 系で実装。ActorRef/Scope 管理は `Shared` を多用せずライフタイムで制御し、必要な場合のみ `ArcShared` を利用。トランスポート層は抽象 trait とし、具体的プロトコルに固定しない。  
- **ゲートP2（原則2）**: 各ユーザーストーリーの着手時に失敗する統合テストを `modules/actor-core/tests.rs` に追加し、進行中も `./scripts/ci-check.sh all` と `makers ci-check -- dylint` を毎ステップで実行する計画。テストの無効化は禁止。  
- **ゲートP3（原則3）**: 研究フェーズで protoactor-go / Pekko の対応箇所を調査し、差分を `research.md` と plan の各節に記録。乖離理由を quickstart と contracts に反映させる。  
- **ゲートP4（原則4）**: 新規型は 1 ファイル 1 型方針で配置。`modules/actor-core/src/actor_system.rs` 等を個別に追加し、テストは `modules/actor-core/tests/actor_system/tests.rs` のように分割。`mod.rs` は作らない。  
- **ゲートP5（原則5）**: 破壊的変更は初期実装のため既存 API 影響なし。将来の互換性方針を spec/plan/tasks に記載し、変更箇所が増えた場合は proposal を作成。  
- **ゲートP6（原則6）**: 着手前に `modules/utils-core` の queue/sync 実装と既存サンプルを読み込み、同一抽象を利用する。乖離が必要な場合は理由と参照ファイルを research.md/plan.md に追記する。

## プロジェクト構成

```text
specs/002-init-actor-lib/
├── plan.md          # 本ファイル
├── research.md      # フェーズ0成果（未作成）
├── data-model.md    # フェーズ1成果（未作成）
├── quickstart.md    # フェーズ1成果（未作成）
├── contracts/       # フェーズ1成果（未作成）
└── tasks.md         # /speckit.tasks 出力予定
```

```text
modules/
├── actor-core/
│   ├── src/
│   │   └── lib.rs (既存、将来は actor_system.rs などを分割追加)
│   └── tests/ (新規追加予定)
└── utils-core/
    ├── src/
    └── tests/
```

**構成決定**: コアロジックは `modules/actor-core` の新規モジュールへ分割配置し、`actor_system.rs`, `behavior.rs`, `mailbox.rs`, `dispatcher.rs`, `supervision.rs`, `event_stream.rs`, `error.rs` などを想定。各モジュールのテストは `modules/actor-core/tests/<module>/tests.rs` に分離する。ホスト専用補助は別クレートへ。

## 複雑度トラッキング

| 違反項目 | 必要な理由 | 却下した単純案 |
|----------|------------|----------------|
| *(なし)* |            |                |
