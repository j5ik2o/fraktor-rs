# `Cancellable` alias 追加計画

## サマリー

`SchedulerHandle` がすでに `cancel()` / `is_cancelled()` / `is_completed()` を提供しているため、実装方針は **`pub type Cancellable = SchedulerHandle` を `core/kernel` に追加するだけ** とする。  
公開 API の戻り値型はこのタスクでは変更せず、Pekko parity 用の別名を最小追加する。`Less is more` と既存利用箇所への影響最小化を優先する。

## 変更内容

- `modules/actor/src/core/kernel/actor/scheduler.rs`
  - `pub use handle::SchedulerHandle;` の並びに `pub type Cancellable = SchedulerHandle;` を追加する。
  - alias は scheduler 公開面の最上位に置き、利用側が `crate::core::kernel::actor::scheduler::Cancellable` で参照できるようにする。
- `modules/actor/src/core/kernel/actor/scheduler/handle.rs`
  - `SchedulerHandle` の rustdoc に「Pekko parity では `Cancellable` alias として公開される」旨を追記する。
  - `cancel()` / `is_cancelled()` が Pekko `Cancellable` の対応面であることを明記する。
- ギャップ分析ドキュメント
  - 既存の `actor-gap-analysis.md` の `Cancellable alias` 項目を「実装済み」扱いに更新する。

## 公開 API / 型の扱い

- 追加する公開型:
  - `crate::core::kernel::actor::scheduler::Cancellable`（type alias）
- 変更しないもの:
  - `Scheduler::schedule_*` 系の戻り値は引き続き `SchedulerHandle`
  - `Typed` 側・`std` 側の戻り値型注釈も変更しない
  - `SchedulerHandle` 自体の名前変更や wrapper 化は行わない

## テスト計画

- `Cancellable` alias 経由で `cancel()` と `is_cancelled()` が呼べることを確認する単体テストを追加する。
- `SchedulerHandle` 既存テストはそのまま通ること。
- `schedule_once` 等が返す値を `let handle: Cancellable = ...;` の形で受けられることを確認する。
- 最後に `./scripts/ci-check.sh ai all` を実行する。

## 前提

- 公開形は `alias のみ` を採用する。
- `Facade` 命名や薄い wrapper 追加は行わない。
- 今回の目的は Pekko parity の名前補完であり、API の全面改名ではない。
- `is_completed()` は fraktor 固有の拡張として残す。
