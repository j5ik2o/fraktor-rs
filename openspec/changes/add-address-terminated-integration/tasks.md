## 1. ベースラインと契約確認

- [x] 1.1 Pekko `RemoteWatcher` / `RemoteDaemon` の address termination 挙動を確認し、関連する参照行を実装メモまたはテストコメントに記録する。
- [x] 1.2 編集前に、現在の fraktor event stream variant、classifier key、watcher effect、std watcher task 境界を確認する。
- [x] 1.3 actor-core event stream、remote-core watcher state、remote-adaptor-std watcher / deployment 経路の対象ベースラインテストを実行する。

## 2. actor-core event stream 公開面

- [x] 2.1 std 専用型を使わず `remote-core` にも依存しない形で、terminated remote authority string、reason、monotonic millis observation timestamp を保持する `AddressTerminated` event payload 型を追加する。
- [x] 2.2 `EventStreamEvent::AddressTerminated`、`ClassifierKey::AddressTerminated`、public re-export、clone / classifier mapping support を追加する。
- [x] 2.3 event stream classifier と subchannel tests を更新し、`AddressTerminated` が concrete-key subscription と `ClassifierKey::All` の両方で扱われるようにする。
- [x] 2.4 actor-core の no_std compatibility を維持し、event stream 型へ std runtime dependency を導入しない。

## 3. remote-core watcher state

- [x] 3.1 std publication 用に、unavailable になった `remote-core` address、reason metadata、monotonic millis observation timestamp を含む address-level termination の watcher effect を追加する。
- [x] 3.2 failure epoch 内で remote node が初めて unavailable になったとき、`WatcherState::handle(HeartbeatTick)` から address termination effect を emit する。
- [x] 3.3 address termination を emit する場合でも、watched remote actor 向けの既存 `NotifyTerminated` effect を維持する。
- [x] 3.4 one-shot address termination emission、repeated tick suppression、heartbeat / heartbeat-response reset の watcher state tests を追加する。

## 4. std watcher による発行

- [x] 4.1 `remote-core` address を actor-core authority string に map し、actor system event stream 経由で `EventStreamEvent::AddressTerminated` を発行することで、新しい watcher effect を `remote-adaptor-std` に適用する。
- [x] 4.2 local watcher への `DeathWatchNotification` delivery は既存 actor-core 経路に維持し、address termination publication に置き換わっていないことを検証する。
- [x] 4.3 address termination publication、event classifier filtering、同時 DeathWatch notification delivery の std watcher tests を追加する。

## 5. remote deployment cleanup

- [x] 5.1 watcher task から deployment code を直接呼ぶのではなく、remote deployment watcher / dispatcher state が `ClassifierKey::AddressTerminated` を購読する。
- [x] 5.2 pending deployment start timestamp を追跡し、pending request より古い replayed address termination event を無視する。
- [x] 5.3 terminated authority に対する pending deployment request は、timeout ではなく address termination 固有の error で失敗させる。
- [x] 5.4 cleanup 済み correlation id に対する late deployment response を stale response として reject し、deployment を failed state のまま維持する。
- [x] 5.5 pending deployment failure、replayed old termination suppression、address termination 後の late response rejection の unit または integration tests を追加する。

## 6. 統合、spec、ドキュメント

- [x] 6.1 remote node failure が address termination を発行し、local DeathWatch watcher へも引き続き通知することを示す対象 integration test を追加する。
- [x] 6.2 実装後に `docs/gap-analysis/remote-gap-analysis.md` を更新し、`AddressTerminated` の残ギャップを削除する。
- [x] 6.3 影響 crate の対象テストを実行し、その後 `mise exec -- openspec validate add-address-terminated-integration --strict` と `git diff --check` を実行する。
- [x] 6.4 より狭い検証範囲がユーザー承認で明示的に選ばれない限り、change を complete とする前に `./scripts/ci-check.sh ai all` を実行する。
