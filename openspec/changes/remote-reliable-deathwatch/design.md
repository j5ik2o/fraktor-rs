## Context

remote delivery は provider dispatch、TCP transport、serialization-backed envelope delivery まで接続済みである。一方で remote DeathWatch はまだ利用者可視の機能になっていない。`StdRemoteActorRefProvider::watch` / `unwatch` は remote path を validation して core provider へ委譲するだけで、`WatcherState` へ command を送らない。`Remote::handle_inbound_ack_pdu` は `AckPdu` を log するだけで、再送 window を進めない。

actor-core 側には `RemoteWatchHook` があり、local cell が存在しない target への `SystemMessage::Watch` / `Unwatch` を remote provider が消費できる。ただし hook は現状 pid だけを受け取るため、remote wire にそのまま流すには情報が足りない。Pid は node local な値であり、受信側 actor system の pid と一致する保証がない。remote DeathWatch では actor path を wire 上の安定識別子として使い、受信側で actor-core の pid へ再解決する必要がある。

## Goals / Non-Goals

**Goals:**

- remote actor を `watch` した local actor が、remote actor 終了時に actor-core の通常 DeathWatch 経路で通知を受ける。
- remote `Watch` / `Unwatch` / `DeathWatchNotification` を system priority envelope として扱い、ACK/NACK resend で一時的な欠落から回復する。
- `WatcherState` は no_std の純粋状態機械のまま維持し、timer、task、actor-core delivery は std adaptor に置く。
- wire 上では pid 値ではなく actor path metadata を remote identity の真実源にする。
- duplicate delivery は actor-core の既存 DeathWatch dedup と remote redelivery state の sequence dedup で抑止する。

**Non-Goals:**

- DeathWatch notification 前の flush。
- remote deployment daemon。
- compression table / manifest compression。
- Pekko byte wire 互換。
- classic remoting の `AckedDelivery` 互換。

## Decisions

### Decision 1: remote DeathWatch の wire identity は actor path を使う

remote `Watch` / `Unwatch` / `DeathWatchNotification` は `EnvelopePdu` の `recipient_path` と `sender_path`、または同等の actor path metadata を使って target / watcher を表す。`SystemMessage` payload 内の `Pid` は受信側で actor-core に渡す直前に、受信側 actor system の pid へ解決または materialize する。

理由:

- `Pid` は node local な識別子であり、別 node に送っても同じ actor を指す保証がない。
- 既存 `EnvelopePdu` は recipient / sender の actor path を既に持つため、新しい path-only PDU を増やさずに actor identity を運べる。
- actor-core の `SystemMessage` と actor cell の DeathWatch 処理は pid ベースのまま維持できる。

代替案:

- `SystemMessage` serializer に actor path を追加する案は、local-only system message まで remote 都合を背負わせるため採用しない。
- remote pid を global にする案は、既存 pid allocator と synthetic remote pid の責務を壊すため採用しない。

### Decision 2: ACK/NACK redelivery state は core association が所有する

system priority envelope の sequence assignment、送信 window、受信 cumulative ACK / NACK bitmap、duplicate detection は `remote-core` の association state に置く。std 側は timer と transport send を担当し、core が返す resend effect を実行する。

理由:

- sequence / ACK / NACK は transport 種別に依存しない remote protocol state である。
- `remote-core` に置くことで no_std unit test で window と duplicate handling を検証できる。
- std 側 task が sequence state を持つと、association recovery と redelivery の責務が分散する。

代替案:

- std retry driver だけに window を持たせる案は、future adaptor ごとに再実装が必要になり、core の `AckPdu` が実質的に利用されないため採用しない。

### Decision 3: std provider は remote pid/path registry と hook wiring を持つ

`StdRemoteActorRefProvider` は remote actor ref materialization 時に synthetic remote pid と canonical actor path の対応を保持する。installer は actor-core に remote watch hook を登録し、hook は target pid / watcher pid を path へ解決して watcher task へ渡す。path が解決できない場合は hook は消費せず、actor-core の既存 fallback に任せる。

理由:

- hook 入力が pid である現状でも、provider が発行した remote pid なら path へ戻せる。
- local watch は引き続き actor-core の通常経路に残せる。
- provider dispatch が remote actor ref の唯一の materialization 点なので、pid/path registry の所有者として自然である。

代替案:

- actor-core に remote provider 参照を直接持たせる案は no_std core と std adaptor の境界を広げるため採用しない。

### Decision 4: watcher task は effect 適用だけを担当する

std watcher task は `WatcherState::handle` に command を渡し、返った effect を次の外部操作へ変換する。

- `SendHeartbeat` は `ControlPdu::Heartbeat` / `HeartbeatResponse` の送受信へ変換する。
- `SendWatch` / `SendUnwatch` / `RewatchRemoteTargets` 相当の effect は system priority envelope へ変換する。
- `NotifyTerminated` は local watcher へ `SystemMessage::DeathWatchNotification` を送る。
- `NotifyQuarantined` は actor-core event stream または明示 error path へ流す。

理由:

- core watcher は `&mut self` の純粋状態機械として維持できる。
- std 側が timer、transport、actor-core state の橋渡しをまとめて担うことで、remote-core に std 依存を入れずに済む。

代替案:

- watcher を actor-core actor として実装する案は、remote-core の状態機械と actor-core scheduling を強く結合するため採用しない。必要なら std task が actor-core API を呼ぶ薄い境界に留める。

## Risks / Trade-offs

- [Risk] pid/path registry が stale になると remote watch hook が誤って消費する → provider は remote actor ref cache eviction と同じ単位で registry を更新し、解決不能時は hook を消費しない。
- [Risk] resend により `DeathWatchNotification` が重複する → inbound redelivery state で sequence duplicate を捨て、actor-core の既存 dedup でも二重通知を抑止する。
- [Risk] `Unwatch` 欠落後に古い notification が届く → `Unwatch` も system priority envelope として ACK 対象にし、actor-core 側の watching entry が消えている通知は既存 dedup で無視する。
- [Risk] heartbeat loss と watch system message loss の原因が混ざる → Watch/Unwatch/DeathWatchNotification の redelivery は ACK/NACK、node liveness は `WatcherState` + failure detector に分けて test する。
- [Risk] DeathWatch 前 flush がないため、remote actor の最後の user message と通知順序が厳密化されない → 本 change では通知の到達性を扱い、flush ordering は `remote-graceful-flush` に残す。
