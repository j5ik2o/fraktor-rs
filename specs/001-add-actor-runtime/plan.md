# 実装計画: セルアクター no_std ランタイム初期版

**ブランチ**: `001-add-actor-runtime` | **作成日**: 2025-10-29 | **参照スペック**: specs/001-add-actor-runtime/spec.md  
**入力資料**: `/specs/001-add-actor-runtime/spec.md` に定義された機能仕様

**備考**: ライフタイム優先・未型付けメッセージ方針を守りつつ、protoactor-go / Apache Pekko の設計パターンを no_std 向けに移植する初期リリース。

## 概要

- `AnyMessage` による未型付けメッセージ配送を実装し、`ActorRef`/`ActorSystem` での Ping-Pong サンプルを no_std + alloc 環境で動作させる。  
- Supervisor 戦略（OneForOne / AllForOne）と Deadletter + EventStream を備え、Recoverable/Fatal エラーと panic 非介入ポリシーを明文化する。  
- ライフタイム重視・アロケーション最小化を貫き、ヒープ確保発生箇所を計測・文書化。  
- 64KB RAM 制約下で 1,000 msg/s を処理する性能検証、panic 非介入時の運用フローを quickstart で案内。  
- 将来の Typed レイヤーやクラスタリング拡張を見据え、差し替え可能な Dispatcher/Mailbox トレイト境界を公開する。

## 技術コンテキスト

**言語/バージョン**: Rust 1.81 (stable) + nightly toolchain fallback（`no_std` 機能確認用）  
**主要依存関係**: `portable-atomic`, `portable-atomic-util`, `alloc`, `heapless`, `modules/utils-core::AsyncQueue`; 参照実装として `references/protoactor-go`, `references/pekko`  ️ 
**ストレージ**: SRAM 64KB クラスの組込みデバイス。メッセージバッファは AsyncQueue / ヒープ再利用で管理。  
**テスト**: `./scripts/ci-check.sh all`, `makers ci-check -- dylint`, `cargo test --target thumbv7em-none-eabihf`（panic=abort）, ホスト検証は `cargo test --no-default-features --features std`（テスト専用）。  
**対象プラットフォーム**: RP2040 / RP235x / Cortex-M33、ホスト Linux/macOS (シミュレーション用)。  
**プロジェクト種別**: マルチクレート (`modules/actor-core`, `modules/utils-core`, 後続で `modules/actor-std` 等)。  
**性能目標**: 起動→初回処理 <5ms（ホスト）/<20ms（組込み）、1,000 msg/s でバックログ <=10、ヒープ確保 0〜5 回/秒以内。  
**制約**: `modules/*-core` は `#![no_std]`; `tokio`/`embassy` は各 std/embedded クレートに隔離。`panic!` はランタイム非介入。  
**スケール/スコープ**: 単一ノード・シングルプロセスのローカルアクター、Typed レイヤーは後続フェーズ。  
**未確定事項**: なし（ActorError=Recoverable/Fatal、Mailbox capacity=64・閾値75% で確定）

## 憲章チェック（着手前）

- **P1 no_std コア**: `modules/actor-core` を `#![no_std]` 維持。共有資源は `Shared` 抽象で統一し、`alloc::sync::Arc` 直接使用禁止。Mailbox/Dispatcher は trait + ジェネリクスで循環参照を避ける。  
- **P2 テスト完全性**: 各ユーザーストーリーのテストを red→green サイクルで実装。CI は `./scripts/ci-check.sh all` と `makers ci-check -- dylint` を両方実行しログ添付。  
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
├── contracts/         # フェーズ1 (OpenAPI/GraphQL)
└── tasks.md           # /speckit.tasks で生成予定
```

リポジトリ基盤は既存の `modules/actor-core`, `modules/utils-core` を中心に拡張する。ホスト固有コードは `modules/actor-std` などの別クレートへ隔離し、`no_std` コアを汚染しない。

## フェーズ0: リサーチ計画

1. `protoactor-go` の Mailbox / Dispatcher / Supervisor 実装を調査し、借用・アロケーション挙動を整理。  
2. Apache Pekko の `EventStream` / `DeadLetter` / Supervision ドキュメントを参照し、Recoverable/Fatal ハンドリング差分を抽出。  
3. `AnyMessage` の借用ベース設計に適した Rust パターン（`dyn Any` + `RefCell` 非使用）を調査。  
4. AsyncQueue の容量・バックプレッシャー戦略（BoundedMailbox 相当）のチューニング値を決定。  
5. panic 非介入ポリシー時の運用ベストプラクティス（ウォッチドッグリセットなど）を確認し quickstart に反映。

成果物: research.md（Decision / Rationale / Alternatives 形式）で全 NEEDS CLARIFICATION を解消。

## フェーズ1: 設計 & ドキュメント

前提: research.md 完了。

1. data-model.md: ActorSystem / ActorCell / ActorContext / AnyMessage / SupervisorStrategy / ActorError のエンティティ、属性、関係性を定義。状態遷移図（Actor lifecycle, Supervisor decision）をテキスト整理。  
2. contracts/: Ping/Pong サンプル用エンドポイント（例: 制御インターフェイス）を OpenAPI で定義し、ActorSystem 構成 API の最小セットを記述。  
3. quickstart.md: no_std ボード + ホスト実行の手順、panic 非介入時の対応、計測方法（ヒープ確保計測・1,000 msg/s テスト）を記載。  
4. `.specify/scripts/bash/update-agent-context.sh codex` を実行し、Codex 専用コンテキストに AnyMessage/ActorError ポリシー・panic 非介入を追記。  
5. 憲章ゲート再評価（P1〜P7）。設計で新たに発生したリスクがあれば複雑度トラッキングに記録。

## フェーズ2: 実装準備 & テスト設計

1. Modules: `modules/actor-core` に ActorRef / ActorCell / ActorSystem スケルトン、Supervisor 戦略 trait、Mailbox trait を追加する設計メモを作成。  
2. テスト: ユーザーストーリー毎に red テストケース（Ping/Pong、Supervisor 再起動、イベント購読）と panic 手動シミュレーション手順を tasks.md に反映。  
3. 効率: アロケーション計測用の `no_std` 計測フックと、AsyncQueue 容量設定の検証プランを用意。  
4. 運用: Deadletter/EventStream の Subscribe API と quickstart のサンプルコード尖兵を準備。  
5. リスク: panic 非介入に伴う復旧遅延リスクを `研究結果 + quickstart` で周知し、タスクへウォッチドッグ設定を追加。

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
| なし     | -          | -              |
