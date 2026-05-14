## Why

remote DeathWatch と ACK/NACK redelivery が接続されたことで、remote node 間の終了通知は届くようになったが、終了直前に送出済みの user message が remote inbound delivery 境界を越える前に DeathWatch notification が送られる可能性はまだ残っている。`shutdown_flush_timeout` 設定も現状は待機 driver に接続されておらず、shutdown 時に remote writer lane の flush ack / timeout を観測できない。

## What Changes

- fraktor-native control PDU に flush request / flush ack を追加し、shutdown flush と DeathWatch notification 前 flush で共通利用する。
- `remote-core` association に flush session state を追加し、flush request id、対象 remote、caller が渡す対象 writer lane id、期待 ack 数、期限、完了 / timeout effect を管理する。
- std TCP adaptor / core control handling は flush control frame を対象 writer lane に送信し、inbound flush request へ flush ack を返す。
- `RemotingExtensionInstaller::shutdown_and_join` は transport shutdown 前に active association の flush ack または `shutdown_flush_timeout` を待つ。
- remote watch hook / std flush gate は remote-bound `DeathWatchNotification` を enqueue する前に対象 association の flush を要求し、flush 成功または timeout 後に notification を送る。
- flush の失敗や timeout は shutdown / notification を永久に止めず、log または test-observable error path に残して先へ進める。
- Pekko Artery byte wire compatibility は狙わず、fraktor-native control PDU 上で責務とセマンティクスを揃える。
- **BREAKING**: fraktor-native `ControlPdu` layout に flush request / ack subkind を追加する。pre-release の既存 remote wire 互換は保持しない。

## Capabilities

### New Capabilities

なし。

### Modified Capabilities

- `remote-core-wire-format`: control PDU が flush request / flush ack を運べるようにする。
- `remote-core-association-state-machine`: association が flush session state と ack / timeout effect を所有する。
- `remote-core-extension`: `Remote` / `RemoteShared` が flush 開始、inbound ack、timer input、flush outcome を扱えるようにする。
- `remote-core-transport-port`: lane-targeted flush request delivery を `RemoteTransport` 境界に追加する。
- `remote-core-settings`: `shutdown_flush_timeout` を shutdown / DeathWatch flush deadline の source of truth として明確化する。
- `remote-adaptor-std-tcp-transport`: TCP lane が flush control frame を送受信し、inbound flush request へ ack を返す。
- `remote-adaptor-std-provider-dispatch`: remote watch hook が remote-bound notification を直接 enqueue せず、flush gate へ渡す。
- `remote-adaptor-std-io-worker`: shutdown flush driver と DeathWatch 前 flush driver を core state / transport / actor-core delivery に接続する。

## Impact

- `modules/remote-core/src/wire/` の `ControlPdu` / codec。
- `modules/remote-core/src/association/` の flush session state、effect、timeout handling。
- `modules/remote-core/src/extension/` の flush start / ack / timer input と outcome surface。
- `modules/remote-core/src/transport/remote_transport.rs` の lane-targeted flush request command。
- `modules/remote-core/src/config/remote_config.rs` の `shutdown_flush_timeout` 利用境界。
- `modules/remote-adaptor-std/src/transport/tcp/` の control frame lane dispatch と flush ack handling。
- `modules/remote-adaptor-std/src/provider/remote_watch_hook.rs` の remote-bound notification flush gate。
- `modules/remote-adaptor-std/src/extension_installer/` と std flush gate の shutdown / notification orchestration。
- `modules/remote-adaptor-std` の two-node integration tests。
