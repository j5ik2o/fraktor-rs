## 1. detach orchestration の変更

- [ ] 1.1 `MessageDispatcherShared::detach` が direct cleanup ではなく close request + finalizer handoff を行うように変更する
- [ ] 1.2 detach 中に dispatcher write lock を保持したまま queue cleanup しないことをテストで固定する
- [ ] 1.3 delayed shutdown scheduling との順序関係を確認し、必要なら spec に明文化する

## 2. mailbox finalizer 実装

- [ ] 2.1 `Mailbox` に finalizer orchestration helper を追加する
- [ ] 2.2 idle mailbox では detach caller が immediate finalize できるようにする
- [ ] 2.3 running mailbox では in-flight runner が finalizer を引き受けるようにする
- [ ] 2.4 finalizer が queue cleanup を exactly once で実行することを確認する
- [ ] 2.5 `LeaveSharedQueue` で shared queue を drain しないことを維持する
- [ ] 2.6 finalizer election の authoritative CAS を `MailboxScheduleState` に閉じ込め、`Mailbox` 側に重複判定ロジックを持ち込まない

## 3. run loop の close request 対応

- [ ] 3.1 `run()` が close request 観測後に通常の user dequeue を継続しないようにする
- [ ] 3.2 in-flight message は最大 1 件のみ通常処理され得ることをテストで固定する
- [ ] 3.3 close request 後の残 user queue が dead-letter / cleanup policy に従って処理されることを確認する
- [ ] 3.4 close request 後は `needs_reschedule` が通常 scheduling を再武装しないことを確認する
- [ ] 3.5 close request 観測後に新しい system dequeue を始めないことをテストで固定する
- [ ] 3.6 close request を観測した `run()` が terminal path で通常 reschedule を返さないことを確認する

## 4. 回帰テスト

- [ ] 4.1 idle mailbox detach が caller finalizer で cleanup 完了する test を追加する
- [ ] 4.2 running mailbox detach が runner finalizer で cleanup 完了する test を追加する
- [ ] 4.3 finalizer が二重実行されない test を追加する
- [ ] 4.4 sharing mailbox (`LeaveSharedQueue`) が shared queue を drain しない test を追加する
- [ ] 4.5 既存の close correctness / stash ordering / balancing dispatcher tests に回帰がないことを確認する

## 5. 後続 change への橋渡し

- [ ] 5.1 `user_queue_lock` に残る責務を inventory 化する
- [ ] 5.2 outer lock 削減の次 change で扱う論点（producer race / prepend batch atomicity / metrics snapshot）を proposal に明記する
- [ ] 5.3 `openspec validate mailbox-cleanup-ownership-handoff --strict` を通す
