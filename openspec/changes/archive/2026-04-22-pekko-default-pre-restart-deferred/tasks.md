## 0. 前提確認

- [x] 0.1 ブランチが `main` から最新で、前提 change `2026-04-21-2026-04-20-pekko-restart-completion` が archive 済みであることを確認する
- [x] 0.2 現在の ignored テスト `al_h1_t2_default_pre_restart_stops_children_and_defers_finish_recreate`
  (`modules/actor-core/src/core/kernel/actor/actor_cell/tests.rs:1551`) がまだ `#[ignore]`
  状態で残っていることを確認する。他の AC-H4 / AC-H5 / AL-H1 テストは passing であること
- [x] 0.3 `rtk cargo test -p fraktor-actor-core-rs --lib --no-run` が通り、既存テストを壊さない
  ベースラインを確認する
- [x] 0.4 `ExecutorShared` 既存トランポリン実装 (`executor_shared.rs:40-146`) を読み、
  `running: ArcShared<AtomicBool>` + `trampoline: SharedLock<TrampolineState>` の役割を把握する
  （本 change は既存 `running` フィールドを外部から CAS で操作する API を追加する）

## 1. `ExecutorShared` に `enter_drive_guard` + `DriveGuardToken` を追加

- [x] 1.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs` に以下 API を追加
  （既存の `pub fn` パターンと同じ可視性 `pub` を使う）:
  ```rust
  pub fn enter_drive_guard(&self) -> DriveGuardToken;
  ```
  - 実装: `self.running.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok()` で
    claimed 判定。成功なら自分が drain owner、失敗なら外側 owner を尊重して no-op 挙動
  - 戻り値: `DriveGuardToken { claimed: bool, running: ArcShared<AtomicBool> }`
- [x] 1.2 **`exit_drive_guard` という `&self` メソッドは公開しない** (重要)
  - release 経路は `DriveGuardToken::drop` のみ。enter / exit ペア違反を型システムで防止する
- [x] 1.3 `DriveGuardToken` struct を配置する:
  - 配置先判定: `executor_shared.rs` は現状 163 行。token struct (≤15 行程度) を同居させると
    180 行前後で、type-organization.md の「同居先ファイルが同居後も 200 行を超えない」を**満たす**
  - したがって **`executor_shared.rs` 内に同居可**
  - ただし命名規約 (type-organization.md) では「Handle のライフサイクル責務」は独立ファイル推奨。
    `DriveGuardToken` は RAII token で drain owner ライフサイクル管理の責務を持つ。**別ファイル
    `executor_shared/drive_guard_token.rs` に分離する方が保守性が高い**
  - 採用: **別ファイル `executor_shared/drive_guard_token.rs` に分離配置**
  - `executor_shared.rs` に `mod drive_guard_token;` + `pub use drive_guard_token::DriveGuardToken;` を追加
  - 既存 `executor_shared/tests.rs` のサブディレクトリ構造と一致する
- [x] 1.4 `DriveGuardToken` 実装:
  ```rust
  #[must_use = "DriveGuardToken must be held for the full guarded region; \
                drop it at the end of the scope where `enter_drive_guard` was called"]
  pub struct DriveGuardToken {
      claimed: bool,
      running: ArcShared<AtomicBool>,
  }

  impl Drop for DriveGuardToken {
      fn drop(&mut self) {
          if self.claimed {
              self.running.store(false, Ordering::Release);
          }
      }
  }
  ```
  - rustdoc 英語で、enter / drop のペア契約と「claimed=false の場合は drop で何もしない」仕様を明記
  - **`#[must_use]` 属性必須**: `let _ = executor.enter_drive_guard();` のような即 drop 誤用を
    コンパイル時に検出するため
  - **コンストラクタアクセス方針**: `DriveGuardToken` の struct フィールドは private (デフォルト) と
    し、`DriveGuardToken` の `impl` ブロックに `pub(crate) fn new(claimed: bool, running: ArcShared<AtomicBool>) -> Self`
    を追加する。`ExecutorShared::enter_drive_guard` は `DriveGuardToken::new(claimed, self.running.clone())`
    を呼んで token を生成する。**フィールドを `pub(super)` にして親モジュールから直接構築する案は採用
    しない**（module 境界越しの field アクセスを増やさず、コンストラクタで invariant 保証を明確にする
    ため）
  - 外部 crate は `pub` な `enter_drive_guard` 経由でしか token を取得できない (`new` が `pub(crate)`
    のため)。型システムで不正な token 構築を防ぐ
  - **`drop` 実装に tail drain を追加してはならない (MUST NOT)**: 既存 `ExecutorShared::execute` の
    Step 4 tail drain (`executor_shared.rs:109-132`) 相当の drain ロジックを `DriveGuardToken::drop`
    に書かないこと。理由は design.md 「採用する設計」セクション末尾を参照（tail drain を `drop` で
    実行すると guard 中に積まれた child.Stop が同期 drain され Pekko async dispatcher 非互換の再入を
    引き起こす）。rustdoc にもこの禁止事項を明記する
- [x] 1.5 `executor_shared/tests.rs` に以下を検証するテストを追加:
  - `enter_drive_guard` 呼び出し中の `execute` は trampoline に積まれるだけで `inner.execute` が呼ばれない
    こと（既存 `execute` のロジックそのままで実現されるが、smoke test として記述）
  - `enter_drive_guard` を連続呼び出すと、1 回目は `claimed=true`、2 回目は `claimed=false`
  - token drop 後に `running=false` に戻っていること (1 回目)、2 回目 token drop は no-op
  - `enter_drive_guard` → `f()` 内で `execute` → `f()` 終了後も pending に task が残り、
    次の外部 `execute` で drain されること
- [x] 1.6 `rtk cargo check -p fraktor-actor-core-rs` がクリーンビルドされることを確認

## 2. `MessageDispatcherShared::run_with_drive_guard` を追加

- [x] 2.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs` に
  以下シグネチャの helper を追加（可視性は `pub(crate)`、外部 crate からの利用は本 change では不要）:
  ```rust
  pub(crate) fn run_with_drive_guard<F, R>(&self, f: F) -> R
  where
      F: FnOnce() -> R;
  ```
- [x] 2.2 実装は以下:
  1. 既存 `self.executor()` helper (`message_dispatcher_shared.rs:83`) を使って
     `ExecutorShared` を取得する（`register_for_execution` と同じ経路を流用）
  2. `let _token = executor.enter_drive_guard();` で RAII token を保持
  3. `f()` を呼び戻り値を返す
  4. scope 終了時に `_token` が drop され、`claimed=true` なら `running=false` に戻る
- [x] 2.3 **注意**: `DriveGuard<'a>` のような外部参照 wrapper 型は作らない。`DriveGuardToken` が
  `ArcShared<AtomicBool>` を owned clone で保持するためライフタイム制約がない
- [x] 2.4 `message_dispatcher_shared/tests.rs` に以下を追加:
  - `run_with_drive_guard` 内で `f()` が呼び出し直接実行されること
  - `f()` が panic しても `DriveGuardToken::drop` が呼ばれ `running=false` に戻ること（RAII 確認）
  - `f()` 内で `system_dispatch` を呼んだ場合、target mailbox は guard 解除後も pending として
    残り synchronous に drain されないこと（`InlineExecutor` を差し替えた smoke test）
- [x] 2.5 `rtk cargo check -p fraktor-actor-core-rs` がクリーンビルドされることを確認

## 3. `fault_recreate` 内の `pre_restart` 呼び出しを guard でラップ

**注: ガード適用範囲は `pre_restart` 呼び出し 1 点に限定する**。`ActorCellInvoker::system_invoke`
全体や `fault_recreate` 全体はラップしない（design.md 「却下案 1」参照）。

- [x] 3.1 `modules/actor-core/src/core/kernel/actor/actor_cell.rs` の `fault_recreate` を編集し、
  `self.actor.with_write(|actor| actor.pre_restart(&mut ctx, cause))?` の呼び出しを
  `run_with_drive_guard` でラップする
  - 実装例の詳細は **design.md の「ActorCell::fault_recreate の局所ラップ」セクション**参照
    （`let dispatcher = self.new_dispatcher_shared()` → `dispatcher.run_with_drive_guard(closure)` →
    `result?` → `ctx.clear_sender()` の順序を保持）
  - closure 内では `self.actor.with_write(|actor| actor.pre_restart(&mut ctx, cause))` の戻り値
    (`Result<(), ActorError>`) を返す
  - `ctx` は closure に `&mut ctx` として借用され、closure 完了 (= `run_with_drive_guard` 戻り) で
    borrow が解放されるので、後続の `ctx.clear_sender()` は通る
  - 既存の Pekko parity コメント（`is_failed_fatally` no-op の意図説明、`debug_assert!` の
    AC-H3 precondition コメント等）は **保持すること**（本 change の範囲外で削除しない）
- [x] 3.2 `fault_recreate` 内の他の処理 (`is_failed_fatally` 早期 return、`debug_assert!` の mailbox
  suspended 検査、`set_children_termination_reason` 呼び出し、`finish_recreate` への fall-through) は
  **guard の外側**に置く。これらは再入問題を起こさないため guard 不要
- [x] 3.3 rustdoc コメントで、pre_restart ラップの意図（「default pre_restart が stop_all_children を
  呼ぶ sync dispatch 経路で child mailbox の inline 再入 drain を防ぐ。production は既存 ExecutorShared
  トランポリンが効いているため no-op 的に通過する」）を明記
- [x] 3.4 `rtk cargo check -p fraktor-actor-core-rs` がクリーンビルドされることを確認
- [x] 3.5 既存 AC-H4 / AC-H5 / AL-H1 テスト群が全 passing であることを確認（regression check）:
  `rtk cargo test -p fraktor-actor-core-rs -- ac_h4 ac_h5 al_h1`
- [x] 3.6 `finish_recreate` の `post_restart` 呼び出しには **guard を適用しない**ことを確認
  （design.md 「ActorCell::fault_recreate の局所ラップ」の末尾参照）

## 4. ignored テストの復活 + watch 登録修正 + 複数 child 検証の追加

- [x] 4.1 `modules/actor-core/src/core/kernel/actor/actor_cell/tests.rs:1549-1550` の
  `#[ignore = "..."]` アトリビュートを削除する
- [x] 4.2 **watch 登録を `WatchKind::User` から `WatchKind::Supervision` へ変更する** (**MUST**)
  - 現行 test の `parent.register_watching(child.pid())` (`actor_cell/tests.rs:1574`) は
    `pub fn register_watching` 経由で `WatchKind::User` を登録している
  - **問題**: default `pre_restart` の内部動作 `stop_all_children` は `unregister_watching(child_pid)`
    を呼び、これは `WatchKind::User` のみ除去する（supervision は保持するが、test では supervision が
    登録されていないので空になる）。この結果、後続の `parent.handle_death_watch_notification(child.pid())`
    時点で `watching_contains_pid(child.pid()) == false` になり、silently return して
    `finish_recreate` が起動しない
  - **修正**: 該当行を `parent.register_supervision_watching(child.pid())` に変更する
    （`pub(crate) fn register_supervision_watching` が `actor_cell.rs:528` に存在、`WatchKind::Supervision`
    を登録する）
  - この修正は、production における `spawn_with_parent` 経由の自動 supervision watch 配線を、
    test で手動 simulate する意図と一致する（既存 AC-H4 T3 テストでも同様の手動登録を使用）
- [x] 4.3 テスト本体 (`al_h1_t2_default_pre_restart_stops_children_and_defers_finish_recreate`) の
  アサーション自体はそのまま維持する（本 change + 4.2 の修正で挙動が期待どおりに揃う前提）
  - `mid_snapshot == vec!["pre_start", "post_stop"]`
  - `parent.children_state_is_terminating()`
  - `!parent.children_state_is_normal()`
  - 明示的 `handle_death_watch_notification` 後の `final_snapshot == vec!["pre_start", "post_stop", "pre_start"]`
  - `parent.children().is_empty()`
  - `!parent.mailbox().is_suspended()`
  - `parent.children_state_is_normal()`
- [x] 4.4 `rtk cargo test -p fraktor-actor-core-rs -- al_h1_t2` が passing であることを確認
- [x] 4.5 関連 regression として AC-H4 / AC-H5 / AL-H1 の全テストが passing であることを確認:
  `rtk cargo test -p fraktor-actor-core-rs -- ac_h4 ac_h5 al_h1`
- [x] 4.6 **追加 integration test** を 1 件追加する: default `pre_restart` を持つ actor が
  child を **2 件** 持っているケースで、Recreate 後に children 全員が Terminating{Recreation} に
  残っており、最後の child の `handle_death_watch_notification` でのみ `finish_recreate` が起動する
  ことを確認する（現行 T2 は child 1 件のため、複数 child での順序性も検証）
  - テスト名: `al_h1_t2_default_pre_restart_with_multiple_children_defers_finish_recreate_until_last`
  - 配置: 同ファイル `actor_cell/tests.rs` の `AL-H1` セクション内（T2 の直後）
  - 構造: 既存 T2 の cell 作成パターンをコピーし、parent + child_a + child_b の 3 cell を作る
    - Pid は既存テスト (810-812 は T2 で使用中) と衝突しないように 813 / 814 / 815 を使う
    - `parent.register_child(child_a.pid())` / `parent.register_child(child_b.pid())` を両方呼ぶ
  - watch 登録は両 child 分 `parent.register_supervision_watching(child_a.pid())` /
    `parent.register_supervision_watching(child_b.pid())` を使う（4.2 と同様の理由）
  - 検証ポイント:
    - Recreate 後の `parent.children().len() == 2` + `parent.children_state_is_terminating() == true`
    - child A の DWN を先に呼んで `parent.children_state_is_terminating() == true` のまま維持
      （まだ to_die に B 残存、`parent.children().len() == 1`）
    - child B の DWN 後に `parent.children_state_is_normal() == true` + `final_snapshot` に 3 つ目の
      `"pre_start"` が追加されること（`vec!["pre_start", "post_stop", "pre_start"]`）
    - `parent.mailbox().is_suspended() == false`

## 5. Pekko 非互換の非回帰確認（**MUST**、本 change 固有のゲート）

**本 change は既存の Pekko 互換テストを破壊してはならない。**ユーザーから明示的に念押しされた
最重要原則。以下をすべて確認すること。

- [x] 5.1 `rtk cargo test -p fraktor-actor-core-rs` で全テスト passing（kernel 単体、既存 passing を
  ignored にしていないこと）
  - 特に `handle_watch` / `handle_unwatch` / `handle_stop` / `handle_kill` / `handle_failure` /
    `handle_suspend` / `handle_resume` / `handle_create` / `handle_death_watch_notification` を駆動する
    全テストが passing
  - 既存 `executor_shared/tests.rs` の全トランポリンテストが passing（本 change は既存機構の
    外部駆動 API を追加するだけで既存挙動を変えていない）
- [x] 5.2 本 change で新規に `#[ignore]` を追加したテストがないことを確認:
  `rtk git diff main...HEAD -- modules/actor-core/src/ | grep -E "^\+.*#\[ignore"`
  が `al_h1_t2` の ignore **削除** (`^-`) のみで、追加 (`^+`) が 0 件
- [x] 5.3 guard 適用範囲が `fault_recreate` 内の `pre_restart` 呼び出し 1 点に限定されていること:
  `rtk grep -rn "run_with_drive_guard" modules/actor-core/src/core/kernel/actor/` が 1 件のみ
  （`fault_recreate` 内）で、`actor_cell.rs` の他 handler や `system_invoke` 本体には含まれないこと
- [x] 5.4 `ActorCellInvoker::system_invoke` 本体が本 change で変更されていないこと:
  `rtk git diff main...HEAD -- modules/actor-core/src/core/kernel/actor/actor_cell.rs`
  で `fn system_invoke` の match arm 群やラッパー追加が 0 件
- [x] 5.5 `finish_recreate` / `post_restart` 周辺が本 change で変更されていないこと（guard 非適用）:
  同 git diff で `finish_recreate` 関数内や `post_restart` 呼び出しへの guard 適用が 0 件
- [x] 5.6 **`Executor` trait に変更がないこと**を確認:
  `rtk git diff main...HEAD -- modules/actor-core/src/core/kernel/dispatch/dispatcher/executor.rs`
  で 0 行（本 change は `ExecutorShared` レベルで完結し、`Executor` trait は触らない）

## 6. production dispatcher 影響確認

- [x] 6.1 `rtk cargo test -p fraktor-actor-core-rs` 全テスト passing（kernel）
- [x] 6.2 `rtk cargo test -p fraktor-actor-adaptor-std-rs` 全テスト passing（std adaptor）
- [x] 6.3 `rtk cargo test --workspace` 全テスト passing（ワークスペース全体）
- [x] 6.4 `rtk grep -rn "enter_drive_guard\|DriveGuardToken" modules/actor-adaptor-std/ modules/` で
  adaptor-std や production 側で本 change API が不適切に呼ばれていないことを裏取り
- [x] 6.5 production dispatcher (`PinnedExecutor` / `ForkJoinExecutor` / `BalancingExecutor`) の
  実装が本 change で変更されていないことを確認:
  `rtk git diff main...HEAD -- modules/actor-core/src/core/kernel/dispatch/dispatcher/pinned_dispatcher.rs
  modules/actor-core/src/core/kernel/dispatch/dispatcher/default_dispatcher.rs
  modules/actor-core/src/core/kernel/dispatch/dispatcher/balancing_dispatcher.rs
  modules/actor-core/src/core/kernel/dispatch/dispatcher/inline_executor.rs`
  で 0 行

## 7. 品質ゲート（マージ前 MUST 条件）

### 7.1 原則 1 (Pekko 互換 + Rust らしい設計) のゲート

- [x] 7.1.1 本 change の state machine 図 (design.md 「state machine」セクション) と実装が一致する
  - `fault_recreate` の `pre_restart` 呼び出し入口で `enter_drive_guard`、戻り時に
    `DriveGuardToken::drop` で release（RAII で保証）
  - guard 中の `send_system_message(child, Stop)` が `ExecutorShared` 既存トランポリンで pending に
    積まれるだけで同一 thread 上で drain されないこと
- [x] 7.1.2 Pekko 参照箇所 (`Actor.scala:626-632`, `dungeon/Children.scala:129-142`,
  `dungeon/FaultHandling.scala:92-118`, `dungeon/FaultHandling.scala:278-303`) との対応が design.md
  「Pekko 参照実装との対応表」に明記されている
- [x] 7.1.3 guard 適用によって Pekko 非互換が **新たに生じていない** ことの検証として、design.md
  「Pekko 互換性の非回帰確認」セクションで挙げた全テストカテゴリが passing であること（§5 と重複するが
  設計原則ゲートとして再確認）
- [x] 7.1.4 **`DriveGuardToken::drop` 実装に tail drain 相当のロジックが追加されていないこと**を
  静的に確認 (spec.md Requirement 2 の検証):
  `rtk grep -En "trampoline|with_write|inner\.execute|pending\.pop" modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared/drive_guard_token.rs`
  で 0 件 (drop 内で pending キューや内部 executor 呼び出しをしないこと)
  - 検出されたら実装が既存 `ExecutorShared::execute` Step 4 の tail drain ロジックを `drop` に
    誤ってコピーしている可能性。design.md 「採用する設計」セクション末尾の禁止事項を参照して削除する

### 7.2 原則 2 (本質的な設計を選ぶ) のゲート

- [x] 7.2.1 局所 workaround ではなく既存 `ExecutorShared` トランポリンの外部駆動 API を導入していることを
  confirm。`stop_all_children` / `fault_recreate` 本体の state machine に特例分岐を追加していないこと
    (`rtk git diff main...HEAD -- modules/actor-core/src/core/kernel/actor/actor_cell.rs actor_context.rs
    | grep -E "^\+.*// workaround|暫定|一時"` が 0 行)
- [x] 7.2.2 本 change で新規追加された `TODO(Phase A3)` は proposal.md の「非目標」に列挙済みの
  項目のみであること（`finish_terminate` / `finish_create` / typed reason 伝播 / user message 経路
  guard）:
  `rtk git diff main...HEAD -- modules/actor-core/src/ | grep -E "^\+.*TODO\(Phase A3\):"`
  で出力される全行を proposal.md「非目標」と手動で照合する

### 7.3 原則 3 (後方互換性を保つコードを書かない) のゲート

- [x] 7.3.1 `ExecutorShared` の新規 `enter_drive_guard` API は既存 `pub fn execute` / `pub fn shutdown`
  と同じスタイル。legacy alias / deprecated 経路を残していないこと
- [x] 7.3.2 **公開 `exit_drive_guard` メソッドが存在しないこと**を静的に確認 (spec.md Req1-S5
  「公開 exit_drive_guard API は存在しない」の検証):
  `rtk grep -En "pub fn exit_drive_guard|pub\(crate\) fn exit_drive_guard" modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs`
  で 0 件
  - release 経路は `DriveGuardToken::drop` のみ。enter / release ペア違反 (release 忘れ・二重 release)
    を型システムで防止する設計意図を守ること
- [x] 7.3.3 暫定 API / 互換層を追加していないこと:
  `rtk grep -rn "legacy\|compat\|deprecated\|backwards" modules/actor-core/src/core/kernel/dispatch/
  dispatcher/ modules/actor-core/src/core/kernel/actor/actor_cell.rs` で本 change 由来のヒットが 0 件
- [x] 7.3.4 `#[allow(dead_code)]` を本 change で新規追加していないこと

### 7.4 原則 4 (no_std core + std adaptor 分離) のゲート

- [x] 7.4.1 `rtk grep -rn "^use std::\|^use std$" modules/actor-core/src/core/kernel/dispatch/
  dispatcher/` が本 change 由来の追加分 0 件
- [x] 7.4.2 `cfg-std-forbid` dylint が本 change の新規コードで違反検出しないこと（7.5.1 に含まれる）
- [x] 7.4.3 thread-local / `std::sync` 等の std 依存を新規に導入していないこと（本 change は `AtomicBool`
  / `ArcShared` を流用するのみで新規 std 依存なし）:
  `rtk grep -rn "std::thread_local\|std::sync::\|thread_local!" modules/actor-core/src/core/kernel/dispatch/dispatcher/`
  で 0 件

### 7.5 CI / lint の final ゲート

- [x] 7.5.1 **OpenSpec artifact 整合性の検証**:
  `openspec validate 2026-04-20-pekko-default-pre-restart-deferred --strict`
  が valid を返すこと
  - change name は **ディレクトリ名そのまま** を指定する (archive 済み change 内の tasks.md で
    日付なしで書かれている箇所があるが、実際の `openspec` CLI は実ディレクトリ名を受け付けるため
    `2026-04-20-` プレフィックスが必要)
  - proposal.md / design.md / tasks.md / specs/**/spec.md の構造整合性を OpenSpec 標準 validator で確認
- [x] 7.5.2 `./scripts/ci-check.sh ai all` が exit 0
  - dylint 8 lint 全 pass: mod-file / module-wiring / type-per-file / tests-location / use-placement /
    rustdoc / cfg-std-forbid / ambiguous-suffix
  - cargo test / clippy / fmt が全て pass
  - **TAKT ルール: 本ゲートは change のマージ直前にのみ実行する**
