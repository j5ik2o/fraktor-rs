## Context

本 change は gap-analysis 第 13 版で残る medium semantics gap のうち、fault handling と dispatcher 排他の 2 件 (AC-M1 / AC-M3) を閉塞する。調査 (`openspec/changes/pekko-fault-dispatcher-hardening/` の調査フェーズ) の結果、両項目の現状は gap-analysis の description とは若干異なっていた:

### AC-M1 現状
`PinnedDispatcher::register_actor` / `unregister_actor` (`modules/actor-core/src/core/kernel/dispatch/dispatcher/pinned_dispatcher.rs:59-85`) は Pekko `PinnedDispatcher.scala:48-53` 同等の排他ガード (`owner: Option<Pid>` + 3分岐 match + `SpawnError::DispatcherAlreadyOwned`) を完全実装済み。並行安全性は `MessageDispatcherShared` の外側 mutex で serialization される (`&mut self` 受け取り) ため race なし。テストも 5 件 (`pinned_dispatcher/tests.rs:72-127`) で契約を pin 済み (`register_actor_rejects_second_owner` / `register_actor_allows_same_actor_to_reattach` / `unregister_actor_clears_owner_after_detach` / `detach_then_new_owner_can_register` / `new_normalises_throughput_and_deadline`)。

**→ AC-M1 は実装・テスト完備**。本 change での実装作業は rustdoc 補強と Pekko 参照 line 追加のみ。gap-analysis description が古い (本 change でも同時修正)。

### AC-M3 現状
- `FailedInfo` enum (`failed_info.rs`: `None` / `Child(Pid)` / `Fatal`) と `is_failed` / `is_failed_fatally` / `set_failed` / `set_failed_fatally` / `clear_failed` / `perpetrator` メソッド一式 (`actor_cell.rs:410-474`) は実装済み。
- `fault_recreate` の先頭で `is_failed_fatally()` guard が配線済み (`actor_cell.rs:1188-1190`)。
- `finish_recreate` の pre_start 失敗パスで `set_failed_fatally()` → `report_failure()` の順が配線済み (`actor_cell.rs:1288-1289`)。

**しかし次の 2 点が欠けている**:
1. `set_failed(perpetrator)` が production 経路から **一切呼ばれていない** (`actor_cell.rs` grep で call site = 0)。Pekko `FaultHandling.scala:221-222` の `handleInvokeFailure` 内で `isFailed` guard を通して `setFailed(child)` / `setFailed(self)` を呼ぶ経路が、fraktor-rs の `report_failure` / `handle_failure` に写像されていない。
2. 結果として `FailedInfo::Child(perpetrator)` 状態は production で観測されない (state は enum としてのみ存在)。重複 `report_failure` が来ても state flag が立たず、perpetrator 情報が残らない。

`report_failure` (`actor_cell.rs:1352-1363`) は現在: mailbox suspend → children suspend → system.report_failure という 3 ステップだけで、`_failed` state mutation を行わない。本 change で Pekko 互換の `setFailed(self)` を組み込む。

## Goals / Non-Goals

**Goals:**
- Pekko `FaultHandling.scala:215-245` `handleInvokeFailure` の `_failed: FailedInfo` state 記録経路を fraktor-rs の `report_failure` / `handle_failure` に配線する。
- `report_failure` エントリで `is_failed()` guard を通して `set_failed(self.pid)` / `set_failed(child_pid)` を呼び、perpetrator 情報を `FailedInfo::Child(_)` として保持する。重複コール時は既存の perpetrator を overwrite しない。
- AC-M1 の rustdoc / Pekko 参照強化 (実装変更なし)。
- gap-analysis: AC-M1 / AC-M3 を done 化、medium カウント 10 → 8、第 14 版エントリ追加。

**Non-Goals:**
- `FailedInfo::Child(pid)` 状態を観測する production 挙動変更 (例: perpetrator ベースの supervisor directive 分岐) — 本 change は state 記録のみ、読み取り側の挙動変更は含まない。既存の supervisor 経路は Pekko 同様、state 判定ではなく `FailurePayload` 直接評価で動く。
- `isFailed` guard で supervisor 通知を skip する変更 — Pekko も `handleInvokeFailure` は常に `sendSystemMessage(Failed)` を送る (`FaultHandling.scala:231-234`)。guard は state 書き込みに対してのみ働き、notify は常に発火するので、fraktor-rs の `system.report_failure` も常に呼ぶ。
- AC-M2 (dispatcher config alias 連鎖解決) / AC-M4 (watchWith 重複) / AL-M1 / 他 medium。
- `PinnedDispatcher` の設計変更 (現行で完結している)。

## Decisions

### Decision 1: `set_failed(perpetrator)` の配線位置と順序 — `report_failure` の mailbox.suspend と suspend_children の間

**選択**: `report_failure(&self, error, snapshot)` 内で、Pekko `handleInvokeFailure:215-234` の実行順を行単位写像する。`set_failed` は `mailbox().suspend()` の **後**、`suspend_children()` の **前** に置く:

```rust
// Pekko `FaultHandling.scala:218` suspendNonRecursive()
self.mailbox().suspend();
// Pekko `FaultHandling.scala:222` case _ if !isFailed => setFailed(self)
// fraktor-rs の report_failure は user / system message 処理失敗で呼ばれる
// self-failure 経路なので、perpetrator は常に self.pid。child perpetrator
// 分岐 (FaultHandling.scala:221) は handle_failure 側の責務であり、かつ
// fraktor-rs の snapshot に child_pid を持ち込む現行要件がないため省略する
// (Decision 3 参照)。is_failed() guard で重複記録を抑止する。
if !self.is_failed() {
  self.set_failed(self.pid);
}
// Pekko `FaultHandling.scala:225` suspendChildren(exceptFor = skip)
self.suspend_children();
let timestamp = self.system().monotonic_now();
let payload = FailurePayload::from_error(self.pid, error, snapshot, timestamp);
// Pekko `FaultHandling.scala:231-234` parent.sendSystemMessage(Failed(...))
self.system().report_failure(payload);
```

**順序の根拠**: 機能的には `set_failed` / `mailbox.suspend` / `suspend_children` は独立操作 (データ依存なし) で順序は等価だが、Pekko 行単位 parity を厳密に取ることで「未来の Pekko 互換要件 (child perpetrator / skip set 等) を追加する際にずれが発生しにくい」メリットがある。

**代案 (採用しない)**:
- (a) `handle_failure(&self, payload)` 入口に書く: child failure の supervisor 処理側で、親の `_failed` state を触るのは Pekko と一致しない (Decision 2 で除外)。
- (b) `ActorCellInvoker::invoke` の Err branch に書く: invoke 側は message-processing layer、`report_failure` は supervision-reporting layer で責務が異なる。Pekko も `handleInvokeFailure` 内で state mutation + parent notify を 1 関数でまとめている。
- (c) `snapshot` から child pid を取り出して分岐: 現行 `FailureMessageSnapshot` に child pid 情報を持ち込む必要がなく、意味論上も self-failure が主経路のため YAGNI (Decision 3)。

**根拠**: `report_failure` は現状「failure を supervisor に報告する」single-purpose method で、state mutation を追加しても責務が膨張しない。`is_failed()` guard で idempotent 化し、Fatal 状態は既存の `set_failed` 実装 (`actor_cell.rs:448`) の内部 guard で保持される。

### Decision 2: child failure 側も `handle_failure` で `set_failed(child_pid)` を呼ぶ

**選択**: `handle_failure(&self, payload: &FailurePayload)` (`actor_cell.rs:1368-`) の入口で、child failure 処理中であることを `set_failed(payload.child())` で記録する。ただし Pekko では `handleInvokeFailure` が `Envelope(Failed, child)` のパターンマッチで child perpetrator を設定する経路で、`handle_failure` は「親が supervisor として子の failure を処理する」経路。Pekko の `handleFailure` は親側なので state mutation ではなく supervisor decision のみを行う。

**→ 再検討**: fraktor-rs の `handle_failure` は Pekko の `handleFailure` (親側、supervisor decision) 相当で、`set_failed` は不要。親が自身の state を failed にするのは自分が親から recovery 指示を受けた場合の話。**Decision 2 はスコープ外とする**。Decision 1 のみで Pekko 互換性を満たす (self-failure の perpetrator 記録)。

### Decision 3: `FailureMessageSnapshot` から child pid を取り出す API の扱い

**選択**: `FailureMessageSnapshot` に child perpetrator を示す field があるか確認し、なければ self.pid を perpetrator にする simple 経路を採用する。Pekko `handleInvokeFailure` の child 分岐は `currentMessage` が `Envelope(Failed, child)` だった場合のみで、一般の user message 処理失敗時は self-failure (`setFailed(self)`)。fraktor-rs の `report_failure` は主に user message / system message 処理失敗で呼ばれるため、**self-failure 単純経路で十分**。

**→ 単純化**: `set_failed(self.pid)` を `is_failed()` guard 付きで呼ぶだけ。child perpetrator 分岐は YAGNI (本 change の gap-analysis 契約に入っていない)。

**[既知の divergence (Round 5 で発見)]**: `actor_cell.rs:1379` の `on_child_failed` Err 経路での `self.report_failure(error, None)` は、Pekko `FaultHandling.scala:221` `case Envelope(Failed, child) if !isFailed => setFailed(child)` に対応する **child perpetrator** ケース。本 change では self.pid を記録するため `perpetrator() == Some(self.pid)` になるが、Pekko では `perpetrator() == child_pid`。

- **現状の影響**: `perpetrator()` の戻り値を読む production call site はゼロなので不可視。
- **将来の対処**: child perpetrator ケースを要する supervisor strategy 拡張を実装する際に、`report_failure(error, snapshot, perpetrator_hint: Option<Pid>)` のような signature 拡張で対応する。本 change のスコープ外。

### Decision 5: Resume 経路での `clear_failed` 呼び出しは unconditional (Pekko 挙動との 2 種の divergence を受容)

**選択**: `SystemMessage::Resume` arm に追加する `clear_failed()` は、`FailedInfo::Fatal` も含めた unconditional reset (既存 `clear_failed()` = `actor_cell.rs:470-474`) をそのまま使う。Pekko `FaultHandling.scala:150` は `finally if (causedByFailure ne null) clearFailed()` で 2 つの抑制を行うが、本 change は両方とも受容する:

**Divergence 1: Fatal 状態保持**: Pekko `FaultHandling.scala:83-86` は `case FailedRef(_) => NoFailedInfo; case other => other` で Fatal を保持 (Fatal 状態で Resume が来たら clearFailed でも reset されない)。fraktor-rs の既存 `clear_failed()` は unconditional reset。
- **影響**: production で Fatal + Resume の経路は実質存在しない (`set_failed_fatally` の production call site は `finish_recreate` の post_restart 失敗パスのみ、その後の supervisor 判断は Restart / Stop が typical、Resume 選択は strategy 明示指定時のみ)。
- **受容根拠**: fraktor-rs は Pekko `FaultHandling.scala:141-144` の「Fatal + Resume → Restart 変換」ロジックも未実装。将来この Resume-to-Restart 変換を入れる際に `clear_failed_non_fatal()` variant を同時追加する前提で、現状は unconditional で統一する。

**Divergence 2: 子孫への Resume propagation で `clear_failed` が cascade する**: Pekko `resumeChildren(cause, perp)` は perp 以外の子に `causedByFailure = null` を伝え、受け取った子の `faultResume` で `clearFailed()` が skip される。fraktor-rs の `SystemMessage::Resume` は cause を carry しないため、propagation で `resume_children()` が送る各子の Resume arm でも `clear_failed()` が走る。
- **影響**: 子孫が独立 failure で `FailedInfo::Child(_)` になっている状態で親経由の Resume propagation が到達すると、子孫の state が過剰 clear される。具体的 edge case: (1) grandchild が独立 failure で supervisor directive 待ち、(2) 同時に親 child が別原因 failure、(3) grandparent が child に Resume 発行、(4) child の Resume arm が `resume_children` で grandchild に Resume を伝播、(5) grandchild の Resume arm で grandchild.state も clear される。
- **受容根拠**: 極めて narrow な race condition。`perpetrator()` の戻り値を読む production 経路はゼロ (本 change 後も同様) のため、観測可能な挙動差は発生しない。
- **将来の対処**: `SystemMessage::Resume { cause: Option<ActorErrorReason> }` variant の導入で Pekko 準拠に格上げ可能。本 change のスコープ外 (後続の AC-M* か Resume semantics 拡張 change で対応)。

**代案 (採用しない)**:
- `clear_failed_non_fatal()` の新設 (Divergence 1 対策): YAGNI。
- `SystemMessage::Resume` の cause carry 拡張 (Divergence 2 対策): scope creep。AC-M3 の核心は `set_failed(self.pid)` 配線であり、Resume API 拡張は別軸の Pekko parity 作業。

### Decision 4: AC-M1 の実装修正範囲 = ゼロ

**選択**: AC-M1 は `pinned_dispatcher.rs` の実装・テスト完備。rustdoc に Pekko `PinnedDispatcher.scala:48-53` 参照を追記し、`register_actor` の match 分岐 3 パターンを Pekko の `if (actor ne null) && (actorCell != actor)` と行単位で対応させる comment を追加するのみ。

**代案 (採用しない)**:
- 並行負荷テスト追加: `&mut self` + 外部 mutex で serialize されるため race test は意味がない。
- `owner` を `AtomicCell<Option<Pid>>` に変更: 不要な複雑化。

## Risks / Trade-offs

- **[Risk / 実装前提] `clear_failed()` が restart 成功経路で呼ばれていない場合、`set_failed(self.pid)` 追加で state leak が発生する**: 事前調査 (Round 2) で `actor_cell.rs:1264` の `finish_recreate` 内で `clear_failed()` が既に呼ばれていることを確認済み (Case B = call site あり)。Pekko `FaultHandling.scala:173` `finishCreate` / `FaultHandling.scala:284` (finishRecreate 想定) と同等の位置。→ **restart サイクルは既存配線で OK**。
- **[Risk / 実装前提 (重要)] `SystemMessage::Resume` arm が `clear_failed()` を呼んでいない**: `actor_cell.rs:1573-1579` の Resume arm は `cell.resume_children()` のみ実行し、`clear_failed()` を呼ばない。Pekko `FaultHandling.scala:150` は `faultResume` 内で `finally if (causedByFailure ne null) clearFailed()` を呼ぶため、本 change で `set_failed(self.pid)` を追加すると、supervisor が Resume directive を返した場合に `FailedInfo::Child(self.pid)` が永久に残り `is_failed()` が stale になる。→ Mitigation: Task 3.4 (新設) で `SystemMessage::Resume` arm 先頭に `cell.clear_failed();` を追加する。fraktor-rs の Resume は causedByFailure パラメータを持たないため unconditional クリアになるが、`FailedInfo::None` 状態での `clear_failed()` は no-op なので安全 (grandchild への Resume propagation でも害なし)。
- **[Risk] restart 後に stale perpetrator が残る**: 上記 2 件と関連。Mitigation: `ac_m3_restart_clears_perpetrator` regression test (Task 4.4) + `ac_m3_resume_clears_perpetrator` regression test (Task 4.5、新設) で 2 経路とも state clear を pin する。
- **[Non-Goal 確認] fault_resume-to-restart 変換**: Pekko `FaultHandling.scala:141-144` は `isFailedFatally && causedByFailure != null` のときに Resume を Restart へ変換するが、fraktor-rs の SystemMessage::Resume は causedByFailure を持たず、この変換は現状 AC-H3+ 拡張でも未実装。本 change のスコープ外 (将来の別 change で対応)。
- **[Risk] AC-M1 を「修正ゼロ」扱いで済ませると、gap-analysis を軽率に done 化したと見える**: → Mitigation: proposal / design / tasks で AC-M1 の実装完備状態を Pekko 行単位対応付けで明示し、rustdoc にも Pekko 参照を追加することで「確認済みで done」を根拠化。
- **[Trade-off] perpetrator 情報を記録するが production 経路で読み取らない**: 将来 `perpetrator()` を使う supervisor strategy 拡張 (例: cluster singleton の child perpetrator-based routing) が来たときに wire-up 済み → Mitigation: 本 change の rustdoc で「将来の拡張ポイント」を明示。

## Migration Plan

破壊的 API 変更なし。既存 caller への影響なし (`report_failure` は module-private = `fn` で可視性修飾なし、同一モジュールの submodule `tests` から呼び出し可能; `set_failed` は `pub` だが外部 caller はゼロ = grep で確認済み)。

1. AC-M3 の `set_failed` wiring 追加 + `is_failed()` guard
2. テスト追加 (AC-M3: `report_failure` 二重呼び出しで perpetrator が overwrite されない / 初回は self.pid が記録される)
3. AC-M1 rustdoc 補強
4. gap-analysis 更新 (AC-M1 / AC-M3 done、第 14 版、medium 10 → 8)
5. `./scripts/ci-check.sh ai all` pass
6. PR 作成 → レビュー対応 → マージ → アーカイブ

## Open Questions

- [解決済] `FailureMessageSnapshot` の child perpetrator field 有無 → Decision 3 で「不要 (YAGNI)」と判断。self.pid のみを記録する。
- [解決済] `handle_failure` (child supervisor path) で `set_failed` を呼ぶべきか → Decision 2 で「Pekko `handleFailure` 相当であり不要」と判断。
