## Why

gap-analysis `docs/gap-analysis/actor-gap-analysis.md` の第 13 版時点で残る medium semantics gap のうち、fault handling と dispatcher 層に属する 2 件 (AC-M1 / AC-M3) を同一 change で閉塞する。

- **AC-M1**: `PinnedDispatcher` の「1 actor 1 thread」契約について、`pinned_dispatcher.rs:59-74` に排他ガード (`register_actor` で `owner: Option<Pid>` を確認して `SpawnError::DispatcherAlreadyOwned` を返す) は存在するが、`unregister_actor` 時の owner クリアと並行登録の ordering が `MessageDispatcherShared` の lock 取得粒度に依存しており、shutdown と新規 attach の競合に対する挙動が未固定。Pekko `PinnedDispatcher.scala:48-53` は `attach` 側で `if ((actor ne null) && actorCell != actor) throw` を同期ブロック内で評価するため、ordering に依存せず排他が成立する。
- **AC-M3**: `FailedInfo` enum (`None` / `Child(Pid)` / `Fatal`) および `is_failed` / `is_failed_fatally` / `set_failed` / `set_failed_fatally` は実装済みで、`fault_recreate` の先頭で `is_failed_fatally` guard も配線済み (`actor_cell.rs:1188-1190`)。しかし **Pekko `FaultHandling.scala:73-74` が `handleInvokeFailure` の冒頭で評価する `if (isFailed) ... 重複 fail を抑制` 相当の guard が、fraktor-rs の `report_failure` 入口に存在しない**。結果として、失敗処理中の actor が続くメッセージで再度 failure を report する競合が起きた場合に、stash 経由の handle_failure が二重駆動される余地が残っている (実際のシステムメッセージフローでは現状問題化していないが、契約レベルで欠けている)。

同時に閉じることで medium カウントを **第 13 版 10 → 第 14 版 8** に進める (AC-M1 + AC-M3 の 2 件分)。

## What Changes

- AC-M1: `PinnedDispatcher::register_actor` / `unregister_actor` の並行安全性を明示し、shutdown + 同時 attach の ordering を固定する。必要なら `owner` の CAS 化または lock 保持期間の明確化を行う。Pekko 互換テストで「同一 dispatcher に 2 actor が同時に attach を試みたら後者が `DispatcherAlreadyOwned` で reject される」ケースをピン留めする。
- AC-M3: `ActorCell::report_failure` (およびその周辺の fault entry point) に `is_failed()` guard を追加し、既に fail 処理中の actor に対する重複 `report_failure` を抑制する。Pekko `FaultHandling.scala:73-74` の `if (isFailed) ... return` を行単位で写像し、rustdoc で参照する。
- テスト追加: 両 gap についてシナリオピンテストを新設 (AC-M1 は dispatcher レベル、AC-M3 は actor_cell レベル)。
- gap-analysis 更新: AC-M1 / AC-M3 を done に書き換え、第 14 版エントリを追加。

## Capabilities

### New Capabilities
- `pekko-fault-dispatcher-hardening`: Pekko `FaultHandling` の失敗フラグ semantics と `PinnedDispatcher` の排他契約を fraktor-rs 側で Pekko 準拠に整合させる capability。`isFailed` / `isFailedFatally` の意味論的等価と、`1 actor 1 thread` 契約の並行順序を静的に保証する。

### Modified Capabilities

(なし)

## Impact

- 触るファイル:
  - `modules/actor-core/src/core/kernel/actor/actor_cell.rs` — `report_failure` エントリに `is_failed()` guard 追加
  - `modules/actor-core/src/core/kernel/actor/actor_cell/tests.rs` — AC-M3 シナリオテスト追加
  - `modules/actor-core/src/core/kernel/dispatch/dispatcher/pinned_dispatcher.rs` — 並行安全性の明確化 (必要なら `owner` 書き込み順序の見直し)
  - `modules/actor-core/src/core/kernel/dispatch/dispatcher/pinned_dispatcher/tests.rs` (または新設) — AC-M1 シナリオテスト
  - `docs/gap-analysis/actor-gap-analysis.md` — AC-M1 / AC-M3 done 化 + 第 14 版エントリ

- 公開 API:
  - **追加なし / 破壊的変更なし** の想定。`ActorCell::report_failure` は既に `pub(crate)` で外部公開されていないため、guard 追加は内部実装詳細として扱える。`PinnedDispatcher` 側も `SpawnError::DispatcherAlreadyOwned` が既存なので追加 variant は不要。

- 依存: なし (Pekko 参照のみ)。
- 非対象: AC-M2 (dispatcher config alias 連鎖解決) / AC-M4 (watchWith 重複) / AC-M5 (完了済) / AL-M1 (AL-H1 時点で実質完了、gap-analysis は別 PR で done 化予定) は本 change のスコープ外。
