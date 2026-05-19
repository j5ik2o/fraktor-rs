# 実装計画

## 対象
- kernel logging の `LoggingFilter` / `DefaultLoggingFilter`
- `LoggingAdapter` の pre-publish 判定
- `SystemState` / `SystemStateShared` の共通 filter 経路

## 方針
- Pekko の `LoggingFilter` / `LoggingFilterWithMarker` は、`LogEvent` 全体を受ける単一 predicate trait に翻訳する
- marker 判定は別 trait を増やさず、`LogEvent` の marker 情報を見る実装で表現する
- `DefaultLoggingFilter` は最小 log level による既定判定だけを持つ
- `LoggingAdapter` と `emit_log` は同じ filter 判定を使い、event stream publish 前に reject できるようにする
- 追加型は logging モジュール内部または `pub(crate)` に留め、公開境界は広げない

## 実装手順
1. `logging_filter.rs` と `default_logging_filter.rs` を追加し、module wiring を通す
2. `SystemState` に filter 保持・差し替え・判定 API を追加する
3. `SystemStateShared` に setter と publish helper を追加し、`emit_log` を共通経路へ寄せる
4. `LoggingAdapter::log` を helper 経由に切り替える
5. `cargo fmt`、変更範囲 check/test、対象 lint を実行して結果を report に残す
