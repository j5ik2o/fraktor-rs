# pekko-phase-kernel-typed logging write-tests 計画

## 概要
`00-plan.md` に従い、今回の write-tests ステップでは kernel logging の `LoggingFilter / DefaultLoggingFilter` 向け先行テストだけを追加する。プロダクションコードは変更しない。

## テスト対象
- `LoggingFilter` trait 契約
- `DefaultLoggingFilter` の level 判定
- `LoggingAdapter` の publish 前 filter 適用
- `SystemStateShared::emit_log` の publish 前 filter 適用

## 方針
- `{type}/tests.rs` に type-local test を置く
- compile 対象に乗る既存テスト面として `logging_adapter/tests.rs` と `system_state_shared/tests.rs` を更新する
- marker 判定は `LoggingFilter` 契約側と `LoggingAdapter` 側で押さえ、`SystemStateShared::emit_log` 側は level 判定に限定する

## 非対象
- `logging.rs` の wiring
- `logging_adapter.rs` / `system_state.rs` / `system_state_shared.rs` の実装変更
- config-driven な filter 差し替え機構
