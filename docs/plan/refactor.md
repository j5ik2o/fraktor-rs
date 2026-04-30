# actor-adaptor/std の Adapter-Only 化計画

## 要約

`modules/actor-adaptor/src/std` を「`modules/actor/src/core` が定義した契約を `std` / `tokio` / `tracing` に接続する実装だけ残す」構造へ整理する。
方針は次の 3 つです。

- 重要なロジックとドメイン概念は `core` に寄せる
- `std` は runtime integration と adapter 実装だけに絞る
- サンプル都合の facade / shim は削除し、showcase と tests も `core` API に追随させる

特に event/logging は facade をそのまま `core` へ移すのではなく、facade が担っていた能力を `core` の API に再配置し、その上で facade 群を削除する。

## この計画で確定する設計判断

- `LogEvent` の timestamp は引き続き actor system / system state 側で一元的に付与する
  - 利用側が `LogEvent` を直接構築して `publish_event` する public API は追加しない
  - actor 外からの structured logging は `ActorSystem` / `TypedActorSystem` / `TypedActorSystemLog` の public API に集約する
- `logger_name` だけを context に保持する
  - 既存の `set_logger_name` / `logger_name` は維持する
  - marker と MDC は context / system に保持しない
  - marker と MDC は各ログ呼び出しに渡す一時データとし、`set_marker` / `clear_marker` / `insert_mdc` / `clear_mdc` のような stateful API は導入しない
- marker 表現は既存の `LogEvent::with_marker` に合わせる
  - 必須なのは marker 名と properties の組であり、専用 facade 状態型は追加しない
  - dead letter marker などの convenience が必要なら、`core` logging 側に薄い helper を追加する
- `std/event/stream` shim を削除するが、`std/system/*` は adapter モジュールとして維持する
  - ただし shim に依存している import / type alias / public signature は `core` の型へ切り替える
- `TypedActorSystem` 系の surface も untyped 側と同じ設計に揃える
  - untyped 側に marker / MDC 対応の emit API を足すなら、typed 側も同等の API を持つ

## 主要変更

### 1. core logging へ能力を吸収する

- `ActorContext` に facade 置換用の logging API を追加する
  - 通常ログ: `log(level, message)`
  - marker 付きログ: `log_with_marker(level, message, marker_name, marker_properties)`
  - MDC 付きログ: `log_with_mdc(level, message, mdc)`
  - marker + MDC 付きログ: `log_with_marker_and_mdc(level, message, marker_name, marker_properties, mdc)`
- marker / MDC の寿命は 1 回のログ呼び出し単位にする
  - `ActorContext` は `logger_name` だけ保持する
  - 連続するログ呼び出しの間で marker / MDC を暗黙に持ち越さない
- actor 外から使う logging 能力は `ActorSystem` 側に寄せる
  - `emit_log` を structured logging へ拡張する
  - 追加候補
    - `emit_log_with_marker`
    - `emit_log_with_mdc`
    - `emit_log_with_marker_and_mdc`
  - `TypedActorSystem` と `TypedActorSystemLog` も同等の API を持つ
- `LoggingReceive` が担っていた receive 補助は `core` 側の小さな helper に置き換える
  - 例: `log_receive(context, message, handled, label, level)`
  - helper は受け取った message / handled / unhandled / label / level から receive log を 1 回 emit するだけに留める
  - facade のような内部状態は持たせない
- `NoLogging` 専用型は廃止する
  - no-op は「呼ばない」で表現する
  - typed の `LogOptions::with_enabled(false)` ですでに表現できる箇所はその既存 API を使う

### 2. actor-adaptor/std から facade / shim を削る

- 削除対象
  - `std/event/logging/actor_log_marker.rs`
  - `std/event/logging/logging_adapter.rs`
  - `std/event/logging/actor_logging.rs`
  - `std/event/logging/bus_logging.rs`
  - `std/event/logging/diagnostic_actor_logging.rs`
  - `std/event/logging/logging_receive.rs`
  - `std/event/logging/no_logging.rs`
- `std/event/logging.rs` は `TracingLoggerSubscriber` だけを公開する薄い adapter surface にする
- `std/event/stream` は shim を廃止する
  - `EventStreamSubscriberShared`
  - `subscriber_handle`
  - 既存の local wrapper / alias
  - 利用側は `core` の `EventStreamSubscriber` / `EventStreamSubscriberShared` / `subscriber_handle` を直接使う
- `std/system/*` は残すが、stream shim 依存を外す
  - `std/system/base.rs` などの import と公開シグネチャは `core` の subscriber 型へ切り替える
  - 「module は残すが、shim を経由しない」状態にする
- `std/pattern.rs` の core 横流し wrapper を削除する
  - `ask_with_timeout`
  - `graceful_stop`
  - `graceful_stop_with_message`
  - `retry`
  - `CircuitBreaker` / `CircuitBreakerShared` は、残すなら `StdClock` を差す adapter alias と constructor に限定し、`core` の state machine が正であることを明示する
- `dispatch/*`、`system/*`、`tracing_logger_subscriber.rs`、`dead_letter_log_subscriber.rs`、`std_clock.rs` は残す
  - これらは `std` / `tokio` / `tracing` 依存の adapter 実装だから

### 3. showcase / tests を core API に追随させる

- `showcases/std/legacy/classic_logging/main.rs` は actor-adaptor の logging facade を使わず、`core` の新しい logging API だけで書き直す
  - `logger_name` は `ctx.set_logger_name(...)` を使う
  - marker / MDC は各ログ呼び出しで明示的に渡す
  - receive logging は `core` helper を使う
- `cluster_membership` など event stream subscriber を使う showcase は `core` の `EventStreamSubscriber` / `EventStreamSubscriberShared` / `subscriber_handle` に統一する
- actor-adaptor の unit tests は facade 前提を削除し、残る adapter 実装だけを確認する
- package boundary test は `std` surface が adapter-only になったことを確認する内容へ更新する
  - `std::event::logging` は `TracingLoggerSubscriber` だけが live entry point であること
  - `std::event::stream` に shim API が残っていないこと
  - `std/system/*` は `core` の subscriber 型で引き続き利用できること

## 実装順序

1. `core` 側へ logging API と receive helper を追加する
   - `ActorContext`
   - `ActorSystem`
   - `TypedActorSystem`
   - `TypedActorSystemLog`
2. showcase / tests / `std/system/*` を新しい `core` API へ移行する
   - この段階で facade / shim をまだ残してもよいが、新規参照は増やさない
3. `actor-adaptor/std` から facade / shim を削除する
   - `std/event/logging/*`
   - `std/event/stream` shim
   - `std/pattern` wrapper
4. boundary test と公開面テストを更新し、adapter-only になったことを固定する

この順序で進めることで、中間状態でもビルド不能時間を最小化する。

## 公開 API 変更

- 追加
  - `fraktor_actor_core_rs::core::kernel::actor::ActorContext` の marker / MDC 対応 logging API
  - `fraktor_actor_core_rs::core::kernel::system::ActorSystem` の marker / MDC 対応 emit API
  - `fraktor_actor_core_rs::core::typed::TypedActorSystem` の marker / MDC 対応 emit API
  - `fraktor_actor_core_rs::core::typed::TypedActorSystemLog` の marker / MDC 対応 emit API
  - `fraktor_actor_core_rs::core::kernel::event::logging` の receive logging helper
- 削除
  - `fraktor_actor_adaptor_rs::std::event::logging::{ActorLogMarker, LoggingAdapter, ActorLogging, BusLogging, DiagnosticActorLogging, LoggingReceive, NoLogging}`
  - `fraktor_actor_adaptor_rs::std::event::stream` の shim API
  - `fraktor_actor_adaptor_rs::std::pattern` の core 横流し helper
- 維持
  - `fraktor_actor_adaptor_rs::std::event::logging::TracingLoggerSubscriber`
  - `fraktor_actor_adaptor_rs::std::event::stream::DeadLetterLogSubscriber`
  - `dispatch/*`
  - `system/*`
    - ただし stream shim 依存の型参照は `core` の型へ更新する

## テスト計画

- core logging
  - `emit_log*` 系 API が timestamp を system state 由来で付与する
  - marker / MDC / `logger_name` が `LogEvent` に正しく載る
  - `ActorContext` 経由のログで `origin` / `logger_name` / marker / MDC が埋まる
  - marker / MDC が次のログ呼び出しへ暗黙に持ち越されない
  - receive logging helper が handled / unhandled と label を反映する
- typed surface
  - `TypedActorSystem` / `TypedActorSystemLog` が untyped と同等の structured logging API を持つ
  - typed 側の API でも marker / MDC / `logger_name` が期待通りに出る
- actor-adaptor
  - `TracingLoggerSubscriber` が core `LogEvent` を `tracing` に正しく転送する
  - `DeadLetterLogSubscriber` が core event stream subscriber 契約で動く
  - `std/system/*` が core subscriber 型へ切り替わっても利用できる
  - `std` public surface に facade / shim が残っていないこと
- showcases
  - `classic_logging` が core API だけで従来のデモ内容を維持する
  - event stream 利用 examples が core subscriber API で動く
- 最終確認
  - `./scripts/ci-check.sh examples`
  - `./scripts/ci-check.sh std`
  - `./scripts/ci-check.sh ai all`

## 前提

- 後方互換は不要なので facade は残さず削除する
- Pekko 互換は「facade 名を残すこと」ではなく「能力と意味論を維持すること」で担保する
- event/logging は `core` へ「型ごと移植」ではなく「能力の再配置」を行う
- `dispatch/*` / `system/*` の adapter としての責務は維持する
  - ただし shim 削除に伴う型参照と公開面の更新はこの計画の対象に含める
