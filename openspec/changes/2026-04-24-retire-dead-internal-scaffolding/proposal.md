## Why

直近の dead code 退役フェーズ (`cf786638` Phase 1 / `4f79808a` Phase 2 / `6be568c1` Phase 3 / `561052d8` SharedDyn retire) を経ても、`#![allow(dead_code)]` (ファイル冒頭一括) で警告抑制されたまま内部 scaffolding が残存している。前フェーズの sweep は個別 `#[allow(dead_code)]` アノテーション粒度が中心で、「ファイル丸ごと `#![allow(dead_code)]` で包まれた `pub(crate)` 型」は検出から漏れていた。

本 change ではそのうち最も「機械的に `git rm` できる」条件を満たすもの (= 外部参照ゼロ / 公開 API 未露出 / 将来再接続の doc 明記なし / test でしか起動しない閉じた試験装置) を 1 組退役する。

## What Changes

### 退役対象 — `actor-core` system state machine placeholder

`BootingSystemState` / `RunningSystemState` は Pekko の `ActorSystemImpl` boot → running state machine を模した `pub(crate)` 薄ラッパー (計 67 行)。production init フローは `SystemStateShared` を直接扱う経路に切り替わっており、両 wrapper は **`system_state/tests.rs` の 2 テスト関数 (`booting_into_running_requires_all_guardians` L543-572 / `booting_into_running_fails_when_guardian_missing` L574-587) からのみ参照される閉じた試験装置**。production の他 callsite は皆無。

試験しているのは「3 ガーディアン全登録で `into_running()` が `Ok(RunningSystemState)`」「未登録なら `SpawnError::SystemNotBootstrapped`」という **BootingSystemState 自身の契約**。production の init パスはこの wrapper を使わない経路で走るため、wrapper もろとも削除しても production 契約の検証カバレッジは変化しない。

削除するもの (主対象):

- `modules/actor-core/src/core/kernel/system/state/booting_state.rs` (37 行) 全体
- `modules/actor-core/src/core/kernel/system/state/running_state.rs` (30 行) 全体
- `modules/actor-core/src/core/kernel/system/state.rs` の `mod booting_state;` / `mod running_state;` 2 行
- `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` の 2 テスト関数 (L543-572 / L574-587、合計 約 44 行) および `BootingSystemState` を import している行 (L9 の該当フラグメント)

削除するもの (主対象の退役で連動して dead 化する内部 API):

- `modules/actor-core/src/core/kernel/system/state/system_state.rs:512` の `pub(crate) fn register_guardian_pid(&mut self, kind: GuardianKind, pid: Pid)` (`SystemState::register_guardian_pid`) 全体
- `modules/actor-core/src/core/kernel/system/state/system_state_shared.rs:428` の `pub(crate) fn register_guardian_pid(&self, kind: GuardianKind, pid: Pid)` (`SystemStateShared::register_guardian_pid` wrapper) 全体

これらは production init パスで使われず、唯一の caller が `BootingSystemState::register_guardian` (= 主対象に含まれる削除対象) であるため、主対象の退役後は完全に caller 不在になる。ボーイスカウトルールに従い同 change で掃除する。`guardian_alive_flag` ヘルパーは `mark_guardian_stopped` / `guardian_alive` からも使用されるため保持。

### BREAKING

なし。すべて `pub(crate)` 以下の内部 API。workspace 外利用者への影響ゼロ。

### Non-Goals (別 change で扱う余地)

- **stream-core `src/core/impl/` の孤立島**: `graph_dsl` / `graph_dsl_builder` / `port_ops` / `reverse_port_ops` / `flow_fragment` / `graph_chain_macro` の 6 ファイル (本体 約 730 行 + tests 5 ファイル)。`#[macro_export] graph_chain!` が **crate ルートの公開マクロ面** を消すため影響評価を独立 change で扱う。
- **`utils-core` の `SyncQueueBackendInternal` trait method**: 探索中、`overflow_policy()` / `is_closed()` には `#[allow(dead_code)]` が付いているものの `vec_deque_backend/tests.rs` L68 / L80 / L86 で **tests から実際に使用されている** ことが確認できた。production 未使用の理由と合わせて整理する (= trait 契約を保つか落とすか) のは別 change。
- **`actor-core` の `_assert_object_safe` / `_assert_box_object_safe`**: object-safety 確認マーカーという固有意図を持つため「機械的削除」に分類しない。
- **`#[allow(dead_code)] // Used in tests` 系 inline-test helpers**: `middleware_shared.rs` / `pipeline.rs` / `scheduler_runner.rs` / `actor_ref/base.rs` 等。test 経由で実際に使われているため本 change の対象外。
- **`stream-core` `island_splitter.rs` の `Exercised only by tests` アクセサ**: test 経由で使われているため対象外。

## Capabilities

### New Capabilities

なし。

### Modified Capabilities

なし。本 change は挙動変更を伴わない内部 scaffolding の退役のみで、既存 capability spec の Requirement / Scenario に変更はない (production の init パスは `SystemStateShared` 直接操作の経路で既に走っており、boot/running wrapper は spec 上のいずれの Requirement にも紐づいていない)。

## Impact

### 影響を受けるコード

- `modules/actor-core/src/core/kernel/system/state/booting_state.rs` 削除
- `modules/actor-core/src/core/kernel/system/state/running_state.rs` 削除
- `modules/actor-core/src/core/kernel/system/state.rs` (`mod` 宣言 2 行削除)
- `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` (L543-572 / L574-587 の 2 テスト関数 + L9 import の該当フラグメント削除)
- `modules/actor-core/src/core/kernel/system/state/system_state.rs` (L512 付近の `register_guardian_pid` メソッド削除、約 4 行)
- `modules/actor-core/src/core/kernel/system/state/system_state_shared.rs` (L428 付近の `register_guardian_pid` wrapper 削除、約 4 行)

### 影響を受けない範囲

- 公開 API (workspace 外から見える pub 型 / 関数 / trait)
- 隣接する `SystemStateShared` / `AuthorityState` / `SystemStateWeak` など prod 実働型 (無変更)
- production の actor system 初期化フロー (`SystemStateShared` 直接操作に既に切り替わり済みで、今回の削除で何も揺れない)
- test-support feature / test harness 設定 (本 change で触らない)
- `system_state/tests.rs` のそれ以外のテスト (全 851 行のうち BootingSystemState/RunningSystemState 関連参照は L9 import・L546・L577 の 3 grep ヒットのみ、削除対象の 2 テスト関数本体内部に閉じている)

### 依存関係

- 変更なし (削除のみ、新規依存追加なし)

### リスク

- **boot → running 状態機械の再導入要望**: Pekko 互換ロードマップで将来再接続予定があれば再生成コスト発生。ただし doc / TODO / 設計メモに明示記述が無いため本 change 時点ではゼロベースで削除する。必要になった時点で改めて設計して再導入する。
- **`./scripts/ci-check.sh ai all` 通過**: 本 change の最終ゲート。

### 後続 change (hand-off)

- **stream-core の graph_dsl 孤立島退役**: 6 ファイル本体 約 730 行 + tests 5 ファイル。`graph_chain!` の crate root マクロ面を含めた影響評価と Pekko 互換ロードマップ上の再接続意思の確認を別 change で扱う。
- **`SyncQueueBackendInternal` の trait method 整理**: production で未使用のまま tests だけが契約を保っている状態を整理する別 change。tests の契約意義を再確認して (A) trait method を production で使う経路を作る / (B) trait method と tests をセットで落とす のいずれかを選ぶ。
- **object-safety マーカー整理**: `_assert_object_safe` / `_assert_box_object_safe` の位置づけを明文化する別 change (残すなら rustdoc / 命名で意図を明示、落とすなら削除)。
