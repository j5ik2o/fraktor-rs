## 1. Wire Format

- [x] 1.1 `ControlPdu` に `FlushScope`、`FlushRequest`、`FlushAck` を追加する
- [x] 1.2 `ControlCodec` の encode / decode を flush request / ack subkind に対応させる
- [x] 1.3 flush request / ack の round-trip、unknown flush scope rejection、既存 control PDU 互換の unit test を追加する

## 2. Core Association Flush State

- [x] 2.1 flush id、scope、lane id、expected ack、deadline、ack 済み lane を保持する flush session 型を追加する
- [x] 2.2 `Association` に caller supplied writer lane set で shutdown flush と DeathWatch notification 前 flush を開始する API を追加する
- [x] 2.3 flush 開始前に prior outbound queue が残る場合は completed を返さず、failure / timeout outcome へ進める
- [x] 2.4 `AssociationEffect` に flush request、flush completed、flush timed-out、flush failed を表す effect を追加する
- [x] 2.5 `Remote` / `RemoteShared` に flush start、inbound ack、timer input、connection loss release、outcome observation surface を追加する
- [x] 2.6 `RemoteTransport` に lane-targeted flush request delivery を追加し、flush timer scheduling は std adaptor 側に置く
- [x] 2.7 inbound `FlushAck` と flush timer input を association state に適用する
- [x] 2.8 duplicate ack、timeout、connection lost / quarantine、prior outbound queue pending による pending flush release の unit test を `fraktor-remote-core-rs` に追加する

## 3. DeathWatch Notification Flush Gate

- [x] 3.1 `StdRemoteWatchHook::handle_deathwatch_notification` が remote-bound notification を直接 `OutboundEnqueued` せず std flush gate へ渡すよう更新する
- [x] 3.2 std flush gate が pending notification map、flush start failure、flush outcome、duplicate outcome を管理する
- [x] 3.3 unresolved remote watcher は消費せず、unresolved local target は invalid notification を enqueue しない test を追加する
- [x] 3.4 flush completion、timeout、failure、duplicate completion で pending notification が一度だけ enqueue される unit test を追加する

## 4. Std TCP Flush Transport

- [x] 4.1 core の flush request effect を TCP outbound writer lane ごとの `ControlPdu::FlushRequest` enqueue に接続する
- [x] 4.2 DeathWatch 前 flush は lane `0` を control-only と仮定せず、現行 TCP adaptor の message-capable writer lane 全体を対象にする
- [x] 4.3 inbound `FlushRequest` を actor-core delivery へ進めず、core control handling へ渡して同じ flush id / lane id の `FlushAck` を返す
- [x] 4.4 inbound `FlushAck` を core event loop へ戻し、association flush state を進める
- [x] 4.5 flush request / ack の send failure と lane backpressure が観測可能になる test を追加する

## 5. Std Orchestration

- [x] 5.1 `RemotingExtensionInstaller::shutdown_and_join` を追加または更新し、shutdown flush wait → `RemoteShared::shutdown` → wake → join の順にする
- [x] 5.2 active association がない場合、flush timeout の場合、flush start failure の場合でも shutdown が完了する test を追加する
- [x] 5.3 std run loop が flush outcomes を write lock 外で shutdown waiter / std flush gate へ渡すよう更新する
- [x] 5.4 std flush gate が remote-bound `DeathWatchNotification` を pending map に保持し、flush outcome 後に一度だけ enqueue するよう更新する
- [x] 5.5 DeathWatch 前 flush の completion、timeout、start failure が notification delivery を解放する integration test を追加する

## 6. Integration Verification

- [x] 6.1 two-node TCP test で shutdown 前に flush ack を待ってから run task が終了することを確認する
- [x] 6.2 two-node TCP test で DeathWatch notification が flush completion 後に届くことを確認する
- [x] 6.3 two-node TCP test で flush timeout 後も DeathWatch notification と shutdown が止まらないことを確認する
- [x] 6.4 `cargo test -p fraktor-remote-core-rs` を実行する
- [x] 6.5 `cargo test -p fraktor-remote-adaptor-std-rs` を実行する
- [x] 6.6 `cargo build -p fraktor-remote-core-rs --no-default-features` を実行する
- [x] 6.7 実装完了時に `docs/gap-analysis/remote-gap-analysis.md` の flush lifecycle gap を更新する
