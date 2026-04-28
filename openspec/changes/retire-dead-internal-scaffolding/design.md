## Context

旧 change は `BootingSystemState` / `RunningSystemState` と、そこから呼ばれる `register_guardian_pid` までを一括削除する設計だった。しかし現行コードでは `register_guardian_pid` が `system_state/tests.rs` と `system_state_shared/tests.rs` からも使われており、wrapper 削除後に完全な caller 不在にはならない。

今回の目的は「file-level `#![allow(dead_code)]` で残った閉じた scaffolding を安全に退役すること」に限定する。現行テストが使っている guardian 登録 API の整理は、契約意義の判断を伴うため別 change に分離する。

## Goals / Non-Goals

**Goals:**

- `BootingSystemState` / `RunningSystemState` とその専用テストを削除する。
- `state.rs` の module wiring を整理する。
- OpenSpec CLI で validate 可能な change 名にする。
- 削除範囲を production 未使用かつ wrapper 自身に閉じた箇所へ限定する。

**Non-Goals:**

- `register_guardian_pid` の削除。
- guardian registration tests の再設計。
- Pekko 準拠 boot/running type-state の再導入。
- stream-core graph DSL 孤立島、utils-core queue backend、object-safety marker の整理。
- remote B 方針の実装。

## Decisions

1. **valid change 名へ切り直す。**
   `2026-04-24-retire-dead-internal-scaffolding` は数字始まりで OpenSpec CLI が invalid と判定するため、`retire-dead-internal-scaffolding` を正式な change とする。

2. **wrapper 2 型は同時に削除する。**
   `RunningSystemState` は `BootingSystemState::into_running()` の戻り値としてのみ使われる。片方だけを残すと dead code が残るため、2 型を同一 change で削除する。

3. **専用テストは wrapper と同時に削除する。**
   `booting_into_running_requires_all_guardians` と `booting_into_running_fails_when_guardian_missing` は削除対象 wrapper 自身の契約だけを検証している。production 初期化フローの contract ではないため、wrapper と同時に削除する。

4. **`register_guardian_pid` は削除しない。**
   現行コードでは `guardian_cell_via_cells_returns_none_when_missing`、`termination_signal_completes_after_root_marked_terminated`、`extension_or_insert_with_after_root_started_succeeds`、`clear_guardian_does_not_block_on_read_lock` などが利用している。これらを同時に置き換えると「dead scaffolding 退役」から「guardian registration test API 再設計」に scope が広がるため、本 change では保持する。

5. **spec delta は `source-test-layout-hygiene` に追加する。**
   `BootingSystemState` / `RunningSystemState` は production behavior の Requirement ではないが、file-level `#![allow(dead_code)]` で隠れた test-only scaffolding を production source tree に残さない方針は `source-test-layout-hygiene` の責務に合う。OpenSpec strict validation も delta を要求するため、挙動変更ではなく hygiene requirement として記録する。

## Risks / Trade-offs

- **Risk: `register_guardian_pid` が test-only API として残る。** → この change では安全な削除範囲を優先し、後続 change で削除 / test helper 化 / production 契約化のいずれかを判断する。
- **Risk: boot/running type-state を将来欲しくなる。** → 旧 wrapper を戻すのではなく、その時点の `SystemStateShared` 設計に合わせてゼロベースで設計する。
- **Risk: tests カバレッジが減る。** → 削除される wrapper 自身のテストだけを削除するため、production behavior のカバレッジは維持される。
