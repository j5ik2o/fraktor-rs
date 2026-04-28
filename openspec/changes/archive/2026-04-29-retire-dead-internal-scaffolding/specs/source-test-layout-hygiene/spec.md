## ADDED Requirements

### Requirement: file-level dead-code scaffolding は production source tree に残さない

production source tree にある内部型は、production callsite を持たない状態でファイル冒頭 `#![allow(dead_code)]` により丸ごと警告抑制されていてはならない（MUST NOT）。test-only に閉じた古い scaffolding は、対象テストと一緒に削除するか、必要性を再評価して明示的な production contract に昇格しなければならない（MUST）。

#### Scenario: production 未使用の boot/running wrapper を削除する

- **WHEN** `modules/actor-core/src/core/kernel/system/state/` を確認する
- **THEN** `BootingSystemState` と `RunningSystemState` は production source tree に存在しない
- **AND** `booting_state.rs` / `running_state.rs` の file-level `#![allow(dead_code)]` は存在しない

#### Scenario: wrapper 専用テストを残さない

- **WHEN** `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` を確認する
- **THEN** `BootingSystemState` / `RunningSystemState` のみを検証するテストは存在しない

#### Scenario: test-only guardian PID registration API を残さない

- **WHEN** `register_guardian_pid` の利用箇所を確認する
- **THEN** `SystemState::register_guardian_pid` と `SystemStateShared::register_guardian_pid` は存在しない
- **AND** guardian registration を必要とするテストは実 `ActorCell` を `set_*_guardian` に渡す production API 経由で表現されている
