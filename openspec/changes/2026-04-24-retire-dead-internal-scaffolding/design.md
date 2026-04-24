## Context

### 探索経路と検出方法

`./scripts/` 等で自動検出される dead_code とは別に、`grep -rn "allow(dead_code)" modules/ src/` で全 21 件の `allow(dead_code)` 注釈を列挙し、以下の軸で選別した:

1. **ファイル冒頭 `#![allow(dead_code)]`** (モジュール丸ごと警告抑制) = 8 件
2. **関数 / メソッド / field 粒度 `#[allow(dead_code)]`** = 13 件

(1) の中でも `pub(crate)` 以下で外部参照ゼロのものを「機械的削除」候補として抽出。結果、以下の 2 組が残った:

- **候補 A**: `stream-core/src/core/impl/` 配下 5 ファイル + `graph_chain_macro.rs` (6 ファイル孤立島、本体 約 730 行)
- **候補 B**: `actor-core/src/core/kernel/system/state/` の `booting_state.rs` / `running_state.rs` (2 ファイル、本体 67 行)

候補 A は `#[macro_export] graph_chain!` を含むため crate root の公開マクロ面に影響が出る (`#[macro_export]` は `pub(crate)` ではなく workspace 外からも見える)。機械的削除ではあるが「公開マクロ面の破壊的変更」という副次効果が発生するため、本 change では候補 B のみを扱い、候補 A は別 change に hand-off する。

### 現状の参照関係

```
state.rs
  ├─ mod booting_state;   ◀──── state/system_state/tests.rs (L9, L546, L577)
  ├─ mod running_state;   ◀──── booting_state.rs のみ (into_running 戻り値)
  ├─ mod authority_state;        ─── pub use AuthorityState;  [prod 実働]
  ├─ pub mod system_state;       ─── [prod 実働]
  ├─ mod system_state_shared;    ─── pub use SystemStateShared;  [prod 実働]
  └─ mod system_state_weak;      ─── pub use SystemStateWeak;  [prod 実働]

register_guardian_pid 経路 (連動 dead 化対象):
  SystemStateShared::register_guardian_pid  (system_state_shared.rs:428)
  └─ SystemState::register_guardian_pid     (system_state.rs:512)
     └─ self.guardians.register(kind, pid) + guardian_alive_flag(kind).store(true)

  唯一の caller: BootingSystemState::register_guardian (削除対象)

  production の対応経路:
  SystemStateShared::set_root_guardian / set_system_guardian / set_user_guardian
  └─ SystemState::set_*_guardian (system_state.rs:507, 518, 524)
     └─ self.guardians.register(GuardianKind::XXX, cell.pid())
        + root/system/user_guardian_alive.store(true)
  caller: extended_actor_system::bootstrap (base.rs:612-640 付近)
```

`grep -rn --include="*.rs" -E "(use .*booting_state|booting_state::|mod booting_state)" modules/` の結果、外部参照は `state.rs:21` の `mod` 宣言と `system_state/tests.rs:9` の import / L546・L577 の `BootingSystemState::new()` 呼び出し 2 箇所のみ (それぞれ L543 から始まる `booting_into_running_requires_all_guardians` と L574 から始まる `booting_into_running_fails_when_guardian_missing` の 2 テスト関数内部)。`running_state` も同様で `booting_state.rs` 内で `RunningSystemState::new()` を呼ぶだけ。

### `#[allow(dead_code)]` が残っている理由

本 change 以前から `booting_state.rs` / `running_state.rs` の冒頭に `#![allow(dead_code)]` が付与されているのは、production init パスが `SystemStateShared` を直接扱う形に切り替えられた後、**wrapper を削除する意思決定が保留されていた** ためと推定される (commit 履歴 / doc に再接続計画の記述なし)。

## Goals / Non-Goals

**Goals:**

- `BootingSystemState` / `RunningSystemState` と関連テスト 2 本の削除
- 連動 dead 化する `SystemState::register_guardian_pid` (`system_state.rs:512`) および `SystemStateShared::register_guardian_pid` (`system_state_shared.rs:428`) の同時削除
- `./scripts/ci-check.sh ai all` が pass
- `state.rs` の `mod` 宣言整理で隣接型 (`AuthorityState`, `SystemStateShared`, `SystemStateWeak`) の **残すメンバーには** 一切触らない (ただし `SystemStateShared::register_guardian_pid` は削除対象)
- 削除後の再登場を防ぐため本 proposal / design.md に「再導入時はゼロベース設計」と明記する

**Non-Goals:**

- 候補 A (stream-core 孤立島) の退役 (別 change)
- `SyncQueueBackendInternal` の trait method 整理 (別 change)
- `_assert_object_safe` 系 object-safety マーカーの整理 (別 change)
- inline-test helpers `Used in tests` 系の再編 (対象外)
- Pekko 準拠の boot/running state machine の再設計 (スコープ外、将来必要になったら別 change)

## Decisions

### Decision 1 — 2 つの wrapper を独立削除ではなく同時削除する

`RunningSystemState` は `BootingSystemState::into_running()` の戻り値型として **相互依存** している (running_state の唯一の caller が booting_state)。片方だけを残すと:

- `BootingSystemState` だけ残す → `RunningSystemState` が削除されると `into_running` の戻り値型が無くなりコンパイル失敗
- `RunningSystemState` だけ残す → 唯一の生成元 `BootingSystemState::into_running()` が消えるため factory が皆無になり、結局 dead

→ 2 型は分離不能。同一 change で同時に削除する。

### Decision 2 — tests の削除判断

`system_state/tests.rs` の 2 テスト関数は **`BootingSystemState` 自身の state machine 契約** (全ガーディアン登録で `into_running()` 成功 / 1 つでも未登録なら `SpawnError::SystemNotBootstrapped`) を検証している。

production init パスではこの wrapper を使っておらず、代わりに `extended_actor_system::bootstrap` (実装は `base.rs:612`) が `SystemStateShared::set_root_guardian` / `set_system_guardian` / `set_user_guardian` の 3 つを個別に呼ぶ構造になっている (検証参照: `base.rs:618, 622, 637` 付近)。これらの `set_*_guardian` は `register_guardian_pid` を **呼ばず**、`self.guardians.register(kind, cell.pid())` と alive flag の直接操作を行う並列実装を持つ (`system_state.rs:507-529` 付近)。

「未登録時にエラーを返す」契約も production では `SystemStateShared::user_guardian_pid()` 等が `Option<Pid>` を返し呼び出し側が `SpawnError::system_unavailable()` で失敗させる経路で表現されており (`base.rs:424, 464` 付近)、wrapper の `SpawnError::SystemNotBootstrapped` 契約は **production の contract とは別系統** である。

従って、2 テスト関数は「削除される wrapper 自身の単体テスト」であり、wrapper ごと落とせる。production 側の初期化契約を守るテストが別途必要なら、それは新規 Requirement として別 change で書き起こす方がクリーン (本 change は「退役」に徹する)。

### Decision 3 — 連動 dead 化する `register_guardian_pid` を同一 change で掃除する

主対象 (`BootingSystemState::register_guardian`) が唯一の caller となっている `register_guardian_pid` (SystemState と SystemStateShared の 2 メソッド定義、計 約 8 行) は、主対象の退役直後から完全に caller 不在になる。

選択肢:

- **A. 同一 change で削除** (本 proposal 採用): ボーイスカウトルールに従い、主対象削除の帰結として dead 化する API をまとめて落とす。review 単位が 1 PR に収まり、「削除して試したら別の dead が残った」というハンドオフを避けられる。
- **B. 別 change に分離**: 主対象削除後に一度緑化してから、follow-up PR で `register_guardian_pid` を削除。切り戻しが容易な代わりに PR が 2 本に分かれ、中間状態 (`register_guardian_pid` だけ dead) が merge される。

CLAUDE.md に「優先順位や依存関係を考慮した上でボーイスカウトルールを適用」とあり、今回は依存関係上ほぼノーコストで掃除できるため A を採用。連動退役の範囲は limited (2 メソッド、各々 3-4 行、呼び出し元なし) で、主対象と同じ `state/` サブツリーに閉じる。

なお `register_guardian_pid` 削除後、その内部で使われていた `guardian_alive_flag` ヘルパー (`system_state.rs:591`) は依然として `mark_guardian_stopped` / `guardian_alive` から使われるため生存。波及カスケードはここで止まる。

### Decision 4 — spec 更新は行わない

proposal `Capabilities → Modified Capabilities: なし` の方針:

- `booting_state` / `running_state` は既存の capability spec (例: `actor-system-default-config`, `actor-kernel-new-packages` 等) のいずれの Requirement にも登場していない (`grep -rn "BootingSystemState\|RunningSystemState" openspec/specs/` で 0 件)
- 本 change は spec 未記述の内部 scaffolding の退役であり、Requirement / Scenario 変更を伴わない
- 「spec に未記述の内部実装を退役する」ことを示す meta-spec は存在しないため追加しない

### Decision 5 — 削除対象の粒度

ファイル単位 (`git rm`) で削除する:

- `booting_state.rs` / `running_state.rs` は他型との共存ファイルではないので丸ごと削除
- `state.rs` は `mod` 宣言 2 行のみ削除、`pub use` には変更なし (両 wrapper が `pub use` されていないため)
- `system_state/tests.rs` は 851 行中 2 関数 (L543-572 / L574-587、計 44 行) + L9 の import フラグメント 1 行 (実質 約 45 行) を削除、他テストは無変更

### Decision 6 — 削除後の再導入方針

proposal で「再導入時はゼロベース設計」と宣言する理由:

- Pekko の `ActorSystemImpl` state machine をそのまま移植した旧設計は、fraktor-rs の現実装 (`SystemStateShared` 直接操作) と設計哲学が合わなくなっている (`SystemStateShared` は `ArcShared<SpinSyncMutex<SystemState>>` ベースで状態遷移を type-state ではなく runtime flag で表現する方針)
- 再導入時は `SystemStateShared` ベースで type-state を設計するか、runtime flag ベースで `start_stage: InitStage` を追加するかの設計判断が必要
- 旧 wrapper をリバートするのではなく、その時点の設計思想に合わせて再設計するべき

## Risks / Trade-offs

- **tests カバレッジの微減**: 削除する 2 テスト関数の分だけ `system_state/tests.rs` のカバレッジは下がる。ただし wrapper 本体が消えるため「カバーすべき対象」も同時に消え、実質的な品質低下はなし。
- **Pekko 準拠の再接続要望が将来発生する可能性**: 現時点では doc / comment / roadmap に再接続予定の明示がないため、再導入要望が発生した場合は Decision 6 の方針でゼロベース設計する。本 change による retire がブロックにはならない (再設計の方がクリーン)。
- **review コスト**: 削除量は本体 67 行 + tests 約 40 行と小さく、変更範囲も `state/` 配下に閉じる。review コストは低い。

## Migration Plan

1 change で完結。段階的 migration は不要 (pre-release phase / 破壊的変更許容 / 公開面ゼロ)。

依存関係上、参照側から先に落とす必要がある (tests → wrapper → 連動 dead API の順):

- Phase 1 (tests 削除): `system_state/tests.rs` から 2 テスト関数 (L543-572 / L574-587) + L9 import フラグメント削除
- Phase 2 (wrapper 削除): `booting_state.rs` / `running_state.rs` 削除、`state.rs` の `mod` 宣言 2 行削除
- Phase 3 (連動 dead API 削除): `SystemState::register_guardian_pid` / `SystemStateShared::register_guardian_pid` の両メソッド削除
- Phase 4 (検証): `cargo test -p fraktor-actor-core-rs` pass、`cargo build --workspace` pass、`cargo test --workspace` pass、`./scripts/ci-check.sh ai all` pass

## Open Questions

なし。proposal / design の全判断は本文で確定済み。
