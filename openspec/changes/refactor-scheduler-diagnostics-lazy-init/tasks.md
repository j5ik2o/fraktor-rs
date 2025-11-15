# 実装タスク: Scheduler Diagnostics 遅延初期化

1. **コンテキスト整理**
   - [ ] `modules/actor-core/src/scheduler/` と `system/base.rs` で診断が初期化されるパスを洗い出し、既存 API 互換性の制約をドキュメント化する。
   - [ ] `diagnostics_capacity` を利用しているテスト・サンプルのリストを作成し、モード分岐に置き換える計画を固める。
2. **Lazy ラッパの導入**
   - [ ] `Scheduler` に `LazySchedulerDiagnostics`（`Option` 包装＋空プロキシ）を追加し、`Scheduler::new` から即時初期化を取り除く。
   - [ ] `Scheduler::diagnostics`／`enable_deterministic_log`／`subscribe_diagnostics` をラッパ経由で動かし、未初期化時は空の参照や no-op を返すようにする。
3. **イベント／警告経路の更新**
   - [ ] `record_*`／`publish_stream_with_drop`／`record_warning` から診断の存在チェックを行い、無効時にはヒープ割り当てを避ける。
   - [ ] 初回購読者に対して「遅延初期化前のイベントが欠落する」ことを通知するか判断し、必要なら `SchedulerWarning` へ新 variant を追加する。
4. **設定と SystemBase 連携**
   - [ ] `SchedulerConfig` に `diagnostics_mode`（Disabled/Lazy{capacity}）を追加し、`SchedulerContext::new` が新パラメータを受け取るようにする。
   - [ ] `SystemBase::ensure_scheduler_context` と関連する builder で `Disabled` をデフォルトにし、テストや例では `Lazy` を選べる API を提供する。
5. **テスト・サンプル更新**
   - [ ] 診断購読テストを遅延初期化前提に書き換え、無観測パスで追加割り当てが無いことを検証する。
   - [ ] `scheduler_diagnostics_*` サンプルを opt-in 形式に更新し、README/ドキュメントへ手順を追記する。
6. **検証**
   - [ ] 影響範囲の `cargo test` を実行し、最終的に `./scripts/ci-check.sh all` を成功させる。
