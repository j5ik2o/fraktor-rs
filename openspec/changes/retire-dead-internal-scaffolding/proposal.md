## Why

`actor-core` の system state 配下に、ファイル冒頭 `#![allow(dead_code)]` で警告抑制された `BootingSystemState` / `RunningSystemState` が残っている。production の actor system 初期化は `SystemStateShared` を直接扱う経路に切り替わっており、この 2 つの wrapper は production callsite を持たない。

既存の `openspec/changes/2026-04-24-retire-dead-internal-scaffolding/` は change 名が数字始まりで OpenSpec CLI の validate 対象にできない。また、現行コードでは `register_guardian_pid` が wrapper 以外のテストからも使われており、旧 artifact の「wrapper 削除後に連動 dead 化する」という前提が成り立たない。この change では現行コードに合わせ、機械的に退役できる範囲だけを valid な OpenSpec change として切り直す。

## What Changes

- `BootingSystemState` / `RunningSystemState` を削除する。
- `modules/actor-core/src/core/kernel/system/state.rs` から `mod booting_state;` / `mod running_state;` を削除する。
- `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` から `BootingSystemState` 専用の 2 テストと import を削除する。
- `SystemState::register_guardian_pid` / `SystemStateShared::register_guardian_pid` はこの change では削除しない。現行テストが利用しているため、削除または test helper 化は別 change で判断する。

## Capabilities

### New Capabilities

なし。

### Modified Capabilities

- `source-test-layout-hygiene`: production source tree に残る test-only / dead internal scaffolding の退役方針へ、file-level `#![allow(dead_code)]` で隠れた内部 wrapper を残さない requirement を追加する。

## Impact

- 影響コード:
  - `modules/actor-core/src/core/kernel/system/state/booting_state.rs`
  - `modules/actor-core/src/core/kernel/system/state/running_state.rs`
  - `modules/actor-core/src/core/kernel/system/state.rs`
  - `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs`
- 影響しないもの:
  - workspace 外公開 API
  - production の actor system 初期化フロー
  - `SystemStateShared` / `SystemState` の guardian 登録 API
  - remote B 方針および `remote-artery-settings-parity`
- 後続候補:
  - `register_guardian_pid` を production API として残すか、test helper へ移すか、呼び出しテストを置き換えて削除するかを別 change で判断する。
