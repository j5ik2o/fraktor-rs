# `actor-adaptor-std` の `std_*.rs` ファイル名整理

## 概要

`modules/actor-adaptor-std/src/std` 配下だけを対象に、`Std*` 型名・`std_*` 関数名は維持したまま、冗長な `std_*.rs` ファイル名を通常名へ統一する。公開 API のシンボル名は変えず、`mod` 宣言とテスト配置だけ追従させる。

## 変更方針

- `std_blocker.rs` を `blocker.rs` へ改名し、`src/std.rs` の `mod` / `pub use` を更新する。
- `time/std_clock.rs` を `time/clock.rs` へ改名し、`src/std/time.rs` の `mod` / `pub use` を更新する。
- `time/std_mailbox_clock.rs` を `time/monotonic_mailbox_clock.rs` へ改名し、`src/std/time.rs` の `mod` / `pub use` を更新する。
- `system/std_actor_system_config.rs` を `system/actor_system_config.rs` へ改名し、`src/std/system.rs` の `mod` / `pub use` を更新する。
- `tick_driver/std_tick_driver.rs` は `src/std/tick_driver.rs` へ統合し、`StdTickDriver` の実装本体を親モジュール側へ移す。
- 各 renamed module の `#[cfg(test)] mod tests;` に対応するテスト配置も新しいモジュールパスへ移す。
- `StdTickDriver` の単体テストは `src/std/tick_driver/tests.rs` に統合する。
- 参照更新は `modules/actor-adaptor-std/src` 内に限定し、型名 `StdBlocker` `StdClock` `StdTickDriver` と関数名 `std_monotonic_mailbox_clock` `std_actor_system_config` は変更しない。

## 公開 API 影響

- Rust の公開シンボル名は変更しない。
- `pub use` の再公開先モジュールだけ変わるが、外部利用者から見える import パスは維持する。
- 振る舞い変更はなし。目的はファイル名と内部モジュール名の整理のみ。

## テスト

- `modules/actor-adaptor-std` の関連テストで rename 後のモジュール解決が通ることを確認する。
- `StdBlocker` `StdClock` `StdTickDriver` の単体テストと `std::tick_driver` 親モジュール側のテストを実行する。
- 最後に `rtk ./scripts/ci-check.sh ai all` を完走させてエラーなしを確認する。

## 前提

- 対象範囲は `modules/actor-adaptor-std/src/std` 配下のみとする。
- `remote-adaptor-std` と `utils-adaptor-std` の `Std*` 命名は今回の対象外とする。
- ファイル名は責務ベースで決め、`Std` 接頭辞の除去に伴う型名変更までは行わない。
