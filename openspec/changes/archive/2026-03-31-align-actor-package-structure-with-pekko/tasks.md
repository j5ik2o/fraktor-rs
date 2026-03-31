## 1. 変更土台の確立

- [x] 1.1 proposal / spec / design を確認し、kernel 新 package・typed dsl/internal/eventstream の目標境界と移設対象型一覧を固定する
- [x] 1.2 移設対象型の現在の参照箇所を `grep` で全列挙し、移設前の参照マップを作る（TypedActorRef・routing/*・scheduler/*・Behaviors 等）
- [x] 1.3 実装開始時の運用として、file move / mod wiring ごとに `./scripts/ci-check.sh ai dylint` を実行する手順を作業順へ組み込む

### 1.3 で固定する実装順

1. 対象タスクで作る package と `mod` 宣言だけを先に追加する
2. 1 責務ずつ file move する
3. file move 直後に `./scripts/ci-check.sh ai dylint` を実行する
4. `pub use` / `use` / `mod` / import path などの mod wiring を行う
5. mod wiring 直後に `./scripts/ci-check.sh ai dylint` を実行する
6. tests / examples 追随が必要な場合は、その更新直後にも `./scripts/ci-check.sh ai dylint` を実行する
7. 1 タスク内で複数責務をまとめて動かさず、次の責務へ進む前に直近の `./scripts/ci-check.sh ai dylint` 成功を確認する

## 2. kernel 新 package の確立

- [x] 2.1 `kernel/util/` ディレクトリと `kernel/util.rs` を新設し、`kernel.rs` に `pub mod util;` を追加する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 2.2 `messaging/byte_string.rs` を `util/byte_string.rs` へ移動し、`util.rs` に `pub mod byte_string;` と `pub use byte_string::ByteString;` を追加する。`messaging.rs` から `byte_string` の `pub mod` / `pub use` を削除し、参照箇所の import path を `kernel::util::ByteString` へ更新する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 2.3 `kernel/io/` ディレクトリと `kernel/io.rs` を新設（stub）し、`kernel.rs` に `pub mod io;` を追加する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 2.4 `kernel/routing/` ディレクトリと `kernel/routing.rs` を新設（stub）し、`kernel.rs` に `pub mod routing;` を追加する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 2.5 `kernel/actor/setup/` ディレクトリと `actor/setup.rs` を新設し、`actor.rs` に `pub mod setup;` を追加する。`system/actor_system_config.rs` の設定型を `setup/` へ移設または再 export する。完了後に `./scripts/ci-check.sh ai dylint` を実行する

## 3. typed/dsl/ package の新設と routing / scheduler 吸収

- [x] 3.1 `typed/dsl/` ディレクトリと `typed/dsl.rs` を新設し、`typed.rs` に `pub mod dsl;` を追加する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 3.2 `behaviors.rs`・`failure_handler.rs`・`fsm_builder.rs` を `typed/dsl/` へ移動し、`dsl.rs` と `typed.rs` の mod wiring を更新する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 3.3 `stash_buffer.rs`・`status_reply.rs`・`status_reply_error.rs`・`supervise.rs` を `typed/dsl/` へ移動し、mod wiring を更新する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 3.4 `timer_key.rs`・`timer_scheduler.rs`・`typed_ask_error.rs`・`typed_ask_future.rs`・`typed_ask_response.rs` を `typed/dsl/` へ移動し、mod wiring を更新する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 3.5 `typed/routing/` 配下の全ファイルを `typed/dsl/` へ移動し、`dsl.rs` に追加宣言する。`typed.rs` から `pub mod routing;` と `routing.rs` を削除する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 3.6 `typed/scheduler/` の公開 API（`TimerScheduler` facade）を `typed/dsl/timer_scheduler.rs` へ統合する（既存 `timer_scheduler.rs` の移設と合わせて対応）。完了後に `./scripts/ci-check.sh ai dylint` を実行する

## 4. typed/internal/ package の新設と scheduler 実装吸収

- [x] 4.1 `typed/internal/` ディレクトリと `typed/internal.rs` を新設し、`typed.rs` に `pub(crate) mod internal;` を追加する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 4.2 `behavior_runner.rs`・`typed_actor_adapter.rs`・`receive_timeout_config.rs` を `typed/internal/` へ移動し、`internal.rs` に `mod` 宣言を追加する。`typed.rs` から旧 `mod` 宣言を削除する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 4.3 `behavior_signal_interceptor.rs` を `typed/internal/` へ移動し、`internal.rs` に宣言を追加する。`typed.rs` から旧 `pub use BehaviorSignalInterceptor;` を削除する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 4.4 `typed/scheduler/` の内部実装（`scheduler_context.rs`・`typed_scheduler_guard.rs`・`typed_scheduler_shared.rs`）を `typed/internal/` へ移動し、`typed.rs` から `pub mod scheduler;` と `scheduler.rs` を削除する。完了後に `./scripts/ci-check.sh ai dylint` を実行する

## 5. typed/eventstream/ package の新設

- [x] 5.1 `typed/eventstream/` ディレクトリと `typed/eventstream.rs` を新設し、`typed.rs` に `pub mod eventstream;` を追加する。`EventStream` 型を Pekko の `EventStream.scala` を参照して実装する。完了後に `./scripts/ci-check.sh ai dylint` を実行する

## 6. TypedActorRef の root 昇格と typed root 公開面の最終整理

- [x] 6.1 `typed/actor/actor_ref.rs` の `TypedActorRef` を `typed/actor_ref.rs` へ昇格させる。`typed/actor/actor_ref.rs` に一時的な `pub use crate::core::typed::TypedActorRef;` を残し、参照箇所を追随させてから削除する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 6.2 `typed.rs` の `pub mod` 宣言と `pub use` を見直し、root 公開面が基盤型のみになっていることを確認する。不要な `mod` 宣言が残っていれば削除する。完了後に `./scripts/ci-check.sh ai dylint` を実行する

## 7. std/typed・tests・examples の追随

- [x] 7.1 `modules/actor/src/std/typed.rs` と `std/typed/` 配下の import path を `dsl::` / `internal::` 経由に更新する。更新直後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 7.2 `modules/actor` 内の tests で旧 import path（routing::*、scheduler::*、typed::Behaviors 等）を新 import path へ更新する。更新直後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 7.3 examples / showcases で旧 import path を使っている箇所を新 import path へ更新する。更新直後に `./scripts/ci-check.sh ai dylint` を実行する

## 8. 最終検証

- [x] 8.1 `./scripts/ci-check.sh ai all` を実行し、全テスト・全 lint が通過することを確認する
