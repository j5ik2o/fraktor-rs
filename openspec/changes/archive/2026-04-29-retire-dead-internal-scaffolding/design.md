## Context

旧 change は `BootingSystemState` / `RunningSystemState` と、そこから呼ばれる `register_guardian_pid` までを一括削除する設計だった。現行コードでは `register_guardian_pid` が `system_state/tests.rs` と `system_state_shared/tests.rs` からも使われているが、wrapper 削除後は production callsite が消えるため dylint の dead code 検出対象になる。

今回の目的は「file-level `#![allow(dead_code)]` で残った閉じた scaffolding を安全に退役すること」に限定する。ただし wrapper 削除で連動 dead 化する `register_guardian_pid` は同じ source tree hygiene の問題として、この change で削除する。

## Goals / Non-Goals

**Goals:**

- `BootingSystemState` / `RunningSystemState` とその専用テストを削除する。
- `state.rs` の module wiring を整理する。
- OpenSpec CLI で validate 可能な change 名にする。
- 削除範囲を production 未使用かつ wrapper 自身に閉じた箇所へ限定する。

**Non-Goals:**

- test-only `register_guardian_pid` API の維持。
- guardian registration の新規 production contract 設計。
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

4. **`register_guardian_pid` は削除し、テストは production API に寄せる。**
   wrapper 削除後、`register_guardian_pid` は production callsite を持たず dead code になる。`termination_signal_completes_after_root_marked_terminated` と `clear_guardian_does_not_block_on_read_lock` は `set_root_guardian` と実 `ActorCell` で置き換える。`extension_or_insert_with_after_root_started_succeeds` は root PID 登録に依存していないため `mark_root_started` だけにする。`guardian_cell_via_cells_returns_none_when_missing` は「PID だけ登録され cell がない」という production API では作れない状態を検証しているため、test-only API と一緒に削除する。

5. **spec delta は `source-test-layout-hygiene` に追加する。**
   `BootingSystemState` / `RunningSystemState` は production behavior の Requirement ではないが、file-level `#![allow(dead_code)]` で隠れた test-only scaffolding を production source tree に残さない方針は `source-test-layout-hygiene` の責務に合う。OpenSpec strict validation も delta を要求するため、挙動変更ではなく hygiene requirement として記録する。

## Risks / Trade-offs

- **Risk: test-only helper 削除で一部テストが消える。** → production API で表現できるテストは置き換え、test-only API でしか作れない unreachable state のテストだけを削除する。
- **Risk: boot/running type-state を将来欲しくなる。** → 旧 wrapper を戻すのではなく、その時点の `SystemStateShared` 設計に合わせてゼロベースで設計する。
- **Risk: tests カバレッジが減る。** → 削除される wrapper 自身のテストだけを削除するため、production behavior のカバレッジは維持される。
