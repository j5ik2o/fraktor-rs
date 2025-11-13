# Scheduler Tick Driver 設計メモ

## 背景

- 現状の `SchedulerContext` は no_std の決定論テストを基点に設計されており、std 版でも `SchedulerTickHandle` へ手動で tick を注入しないとタイマーが進まない。
- `std` + `tokio-executor` 環境ではユーザが「アクターを起動したのにスケジューラだけ動かない」状況に陥りやすく、サンプルも `SchedulerRunner::manual` を直接書いている。
- 将来の `embassy` / `SysTick` などハードウェアタイマ連携も考えると、tick 供給を toolbox/driver に抽象化しておくべきフェーズに来ている。

## 目標

1. std 環境（Tokio 等）では ActorSystem 起動時に自動で scheduler tick が供給されること。
2. no_std 環境では従来どおり任意のドライバ（手動 or ハードウェア）を注入できること。
3. Runner API はテスト／デモ用途のみに縮小し、プロダクションではドライバ経由で統一された tick 流路を使うこと。

## 理想設計の骨子

### 1. Driver 抽象

- `SchedulerTickDriver` trait を追加し、`start(handle: SchedulerTickHandle<'_>, stop_token: TickStopToken)` のような API を定義。
- 代表実装
  - `ManualDriver`: 既存の `SchedulerRunner::manual` を包み、テスト＆サンプル専用。
  - `TokioDriver`: `tokio::time::interval(resolution)` を使って `handle.inject_manual_ticks(1)` を繰り返す常駐タスク。
  - 拡張ポイントとして `EmbassyDriver` や `SysTickDriver` を追加可能にしておく。

### 2. Toolbox への組み込み

- `RuntimeToolbox` に `fn tick_driver(&self) -> Arc<dyn SchedulerTickDriver>` を追加。
- `StdToolbox` は `TokioDriver`（tokio feature 有効時）を返し、`NoStdToolbox` は `ManualDriver` か、利用者が独自ドライバを注入できるように構成する。
- `StdToolbox::default()` では解像度 1ms の interval を張り、Tokio runtime に依存しない（または `tokio` feature を前提）よう注意する。

### 3. ActorSystem 起動時の駆動

- `ActorSystemGeneric::ensure_scheduler_context` で `let driver = toolbox.tick_driver(); driver.start(context.tick_source(), stop_token.clone());` のように起動。
- `SystemStateGeneric` に stop token を保持し、`ActorSystem::terminate` / drop 時に driver を停止させる。
- これにより std 版では追加コードなしで scheduler が動作し、no_std では driver 実装を入れ替えるだけで済む。

### 4. Runner / Manual API の整理

- `SchedulerRunner` は `ManualDriver` 内部に限定し、公開 API からは「テストで deterministic に動かしたい場合は `ManualDriver` を使う」導線にする。
- `RunnerMode` は `Manual` と `External`（driver 管理）に縮小し、AsyncHost/Hardware などの分岐は driver 実装に閉じ込める。

### 5. ドキュメント／サンプル更新

- `docs/guides` に「std では tick ドライバが自動で動作する」旨を明記し、no_std サンプルは `ManualDriver` を明示的にセットアップするコードへ書き換える。
- 破壊的変更になるため spec では Requirements→Design→Tasks を順守し、`tokio-executor` feature の有無や `cfg(not(feature = "tokio-executor"))` の fallback（thread ベースの driver など）も検討する。

## 次アクション（Spec 下書き案）

1. **Requirements**: `std` での自動 tick 駆動・`no_std` での driver 差し替えを必須要件として明文化。
2. **Design**: 上記 driver 構成、shutdown 手順、feature 切替を詳細化。
3. **Tasks**: (a) driver trait 追加、(b) toolbox 実装、(c) ActorSystem bootstrap/shutdown フロー変更、(d) サンプル＆ドキュメント更新、(e) 回帰テスト。

この設計を採用すれば、std/no_std ともに一貫した API で scheduler を運用でき、「std なのに tick が進まない」問題を根本的に解消できる。
