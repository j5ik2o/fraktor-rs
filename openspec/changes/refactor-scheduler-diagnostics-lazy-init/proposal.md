# 提案: Scheduler Diagnostics を遅延初期化する

**Change ID**: `refactor-scheduler-diagnostics-lazy-init`
**作成日**: 2025-11-15
**ステータス**: 提案中

## 概要

Scheduler が常に `SchedulerDiagnostics`／`Vec<SchedulerWarning>`／`VecDeque<SchedulerDiagnosticsEvent>` を保持する現行実装を見直し、観測者や deterministic log が明示的に要求されたときだけ診断系を初期化する。未使用状態では diagnostics を `None` 相当で保持し、イベント生成や `DiagnosticsDropped` 警告を完全に停止することで `no_std` でも余計なメモリを消費しない。

## 動機

1. **無観測時のメモリ常駐**: `Scheduler::new` が毎回 `SchedulerDiagnostics::with_capacity` を呼び出すため、実運用経路で診断を使っていなくても `Vec`/`VecDeque` が確保され、`no_std` ターゲットでは顕著なフットプリント増となる。
2. **意味のない警告発火**: `publish_stream_with_drop` は購読者ゼロでも `stream_buffer` に積み続け、容量超過で `SchedulerWarning::DiagnosticsDropped` を蓄積するが、観測者がいないため誰も気付かない。
3. **YAGNI の逸脱**: `SystemBase::ensure_scheduler_context` も診断込みの `SchedulerConfig` を固定で組み立てており、利用者が opt-in する手段が存在しない。必要になった時点で初期化する設計に揃えたい。

## 変更内容

- `Scheduler` に `LazySchedulerDiagnostics`（内部的には `Option<SchedulerDiagnostics>`）を導入し、`enable_deterministic_log`／`subscribe_diagnostics` が初めて呼ばれたときにだけインスタンス化する。既存の `diagnostics()` API は、診断が無効の場合は空イベントを返すラッパを提供して互換性を維持する。
- `publish_stream_with_drop`／`record_*` 系は診断インスタンスの存在をチェックし、未初期化なら即座に no-op で抜ける。これにより `DiagnosticsDropped` は diag が有効化されている時だけ意味を持つ。
- `SchedulerConfig` に `diagnostics_mode`（例: `Disabled`／`Lazy { capacity }`）を追加し、`SystemBase::ensure_scheduler_context` では `Disabled` をデフォルトにする。テストやツールからは明示的に `Lazy` を選べるようにする。
- `Scheduler::subscribe_diagnostics` が初期化時にバッファへ過去イベントをリプレイできるよう、遅延生成でも空配列を返す互換経路を実装する。必要であれば「遅延初期化前のイベントは破棄された」旨の warning を最初の購読者へ通知する。
- `SchedulerDump` や `warnings()` など診断に依存しない API は挙動を変えず、診断が無効でも呼び出し可能とする。
- 既存のテスト・サンプルを更新し、遅延初期化や opt-in 経路を通るシナリオ（購読→イベント受信、無観測→メモリ非確保）を追加する。

## 影響範囲

- `modules/actor-core/src/scheduler/` 配下全般（特に `scheduler_core.rs`, `scheduler_diagnostics*.rs`, `scheduler_config.rs`）。
- `modules/actor-core/src/system/base.rs` および `SchedulerContext` 初期化ロジック。
- `modules/actor-core/examples/scheduler_diagnostics_*` と、診断 API を前提とするテスト群。
- 必要に応じて `docs/` や `README` の診断利用手順。

## オープンな課題

1. 遅延初期化前に発生したイベントを後続購読者へ渡すべきか、それとも「観測開始前の履歴は捨てる」と明示するかを決める必要がある。
2. `Scheduler::diagnostics()` の戻り値をどう互換維持するか（`Option<&SchedulerDiagnostics>` に変えると破壊的）。薄いラッパ型でゼロコスト化できるか検証が必要。
3. `diagnostics_capacity` を `SchedulerConfig` から完全に除去するか、`Lazy` モードのパラメータとして残すかの判断。

## 承認基準

- 観測者を登録しない限り `SchedulerDiagnostics` 関連のヒープ割り当てが行われないことがプロファイルで確認できる。
- `subscribe_diagnostics`／`enable_deterministic_log` を呼び出した後は、これまで通りイベント・ログが取得できること（新旧テストがカバー）。
- `SystemBase` のデフォルト起動では診断が無効化され、診断系の public API は opt-in 経路のみで使用できる。
- `./scripts/ci-check.sh all` を通し、挙動をカバーする単体テスト＋サンプルが追加されている。
