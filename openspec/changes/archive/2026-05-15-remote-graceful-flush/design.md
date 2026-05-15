## Context

remote delivery は serialized payload、remote DeathWatch、system priority envelope の ACK/NACK redelivery まで接続済みである。一方で、remote association を閉じる直前に、既に transport writer lane へ渡された user message が相手側の remote inbound delivery 境界を越えたことを確認する flush protocol はまだない。`RemoteConfig::shutdown_flush_timeout` は存在するが、shutdown path から参照されておらず、`RemotingExtensionInstaller::shutdown_and_join` は現在の契約上、`RemoteShared::shutdown` で transport を停止してから wake / join へ進む。

Pekko Artery は `FlushOnShutdown` と `FlushBeforeDeathWatchNotification` でこの問題を扱う。fraktor-rs では Pekko の actor 実装や byte wire 互換を移植せず、fraktor-native `ControlPdu`、`Association` の純粋状態機械、std adaptor の timer / TCP lane driver に分解して実装する。

本 change は `remote-reliable-deathwatch` を前提にする。remote `Watch` / `Unwatch` / `DeathWatchNotification` が system priority envelope として送れること、ACK/NACK redelivery state が `remote-core` association にあることを利用する。

## Goals / Non-Goals

**Goals:**

- shutdown 時に active association へ flush request を送り、flush ack または `shutdown_flush_timeout` を待ってから transport を停止する。
- remote-bound `DeathWatchNotification` は、remote watch hook が直接 outbound enqueue せず、対象 association の flush 成功または timeout 後に enqueue する。
- flush request / ack は actor message や serialized payload ではなく wire-level control PDU として扱う。
- flush session の状態、flush id、caller が渡した対象 writer lane id、期待 ack 数、ack 済み lane、timeout 判定は `remote-core` の `Association` が所有する。
- timer、TCP writer lane topology の解釈、`JoinHandle` 待機、actor-core delivery、pending notification 保持は std adaptor に置く。
- timeout や flush 送信失敗は観測可能にしつつ、shutdown と DeathWatch notification を永久に止めない。

**Non-Goals:**

- Pekko Artery TCP framing / protobuf control PDU との byte compatibility。
- compression table、remote deployment daemon、`AddressTerminated` integration。
- user message の application-level ack、または actor user handler が処理完了したことの保証。
- `RemoteShared::shutdown` に async wait や adapter-owned sender / join handle を持たせること。
- flush timeout を無限待機や retry policy に拡張すること。

## Decisions

### Decision 1: flush は `ControlPdu` として表現する

`FlushRequest` と `FlushAck` を `ControlPdu` の subkind として追加する。flush は user actor に配送される message ではなく、transport lane の drain 境界を示す control signal であるため、`EnvelopePdu` や actor-core serialization registry には載せない。

理由:

- inbound flush request は actor-core mailbox へ進めず、現行の heartbeat response と同じ core control-PDU handling で flush ack に変換するだけでよい。
- flush ack は sender 側の association state に戻す transport-level signal であり、actor identity や serializer metadata を必要としない。
- `ControlPdu::Shutdown` と同じ wire-level lifecycle 領域に置く方が責務境界が明確になる。

代替案:

- system priority envelope として `Flush` message を送る案は、actor-core delivery と serialization に flush を漏らすため採用しない。
- `AckPdu` を flush ack に流用する案は、system envelope redelivery sequence と lane flush boundary が別概念になるため採用しない。

### Decision 2: flush session state は `Association` が所有する

flush id の採番、flush scope、caller が渡す対象 writer lane id、期待 ack 数、ack 済み lane、deadline、completed / timed out effect は `remote-core` の `Association` に置く。std adaptor / `Remote` は transport topology から lane set を決め、core が返す effect に従って control frame を送信し、timer event と inbound ack を core に戻す。

理由:

- flush は transport 種別に依存しない association lifecycle state である。
- `remote-core` に置くことで no_std unit test で ack / timeout / duplicate ack を検証できる。
- std task が flush map を独自に持つと、shutdown と DeathWatch 前 flush で同じ状態管理が二重化する。

代替案:

- std driver だけで flush wait map を持つ案は、future adaptor ごとに同じ protocol state を再実装することになるため採用しない。

### Decision 3: writer lane と control path を混同しない

flush request は `FlushScope::Shutdown` と `FlushScope::BeforeDeathWatchNotification` を持つ。ただし scope は `lane_id = 0` を control-only とみなす意味を持たない。現行 TCP adaptor の outbound writer lane は `RemoteConfig::outbound_lanes()` に従う message-capable lane であり、通常 control PDU は便宜上 lane 0 に enqueue されるが、lane 0 も envelope を運び得る。

そのため `BeforeDeathWatchNotification` flush は、notification より前に送られた user envelope を取り逃がさないよう、現行 TCP adaptor では **すべての message-capable writer lane** を対象にする。将来 dedicated control-only lane を持つ transport を追加する場合、その dedicated control-only lane は DeathWatch 前 flush の待機対象から外してよい。shutdown flush は message-capable writer lane と dedicated control-only lane の両方を対象にできる。

理由:

- 現行 TCP adaptor では `TcpClient::send` が lane 0、`send_with_lane_key` が hashed writer lane を使うため、lane 0 を除外すると lane 0 に hash された user frame を flush できない。
- Pekko の `excludeControlQueue` は dedicated control queue の話であり、fraktor の現行 TCP writer lane 番号へそのまま写すと意味がずれる。
- shutdown と DeathWatch 前 flush の違いは scope / 後続処理 / dedicated control-only lane の扱いで表現し、message-capable lane の取りこぼしは許容しない。

代替案:

- `lane_id = 0` を常に control lane とみなして DeathWatch 前 flush から外す案は、現行 TCP adaptor で user frame が lane 0 に載るため採用しない。
- dedicated control-only lane を今すぐ新設する案は、flush のためだけに TCP writer topology を大きく変えるため採用しない。

### Decision 4: flush request は association outbound queue を drain してから writer lane へ置く

flush success が意味を持つのは、flush 開始より前に core association queue にあった envelope が transport writer lane へ渡された後である。`Remote` は flush session を開始する前に対象 association の outbound queue を drain し、backpressure 等で drain できない場合は flush start failure または timeout outcome として扱う。

理由:

- TCP writer lane 上の flush marker は、その writer lane に既に enqueue 済みの frame の後ろに置かれるだけで、core association queue にまだ残っている envelope までは保証できない。
- remote-bound `DeathWatchNotification` は system priority envelope なので、flush 前に enqueue すると既存 user queue を追い越す可能性がある。
- flush start failure / timeout 後も notification と shutdown は進めるが、その場合は ordering guarantee ではなく bounded wait の失敗として観測可能にする。

代替案:

- flush marker を actor system message として association queue に入れる案は、flush を actor-core serialization / delivery に漏らすため採用しない。
- queue が残っていても flush marker を送る案は、flush completed が ordering guarantee として誤読されるため採用しない。

### Decision 5: `shutdown_and_join` は flush を先に待ち、その後に `RemoteShared::shutdown` する

installer の `shutdown_and_join(&self)` は active association の shutdown flush を開始し、完了または timeout を待ってから `RemoteShared::shutdown` を呼ぶ。`RemoteShared::shutdown` は引き続き同期の薄い delegate で、wake や join を持たない。

理由:

- `RemoteShared::shutdown` が先に transport を止めると flush request を送れない。
- actor system に登録された installer は `&self` API のまま使う必要があるため、adapter-owned sender / run handle の管理は installer 側に残す。
- flush wait が timeout した場合でも停止処理は進める必要がある。

代替案:

- `RemoteShared::shutdown` に flush wait を組み込む案は、core shared wrapper に std の sender / async wait / join handle を持ち込むため採用しない。

### Decision 6: remote-bound DeathWatch notification は std flush gate で保留する

remote watch hook は remote-bound `DeathWatchNotification` を即 enqueue せず、対象 association の `BeforeDeathWatchNotification` flush を std flush gate に要求する。flush gate は notification envelope を pending map に保持し、flush completed / timed out / failed event を受けたら pending envelope を system priority envelope として enqueue する。

理由:

- 現行の remote-bound `DeathWatchNotification` は `StdRemoteWatchHook::handle_deathwatch_notification` から発生し、`WatcherState` の failure detector 経路では発生しない。
- `WatcherState` は local actor が remote target を watch する no_std 状態機械として維持し、逆方向の remote-bound notification pending map を持たせない。
- pending envelope の保持、timeout wait、event sender との接続は std adaptor の orchestration 責務である。
- flush timeout 後も DeathWatch notification を送ることで、remote actor 終了通知が永久に止まらない。

代替案:

- `WatcherState` が pending `OutboundEnvelope` を保持する案は、watcher state に逆方向の remote-bound notification と transport envelope の寿命管理を持ち込みすぎるため採用しない。
- flush 失敗時に notification を破棄する案は、DeathWatch の到達性を壊すため採用しない。

## Risks / Trade-offs

- [Risk] flush ack が欠落すると shutdown / notification が止まる → timeout を必須にし、timeout 後は log / observable error を残して先へ進む。
- [Risk] duplicate `FlushAck` で remaining count が過剰に減る → flush id と lane id の組で ack 済み lane を tracking し、duplicate ack は no-op にする。
- [Risk] lane 0 を control lane と誤解して user frame を取り逃がす → 現行 TCP adaptor では message-capable writer lane 全体を対象にし、dedicated control-only lane だけを別扱いする。
- [Risk] flush marker が association queue に残った user envelope を追い越す → flush 開始前に association outbound queue を drain し、drain できない場合は failure / timeout outcome として扱う。
- [Risk] shutdown flush 中に connection lost が起きる → association は flush を failed / timed out として完了扱いにし、shutdown path は継続する。
- [Risk] DeathWatch notification の latency が増える → flush timeout は `RemoteConfig::shutdown_flush_timeout` から上限を取り、無制限には待たない。
- [Risk] active `remote-reliable-deathwatch` と同じ spec 領域を触る → 本 change は DeathWatch の到達性ではなく、notification 前の ordering flush だけを扱う。
