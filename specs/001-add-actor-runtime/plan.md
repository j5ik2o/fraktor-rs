# 実装計画: セルアクター no_std ランタイム初期版

**ブランチ**: `001-add-actor-runtime` | **作成日**: 2025-10-29 | **参照スペック**: specs/001-add-actor-runtime/spec.md
**入力資料**: `/specs/001-add-actor-runtime/spec.md` に定義された機能仕様

**備考**: ライフタイム優先・未型付けメッセージ方針を守りつつ、protoactor-go / Apache Pekko の設計パターンを no_std 向けに移植する初期リリース。

## 概要

- `AnyMessage` による未型付けメッセージ配送を実装し、`ActorRef`/`ActorSystem` での Ping-Pong サンプルを no_std + alloc 環境で動作させる。
- Supervisor 戦略（OneForOne / AllForOne）と Deadletter + EventStream を備え、Recoverable/Fatal エラーと panic 非介入ポリシーを明文化する。  
- ライフタイム重視・アロケーション最小化を貫き、ヒープ確保発生箇所を計測・文書化。  
- 64KB RAM 制約下で 1,000 msg/s を処理する性能検証、panic 非介入時の運用フローを quickstart で案内。  
- Mailbox については Spec FR-019〜FR-021 を満たすため、初期実装で `DropOldest` を既定としつつ `DropNewest`/`Grow` の検証を行い、`Suspend`/`Resume` 制御および System / User 優先度キューを提供する設計を確定する。`Block` は将来機能だが、ブロッキングなしで背圧を伝播できる待機ハンドラ抽象を先行で用意する。  
- Spec FR-027 に基づき、Mailbox の Bounded/Unbounded 戦略とメモリ監視・警告イベント発火をサポートする。  
- メッセージ所有は `AnyMessage` + `ArcShared` に統一し、Mailbox が所有→借用の変換（`AnyMessageView`）を担当する。  
- System/User の 2 本キュー（`AsyncQueue` バックエンド）と WaitNode ベースの Block 待機戦略を採用し、Busy wait を禁止する。外部 API は同期呼び出しのまま維持し、Dispatcher 内部で協調ポーリングする軽量 executor を用意して `async fn` 依存を回避する。  
- `std` フィーチャ有効時の検証として、examples 配下に TokioExecutor を用いた Ping/Pong サンプルを追加し、Executor 抽象を通じて ActorSystem が Tokio ランタイム上でも動作することを確認する。Tokio 依存は examples の Cargo マニフェストに限定し、コアクレートは no_std 方針を維持する。
- Spec FR-023 に基づき、親アクターが子アクターを生成・監督する API（Props 継承、`Context::spawn_child`）を整備し、Supervisor ツリー操作と EventStream 通知をサポートする。
- Spec FR-024 に基づき、ActorSystem 初期化時にユーザガーディアン Props を必須とし、エントリポイントからアプリケーションツリーを構築するフローを quickstart と data-model に落とし込む。
- Spec FR-025 に基づき、アクター命名の一意性と自動生成ロジック（親スコープごとの NameRegistry）を実装し、名前から PID への逆引きを提供する。
- EventStream には Spec FR-022 に基づき Logger 購読者を標準提供し、ログレベル/発生元 PID/タイムスタンプを含む `LogEvent` を publish → UART/RTT/ホストログへ転送できるようにする。
- 将来の Typed レイヤーやクラスタリング拡張を見据え、差し替え可能な Dispatcher/Mailbox トレイト境界を公開する。
- Spec FR-026 に基づき、MessageInvoker にミドルウェアチェーンを差し込める構造を用意し、初期リリースでは空チェーンだが拡張可能な API を設計する。
- Spec FR-028 に基づき、Dispatcher/MessageInvoker にスループット制限（1 ターン当たりのメッセージ処理数）を導入し、Props/Mailbox 設定で構成可能にする。
- Spec FR-029 に基づき、`sender()` を廃止し、メッセージ設計で `reply_to: ActorRef` パターンを必須とする。API ガイドおよび quickstart にその利用方法を記載。

## 技術コンテキスト

**言語/バージョン**: Rust 1.81 (stable) + nightly toolchain fallback（`no_std` 機能確認用）
**主要依存関係**: `portable-atomic`, `portable-atomic-util`, `alloc`, `heapless`, `modules/utils-core::AsyncQueue`; 参照実装として `references/protoactor-go`, `references/pekko`。ホスト向けサンプルでは `tokio`（`rt-multi-thread`, `macros`）を examples スコープで利用し、コアクレートへの伝播を避ける。
**ストレージ**: SRAM 64KB クラスの組込みデバイス。メッセージバッファは AsyncQueue / ヒープ再利用で管理。
**`no_std` 実装注意点**: `vec!` マクロ使用時は `use alloc::vec;` が必須。`const fn` はコンパイル時評価可能な関数に積極適用。参照渡し（`&T`）でクローン回避を優先し、ドキュメントコメントには `# Errors` / `# Panics` セクションを必ず記載。
**テスト**: 各フェーズでは対象範囲のユニット／統合テストを優先し、`./scripts/ci-check.sh all` と `makers ci-check -- dylint` は全タスク完了後の最終確認時にまとめて実行する。ホスト検証は `cargo test --no-default-features`（std フィーチャを使わない確認用）、組込み検証は `cargo test --target thumbv7em-none-eabihf`（panic=abort）。
**対象プラットフォーム**: RP2040 / RP235x / Cortex-M33、ホスト Linux/macOS (シミュレーション用)。
**プロジェクト種別**: マルチクレート (`modules/actor-core`, `modules/utils-core`, 後続で `modules/actor-std` 等)。
**性能目標**: 起動→初回処理 <5ms（ホスト）/<20ms（組込み）、1,000 msg/s でバックログ <=10、ヒープ確保 0〜5 回/秒以内。
**制約**: `modules/*-core` は `#![no_std]`; `tokio`/`embassy` は各 std/embedded クレートに隔離。`panic!` はランタイム非介入。Mailbox は同期（割込み安全）/非同期（std, embassy）両対応が可能な trait 設計とし、`async fn` 汚染を最小化する（FR-019〜FR-021 の要件を満たす構造）。  
**スケール/スコープ**: 単一ノード・シングルプロセスのローカルアクター、Typed レイヤーは後続フェーズ。
**未確定事項**: FR-020 で定義する `Block` ポリシーの背圧 API と FR-021 の `Suspend` / `Resume` 制御、FR-019 の System/User 優先度実装方式（2 本キュー or priority queue）、FR-023 の子アクター生成 API と FR-024 のユーザガーディアン Props 初期化フロー、FR-025 の命名規則（許容文字・長さ・自動命名プレフィックス）、FR-026 のミドルウェアチェーン API のインターフェイス、FR-027 の Bounded/Unbounded 切替ポリシー詳細と警告基準、FR-028 のスループット制限デフォルト値と構成方法、FR-029 の標準メッセージスキーマ指針（`reply_to` の型や必須性）、FR-030 の Ask 完了フックの具体的実装。Phase 0 でハンドラ抽象の要件を調査する。

## 憲章チェック（着手前）

- **P1 no_std コア**: `modules/actor-core` を `#![no_std]` 維持。共有資源は `Shared` 抽象で統一し、`alloc::sync::Arc` 直接使用禁止。Mailbox/Dispatcher は trait + ジェネリクスで循環参照を避ける。
- **P2 テスト完全性**: 各ユーザーストーリーのテストを red→green サイクルで実装。`./scripts/ci-check.sh all` と `makers ci-check -- dylint` は全タスク完了後にまとめて走らせ、途中では対象範囲のテストとローカル検証に留める。
- **P3 リファレンス整合**: protoactor-go, Pekko の対象機能（メールボックス、Supervisor、EventStream）を調査し差分を research.md に記録。Rust 固有制約は rationale として残す。
- **P4 モジュール構造**: 1 ファイル 1 型/1 trait、`mod.rs` 非使用、`tests.rs` 分離。コード生成時に CI lint で確認。
- **P5 攻めの設計**: 破壊的 API 追加（AnyMessage ベース）であるが後方互換前提がないため許容。proposal 済み（本スペック）。
- **P6 帰納的一貫性**: 既存 utils-core のキュー・共有ポインタ実装を調査し、命名や trait 構造を踏襲。差異は plan/research に明記。
- **P7 ライフタイム優先**: 借用 API を徹底し、アロケーション箇所は計測・再利用戦略を FR-017/SC-005 に沿って設計。`AnyMessage` は借用ベースの API を構築する。

## プロジェクト構成

```text
specs/001-add-actor-runtime/
├── plan.md
├── research.md        # フェーズ0
├── data-model.md      # フェーズ1
├── quickstart.md      # フェーズ1
└── tasks.md           # /speckit.tasks で生成予定
```

リポジトリ基盤は既存の `modules/actor-core`, `modules/utils-core` を中心に拡張する。ホスト固有コードは `modules/actor-std` などの別クレートへ隔離し、`no_std` コアを汚染しない。

## フェーズ0: リサーチ計画

1. `protoactor-go` の Mailbox / Dispatcher / Supervisor / Child actor API (`Context.Spawn`/RootContext) と命名ルール (`ProcessRegistry`)、MiddlewareChain (ProcessStage)、Bounded/Unbounded mailbox、Dispatcher throughput 設定、ProcessMailbox のメッセージ所有モデルを調査し、借用・アロケーション挙動、および `Drop`/`Grow` 相当のキュー戦略を整理。  
2. Apache Pekko の `EventStream` / `DeadLetter` / Supervision ドキュメントを参照し、Recoverable/Fatal ハンドリング差分を抽出。
3. `AnyMessage` の借用ベース設計に適した Rust パターン（`dyn Any` + `RefCell` 非使用）を調査。
4. AsyncQueue の容量・バックプレッシャー戦略（BoundedMailbox 相当）と `DropNewest`/`Grow`/`Block` の遷移条件、`Suspend`/`Resume` の制御手段（同期・非同期双方）および System/User デュアルキュー構成、Bounded/Unbounded 切替時のメモリ監視指標、Block 待機フロー、スループット制限適用方法、`AnyMessage` の所有先（ArcShared）を決定。ActorSystem のライフサイクル API（`terminate()`, `when_terminated()`, `run_until_terminated()`）の振る舞いもこの段階で設計する。
5. panic 非介入ポリシー時の運用ベストプラクティス（ウォッチドッグリセットなど）を確認し quickstart に反映。

成果物: research.md（Decision / Rationale / Alternatives 形式）で全 NEEDS CLARIFICATION を解消。

## フェーズ1: 設計 & ドキュメント

前提: research.md 完了。

1. data-model.md: ActorSystem / ActorCell / ActorContext / AnyMessage / SupervisorStrategy / ActorError のエンティティ、属性、関係性を定義し、Mailbox ポリシー `DropNewest` / `DropOldest` / `Grow` / `Block` と `Suspend` / `Resume` 制御、System/User 優先度の状態遷移・トレイトフック、および親子アクターのツリー構造と伝播規則（ユーザガーディアンを含む）、NameRegistry、MessageInvoker ミドルウェアチェーンの拡張ポイント、Bounded/Unbounded 戦略とスループット制限の設定点を整理。
2. contracts/: Ping/Pong サンプル用エンドポイント（例: 制御インターフェイス）を OpenAPI で定義し、ActorSystem 構成 API の最小セットを記述。  
3. quickstart.md: no_std ボード + ホスト実行の手順、panic 非介入時の対応、計測方法（ヒープ確保計測・1,000 msg/s テスト）、Mailbox ポリシー切り替え手順、EventStream Logger 購読者の設定例、ユーザガーディアン Props を渡してエントリポイントから子アクターを spawn しつつ `system.user_guardian_ref().tell(Start)` でブートストラップするコード例を記載。actor-core 配下の `examples/ping_pong_no_std` は `std` フィーチャを有効にして実行し、`ctx.self_ref()` を payload に埋め込んだ `reply_to` の扱いや、ガーディアンが `ctx.stop(ctx.self_ref())` を呼ばない限り ActorSystem が継続する点を明記する。  
4. `.specify/scripts/bash/update-agent-context.sh codex` を実行し、Codex 専用コンテキストに AnyMessage/ActorError ポリシー・Mailbox ポリシー設計・EventStream Logger・ユーザガーディアン構成・panic 非介入を追記。  
5. 憲章ゲート再評価（P1〜P7）。設計で新たに発生したリスクがあれば複雑度トラッキングに記録。

## フェーズ2: 実装準備 & テスト設計

1. Modules: `modules/actor-core` に ActorRef / ActorCell / ActorSystem スケルトン、Supervisor 戦略 trait、Mailbox trait を追加する設計メモを作成。Mailbox trait にはポリシー切り替え用の `on_full(policy)` フックと将来の `Block` 専用ハンドラ（反復的ポーリング / Waker 相当）、さらに `suspend()` / `resume()` API、System/User 優先度キューの API（例: `enqueue_system`/`enqueue_user`）を備え、同期/非同期環境いずれでも安全にデータ流を停止・再開できる設計を確保。
2. テスト: ユーザーストーリー毎に red テストケース（Ping/Pong、Supervisor 再起動、イベント購読）と `DropNewest` / `DropOldest` / `Grow` / `Suspend` / `Resume` / System vs User 優先度 / 子アクター監視 / 名前重複検出・自動命名 / ミドルウェアチェーン挿入 / Bounded vs Unbounded 切替 / スループット制限の挙動、および `ActorSystem::drain_ready_ask_futures()` を通じた ask 完了確認を tasks.md に反映。
3. 効率: アロケーション計測用の `no_std` 計測フックと、AsyncQueue 容量設定・`Grow` による拡張時の計測プラン、Bounded/Unbounded 切替時のメモリモニタ、スループット制限のプロファイルを用意。  
4. 運用: Deadletter/EventStream の Subscribe API と quickstart のサンプルコード尖兵を準備。Logger 購読者が `LogEvent` を UART/RTT/ホストログへ転送する例を用意。  
5. リスク: panic 非介入に伴う復旧遅延リスクを `研究結果 + quickstart` で周知し、タスクへウォッチドッグ設定を追加。将来の `Block` ポリシー向けに非同期実行（tokio/embassy）での待機戦略を検討タスクとして backlog に追加。

## 憲章チェック（設計後）

| ゲート | 状態 | 補足 |
|--------|------|------|
| P1 | ✅ | no_std 維持・Shared 抽象で実装、循環参照回避方針を data-model に反映。 |
| P2 | ✅ | 各ユーザーストーリーの red テスト計画と CI 実行手順を quickstart に明記。 |
| P3 | ✅ | research.md にリファレンス比較と差分理由を記録。 |
| P4 | ✅ | 1 ファイル 1 型 ルールと tests.rs 配置を tasks（後続）で強制予定。 |
| P5 | ✅ | 破壊的変更は本フェーズ対象。後続フェーズで proposal 管理済み。 |
| P6 | ✅ | 既存 utils-core を参照し命名/抽象整合。差異は research.md に記録。 |
| P7 | ✅ | ライフタイム優先・アロケーション計測を FR-017/SC-005 と quickstart に組み込み。 |

## 複雑度トラッキング

| 違反項目 | 必要な理由 | 却下した単純案 |
|----------|------------|----------------|
| `#[allow(clippy::needless_range_loop)]` in `actor_cell.rs::find_or_insert_stats` | 可変参照を返すパターンでループ内indexingが必須。`iter_mut().take(len)` への変更は借用チェッカーエラーを引き起こす（ループ内で可変参照を返した後、同じベクターへ `push` するため）。 | `iter_mut()` パターンは借用チェッカーエラーで却下。 |
