## 1. 現状把握 / 事前調査

- [x] 1.1 `modules/actor-core/src/core/kernel/actor/actor_cell.rs` の `report_failure` / `handle_failure` / `handle_kill` 経路と、既存の `set_failed` / `is_failed` / `is_failed_fatally` call site をリスト化 (設計時に grep 済み、本フェーズで再確認)
- [x] 1.2 `modules/actor-core/src/core/kernel/dispatch/dispatcher/pinned_dispatcher.rs` の `register_actor` / `unregister_actor` の rustdoc 現状を確認し、Pekko 参照行が既にあるかどうか確認
- [x] 1.3 `set_failed(perpetrator: Pid)` の既存 production caller がゼロであることを `grep -rn "\.set_failed\(" modules/ | grep -v 'tests\.rs\|test_'` で再確認 (設計時点ではゼロ) — **確認済: 0 hits**
- [x] 1.4 Pekko `references/pekko/actor/src/main/scala/org/apache/pekko/actor/dungeon/FaultHandling.scala:73-74,215-234` を開いて意味論を行単位で再確認
- [x] 1.5 `FailureMessageSnapshot` の公開 API を見て、child perpetrator 相当 field の有無を確認 (設計 Decision 3 で「YAGNI = self.pid 記録のみ」としたが裏取り)
- [x] 1.6 `clear_failed()` の production call site を grep 済: `actor_cell.rs:1264` の `finish_recreate` 内 (recreate_actor 直後、post_restart 直前) で呼ばれている。**Case B**
- [x] 1.7 Task 3 は Case B に確定 (既存 call site 活用、追加 Resume 経路のみ Task 3.4 で配線)

## 2. AC-M3: `report_failure` への `is_failed()` guard 付き `set_failed(self.pid)` 配線

- [x] 2.1 `actor_cell.rs:1352` 付近の `report_failure` 本体を次の順序で書き換える (Pekko `FaultHandling.scala:215-234` `handleInvokeFailure` と行単位写像):

```rust
// Pekko `FaultHandling.scala:218` handleInvokeFailure: suspendNonRecursive()
self.mailbox().suspend();
// Pekko `FaultHandling.scala:221-222` handleInvokeFailure:
//   case _ if !isFailed => setFailed(self); Set.empty
// fraktor-rs の report_failure は user / system message 処理失敗で
// 呼ばれるため、perpetrator は常に self.pid。is_failed() guard で
// 初回のみ記録し、既存 perpetrator (Child もしくは Fatal) を overwrite
// しない。既存の set_failed 実装 (actor_cell.rs:448) も Fatal 保持 guard
// を持つため二重防御になる。
if !self.is_failed() {
  self.set_failed(self.pid);
}
// Pekko `FaultHandling.scala:225` suspendChildren(exceptFor = skip ++ ...)
// fraktor-rs では self-failure 経路のため skip = empty (全子を suspend)。
self.suspend_children();
let timestamp = self.system().monotonic_now();
let payload = FailurePayload::from_error(self.pid, error, snapshot, timestamp);
// Pekko `FaultHandling.scala:231-234` parent.sendSystemMessage(Failed(...))
// fraktor-rs では system の report_failure 経由で supervisor に届く。
// is_failed() guard 通過有無に関わらず毎回発火する (Pekko 同挙動)。
self.system().report_failure(payload);
```

  - **ordering の重要性**: `set_failed` を `mailbox().suspend()` の **後** に置くこと。Pekko L218 `suspendNonRecursive()` が先、L221-222 `setFailed` が後。機能的には独立操作で等価だが、Pekko 行単位 parity 確認のため順序を合わせる。

- [x] 2.2 `report_failure` の rustdoc を更新し、Pekko 参照行 (`FaultHandling.scala:221-222`) と guard の意図 (「重複 report_failure で perpetrator が overwrite されない、既存の `set_failed` 実装も `FailedInfo::Fatal` を保持するため downgrade は起きない」) を明記
- [x] 2.3 `is_failed()` / `set_failed()` の既存 rustdoc も更新し、Pekko `FaultHandling.scala:73,79` への行参照を追加 (既に AC-H3 拡張コメントがあるが、`report_failure` caller 側からのリンクを補強)

## 3. AC-M3: `clear_failed()` の呼び出しタイミングを確保

- [x] 3.1 Task 1.6 の grep 結果 (Round 2 事前確認で判明): `clear_failed()` は `actor_cell.rs:1264` の `finish_recreate` 内 (`recreate_actor()` 直後、`post_restart` 直前) で既に呼ばれている。**Case B (call site あり)** を採用する
- [x] 3.2 `actor_cell.rs:1263-1265` (recreate_actor → clear_failed → post_restart) 付近の comment を更新し、Pekko `FaultHandling.scala:173,284` への参照行を追加 (根拠化)
- [x] 3.3 Decision 5 (design.md) の「unconditional clear」方針を裏付ける rustdoc を Resume arm の `clear_failed()` 呼び出し直前コメントとして追加する。内容: Pekko `FaultHandling.scala:83-86` は `case FailedRef(_) => NoFailedInfo; case other => other` で Fatal を保持するが、fraktor-rs は Resume-to-Restart 変換 (Pekko `:141-144`) が未実装のため Fatal + Resume の経路は現状 production で発生しない。したがって既存 `clear_failed()` (unconditional) の呼び出しで Pekko 互換性を損なわない。将来 Resume-to-Restart 変換を実装する際は `clear_failed_non_fatal()` 等を追加して差し替える想定。
- [x] 3.4 **`SystemMessage::Resume` arm (`actor_cell.rs:1573-1579`) に `cell.clear_failed()` を追加** (Pekko `FaultHandling.scala:150` `faultResume` の `finally clearFailed()` 相当):
  - **配置順序**: `cell.clear_failed()` を **先に** 呼び、その後 `cell.resume_children()` を呼ぶ。Pekko `FaultHandling.scala:149-151` の `try resumeNonRecursive() finally clearFailed(); resumeChildren(...)` と同じ順序 (fraktor-rs には `resume_non_recursive` が無いため 2 文で表現する)。
  - 追加位置の comment に「Pekko `FaultHandling.scala:150` faultResume の finally clearFailed() 相当。本 change (AC-M3) で `set_failed(self.pid)` を `report_failure` に配線したため、Resume で state を clear しないと `is_failed()` が stale になる」を記述。
  - Decision 5 に基づき unconditional clear を採用 (`clear_failed_non_fatal` variant は新設しない)。
- [x] 3.5 `handle_create` (`actor_cell.rs:1168-1174`) pre_start 失敗経路は supervisor → Restart/Stop/Resume への合流経路で state クリアされるため、独自追加は不要 (確認のみ、comment で明示)

## 4. AC-M3: 契約 pinned テスト追加

> **配置方針**: 既に `actor_cell/tests.rs:1098-` 以降に `ac_h3_ext_tN_*` 系の `FailedInfo` 契約テストが存在するため、本 change のテストも同セクションに `ac_m3_*` prefix で追加する。
>
> **テスト実行前提 (Round 5 で裏取り)**:
> - 本セクションのテストは `ActorSystem::new_empty()` + orphan cell (parent なし) で組む。`cell.report_failure` は `system_state_shared.rs:1009` の orphan 経路で `SystemMessage::Stop` を自分自身に送るが、テストは `invoker.system_invoke(...)` で直接 system message を処理するため queued Stop は drain されず `cell.is_terminated()` は false のまま維持される。
> - `cell.report_failure` 内部で `self.mailbox().suspend()` が呼ばれるため、続く `invoker.system_invoke(SystemMessage::Recreate(cause))` は `fault_recreate` の AC-H3 precondition (mailbox suspended) を自然に満たす。**追加の `cell.mailbox().suspend()` 呼び出しは不要** (AC-H4 テストとはここが異なる)。
> - 直接 `system_invoke(Resume / Recreate)` を呼ぶのは AC-H4 家族テスト (`tests.rs:1362, 1409` 等) と同じパターンで、ActorCellInvoker を `ActorCellInvoker { cell: cell.downgrade() }` として構築する。

- [x] 4.1 テスト `ac_m3_report_failure_records_self_as_perpetrator`:
  - GIVEN `ActorCell::create(...)` で新規 cell を起動し、`SystemMessage::Create` で pre_start を完走させる
  - AND `cell.is_failed() == false` を事前確認
  - WHEN `cell.report_failure(&ActorError::recoverable("test"), None)` を直接呼ぶ (注: `report_failure` は module-private `fn` だが、`actor_cell/tests.rs` は `actor_cell` の submodule のため private method に直接アクセス可能。既存の `handle_raw` 拡張 impl と同じアクセス経路)
  - THEN `cell.is_failed() == true` かつ `cell.perpetrator() == Some(cell.pid())`
  - AND `cell.is_failed_fatally() == false`
- [x] 4.2 テスト `ac_m3_duplicate_report_failure_preserves_perpetrator`:
  - GIVEN Task 4.1 と同じ setup、かつ初回 `report_failure` 後 `perpetrator == Some(self.pid)` を確認
  - WHEN 2 回目 `report_failure(&ActorError::recoverable("retry"), None)` を呼ぶ
  - THEN `cell.perpetrator() == Some(self.pid)` のまま不変 (overwrite されない)
  - AND `cell.is_failed() == true` を維持
- [x] 4.3 テスト `ac_m3_report_failure_preserves_fatal_state`:
  - GIVEN `cell.set_failed_fatally()` で事前に fatal 化、`cell.is_failed_fatally() == true` を確認
  - WHEN `cell.report_failure(&ActorError::recoverable("after fatal"), None)` を呼ぶ
  - THEN `cell.is_failed_fatally() == true` のまま (Fatal は downgrade されない、既存の `set_failed` 実装 guard で担保)
  - AND `cell.perpetrator() == None` (Fatal 状態では perpetrator は常に None、`actor_cell.rs:431-436` の実装)
- [x] 4.4 テスト `ac_m3_restart_clears_perpetrator` (regression guard):
  - **実装時発見**: orphan cell の `cell.report_failure()` は `system.report_failure` → `send_system_message(Stop)` を呼び、sync dispatcher 上で inline 処理される結果 `cell.is_terminated() == true` になる。このため後続の `system_invoke(Recreate)` が L1549 の `is_terminated` guard で early return し、`fault_recreate` すら呼ばれない。
  - **解決**: AC-H4-T1 と同パターンで事前状態を直接仕込む。`cell.set_failed(cell.pid())` + `cell.mailbox().suspend()` で failure 後の状態を再現し、`system_invoke(Recreate)` で restart cycle を起動する。`report_failure` wiring 自体は Task 4.1-4.3 で別途 pin 済み
  - GIVEN cell を起動 → Create → `cell.set_failed(cell.pid())` + `cell.mailbox().suspend()` で事前状態
  - WHEN `invoker.system_invoke(SystemMessage::Recreate(cause))` を呼ぶ
  - THEN restart 完了後に `is_failed() == false` かつ `perpetrator() == None`
  - AND 次の `set_failed(cell.pid())` で新しい perpetrator が記録される (cycle 回帰)
- [x] 4.5 テスト `ac_m3_resume_clears_perpetrator` (Task 3.4 対応の regression guard):
  - **実装時発見**: Task 4.4 と同じ orphan-Stop race が発生するため、直接 state 操作方式を採用
  - GIVEN cell を起動 → Create → `cell.set_failed(cell.pid())` で事前状態
  - WHEN `invoker.system_invoke(SystemMessage::Resume)` を呼ぶ
  - THEN Resume 処理後に `is_failed() == false` かつ `perpetrator() == None`
  - AND 次の `set_failed(cell.pid())` で新しい perpetrator が記録される
  - **注意 1**: Fatal 状態での Resume は Pekko 側も skip または restart 変換になるため、本テストは Recoverable 失敗 → Resume サイクルのみ検証する
  - **注意 2 (Decision 5 Divergence 2)**: 本テストは orphan cell (子なし) で組むため `resume_children()` は no-op。子孫への Resume 過剰 clear 挙動は本 change の scope 外なのでテストしない (将来の Resume cause carry 拡張で Pekko 準拠化する際に回帰テストを追加)。

## 5. AC-M1: `PinnedDispatcher` の rustdoc 補強

- [x] 5.1 `pinned_dispatcher.rs:25-28` struct `PinnedDispatcher` の rustdoc に、Pekko `PinnedDispatcher.scala:44-59` (owner field + register / unregister) との対応表を追加
- [x] 5.2 `register_actor` (`pinned_dispatcher.rs:59-74`) の rustdoc に、3 分岐と Pekko `PinnedDispatcher.scala:48-54` の対応を明記:
  - `None` (owner 未設定) ↔ Pekko `actor eq null` ブランチ (無条件で `owner = actorCell` 後 `super.register`)
  - `Some(existing) if existing == pid` (同一 actor の再 attach) ↔ Pekko `actor ne null && actorCell eq actor` (if 判定 false で throw されずに `owner = actorCell` → 実質 idempotent)
  - `Some(_)` (別 actor による attach 試行) ↔ Pekko `actor ne null && actorCell ne actor` (throw `IllegalArgumentException`; fraktor-rs は `SpawnError::DispatcherAlreadyOwned` を `Err` として返す)
- [x] 5.3 `unregister_actor` (`pinned_dispatcher.rs:76-85`) の rustdoc に、`owner == pid` 条件付きクリアが Pekko `PinnedDispatcher.scala:56-59` の `unregister` method 経路と等価である旨を記述。Pekko は無条件で `owner = null` を代入するが、fraktor-rs は `owner == pid` 一致時のみクリアする (他 actor の unregister 呼び出しでは owner が維持される)。この差は Pekko 側が `detach` 呼び出し元で owner 一致を前提している semantics を fraktor-rs が API 側で防御的に実装した結果であり、契約上の違反はない旨を明記
- [x] 5.4 struct level rustdoc に、並行安全性が `&mut self` + 外部 `MessageDispatcherShared` mutex で成立する旨の 1 段落を追加 (Pekko `@volatile` + 外部 `attach/detach` lock パターンとの対応に触れる。AtomicCell 化は不要)

## 6. gap-analysis 更新

- [x] 6.1 `docs/gap-analysis/actor-gap-analysis.md` の AC-M1 行を `~~medium~~ done (change pekko-fault-dispatcher-hardening)` に書き換え、description を「実装・テスト完備 (rustdoc 補強のみ実施)」に更新
- [x] 6.2 同 AC-M3 行を `~~medium~~ done (change pekko-fault-dispatcher-hardening)` に書き換え、description を「`report_failure` 先頭に `is_failed()` guard + `set_failed(self.pid)` を配線、Pekko `FaultHandling.scala:221-222` 等価」に更新
- [x] 6.3 第 14 版エントリを「指標」表に追加 (`内部セマンティクスギャップ数 (第14版、AC-M1/M3 完了反映後)`)、medium カウントを 10 → 8 に更新
- [x] 6.4 まとめセクションの「第13版で AC-M5 (NotInfluenceReceiveTimeout marker + Identify 内部封筒化) を完了」の行の後に「第14版で AC-M1 (PinnedDispatcher 1:1 排他契約の rustdoc 補強) / AC-M3 (isFailed guard + setFailed perpetrator 記録) を完了」を追記
- [x] 6.5 「第10版時点の残存ギャップ」表現を「第14版時点の残存ギャップ: medium 8 件」に更新

## 7. 機械的検証 (grep gate + CI)

- [x] 7.1 `grep -rn "\.set_failed\(" modules/actor-core/src/core/kernel/actor/actor_cell.rs` で Task 2.1 の新規 call site が 1 箇所以上 hit
- [x] 7.2 `grep -rn "is_failed\(\)" modules/actor-core/src/core/kernel/actor/actor_cell.rs` で Task 2.1 の guard が 1 箇所以上 hit (既存の `is_failed_fatally` call は対象外)
- [x] 7.3 `grep -rn "PinnedDispatcher\.scala" modules/actor-core/src/core/kernel/dispatch/dispatcher/pinned_dispatcher.rs` で Task 5.1-5.4 の Pekko 参照が 1 箇所以上 hit
- [x] 7.4 `grep -rn "FaultHandling\.scala:221\|FaultHandling\.scala:222" modules/actor-core/src/core/kernel/actor/actor_cell.rs` で Task 2.2 の Pekko 参照が 1 箇所以上 hit
- [x] 7.5 `grep -rn "clear_failed\(\)" modules/actor-core/src/core/kernel/actor/actor_cell.rs` で Task 3.4 の Resume arm 新規追加が 1 箇所以上 hit (既存 `actor_cell.rs:1264` の `finish_recreate` と合わせて **2 箇所以上**)
- [x] 7.6 `grep -rn "FaultHandling\.scala:150" modules/actor-core/src/core/kernel/actor/actor_cell.rs` で Task 3.4 の Pekko 参照が 1 箇所以上 hit
- [x] 7.7 `openspec validate pekko-fault-dispatcher-hardening --strict` が valid

## 8. Pekko 参照検証

- [x] 8.1 `references/pekko/actor/src/main/scala/org/apache/pekko/actor/dungeon/FaultHandling.scala:73-74` (`isFailed` / `isFailedFatally` 定義) / `:215-245` (`handleInvokeFailure` 本体) / `:221-222` (`setFailed` 呼び出し 2 分岐) との行対応を rustdoc から参照
- [x] 8.2 `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/PinnedDispatcher.scala:48-53` (`attach` 排他チェック) との行対応を `pinned_dispatcher.rs` の rustdoc から参照
- [x] 8.3 本 change で Pekko 非互換を新たに作っていないことを確認: 既存の AC-H3 拡張テスト (`ac_h3_ext_t1`-`ac_h3_ext_tN`) / `user_message_failure_does_not_reschedule_receive_timeout` / `kill_user_message_reports_fatal_failure` / `pinned_dispatcher/tests.rs` の 5 件が引き続き pass

## 9. CI / lint の final ゲート

- [x] 9.1 `./scripts/ci-check.sh ai all` が exit 0
  - dylint 8 lint 全 pass
  - cargo test / clippy / fmt が全 pass
  - 本 change で `#[ignore]` 新規付与なし

## 10. PR 作成 / マージ / アーカイブ

- [ ] 10.1 `feat(actor-core): wire setFailed perpetrator + AC-M1 rustdoc hardening (AC-M1/M3)` という題で PR を作成、本 change の change name をリンク
- [ ] 10.2 PR 本文に以下を含める:
  - Pekko `FaultHandling.scala:73-74, 221-222` / `PinnedDispatcher.scala:48-53` との対応表
  - **公開 API 変更**: なし (`report_failure` / `set_failed` / `is_failed` は既存公開、挙動追加のみ)
  - **破壊的変更**: なし
  - **テスト**: AC-M3 シナリオ 3 件 + 既存 AC-H3 拡張 / PinnedDispatcher 5 件の regression 保証
  - gap-analysis AC-M1 / AC-M3 done 化、第 14 版 medium 10 → 8
- [ ] 10.3 レビュー対応: CodeRabbit / Cursor Bugbot の指摘が来た場合は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve
- [ ] 10.4 マージ後、別 PR で change をアーカイブ + main spec を `openspec/specs/pekko-fault-dispatcher-hardening/spec.md` に sync
