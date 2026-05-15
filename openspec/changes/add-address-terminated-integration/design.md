## Context

Pekko `RemoteWatcher` は heartbeat failure を検出したとき、対象 actor ごとの DeathWatch notification に加えて、remote address 全体の喪失を `AddressTerminated` として publish する。fraktor-rs では `WatcherState` が remote actor ごとの `NotifyTerminated` と node quarantine effect を返し、std watcher task が local watcher へ `DeathWatchNotification` を配送しているが、node-level failure を actor-core event stream へ publish する契約がない。

この gap により、remote node failure を DeathWatch とは別の lifecycle signal として観測できず、remote deployment daemon / watcher の cleanup も actor termination 通知に依存しやすくなる。core 側は no_std を維持し、std 側だけが timer、task、event publication、ActorSystem handle を扱う。

## Goals / Non-Goals

**Goals:**

- actor-core event stream に `AddressTerminated` 相当の event と classifier key を追加する。
- `WatcherState` が remote node unavailable を address-level effect として返し、std watcher task がその effect を publish する。
- actor-level `DeathWatchNotification` と node-level `AddressTerminated` の責務を分ける。
- remote deployment watcher / daemon が node termination を購読して remote-created child の cleanup や failure propagation に使える契約を定義する。
- `remote-core` と `actor-core-kernel` の no_std 境界を維持する。

**Non-Goals:**

- Pekko event stream の binary / class hierarchy 互換。
- cluster membership、split brain、reachability gossip、failure detector tuning の再設計。
- remote stop / suspend / resume protocol の追加。
- remote deployment daemon 自体の新規実装や registry 設計の変更。
- address termination event 専用の永続化、独自 replay semantics、外部 metrics export。既存 event stream の buffered replay contract からこの event だけを除外しない。

## Decisions

### Decision 1: `AddressTerminated` は actor-core event stream の first-class variant とする

actor-core に `AddressTerminatedEvent` または同等の concrete payload を追加し、`EventStreamEvent::AddressTerminated` と `ClassifierKey::AddressTerminated` で publish / subscribe できるようにする。`Extension` payload で代替すると型付き購読と subchannel filtering が弱くなり、Pekko の `AddressTerminatedTopic` 相当を明示できないため採用しない。

payload は actor-core が所有できる authority string、reason、monotonic millis timestamp を持つ最小 contract とする。actor-core から `remote-core::Address` へ依存すると crate 境界が逆転するため、std adaptor が `remote-core::Address` を authority string に変換して publish する。std 固有の `Instant` は入れず、core event として clone 可能な value object に留める。

### Decision 2: node-level effect は `remote-core` の watcher state から返す

`WatcherState` は heartbeat failure detector の判断を最も近くで持っているため、unavailable 判定時に actor-level `NotifyTerminated` と address-level termination effect を同じ state transition から返す。address-level effect は `HeartbeatTick` の `now` 由来の monotonic millis timestamp と reason metadata を持ち、std watcher task は effect を actor-core event payload へ変換して publish するだけにする。

std watcher task が独自に address termination を推測する案は、idempotency と heartbeat recovery marker が core state と分離して二重通知を起こしやすいため採用しない。

### Decision 3: actor DeathWatch と address termination は別 signal として同時に出す

remote node failure では、既存の local watcher 向け `DeathWatchNotification` は維持する。一方で `AddressTerminated` は node-level lifecycle signal として publish し、remote deployment watcher / daemon や診断 subscriber が利用する。`AddressTerminated` を `DeathWatchNotification` の代替にはしない。

この分離により、actor の watching 状態、dedup、unwatch suppression は actor-core DeathWatch に残し、node-level cleanup は event stream subscriber に委譲できる。

### Decision 4: publication は idempotent で heartbeat recovery 後だけ再発火できる

同じ remote node が unavailable と判定され続ける間、`AddressTerminated` は一度だけ publish される。heartbeat または heartbeat response を再受信した後に再び unavailable になった場合だけ、新しい termination event を publish できる。これは既存 `NotifyTerminated` の idempotency と揃える。

### Decision 5: remote deployment cleanup は subscriber として接続する

remote deployment watcher / daemon は `ClassifierKey::AddressTerminated` を購読し、該当 authority に紐づく pending deployment、remote-created child tracking、late response handling を cleanup する。watcher task から deployment module を直接呼ぶ案は、remote watcher と deployment の ownership を結合させるため採用しない。

既存 event stream の `subscribe_with_key` は buffered event を replay できるため、deployment 側は pending request の start timestamp と address termination event の monotonic millis timestamp を比較し、request より古い termination event で新しい pending request を失敗させてはならない。remote stack startup 中に subscriber を登録する場合でも、この timestamp guard を持つことで replay と restart の意味論を明確にする。

## Risks / Trade-offs

- `AddressTerminated` と `RemotingLifecycleEvent::Gated` / quarantine event の意味が重なる可能性がある -> `AddressTerminated` は DeathWatch / deployment cleanup 向けの node lost signal、lifecycle event は remoting diagnostics として使い分ける。
- event stream variant 追加で classifier key tests と archived spec の variant count が変わる -> delta spec で 15 variant contract へ更新し、tests も合わせる。
- heartbeat recovery 後の再通知が remote actor lifecycle と食い違う可能性がある -> `WatcherState` の既存 notified marker と同じ reset 条件を使う。
- buffered replay された古い address termination event が新規 deployment を誤って失敗させる可能性がある -> deployment request start timestamp より古い termination event は cleanup 対象から除外する。
- deployment daemon がまだ active change 上の機能に依存する可能性がある -> この change は address termination の購読 contract を定義し、daemon 内部の詳細は実装時に現行 codebase に合わせる。

## Migration Plan

1. actor-core event stream に address termination event type、variant、classifier key、tests を追加する。
2. `remote-core` watcher effect に address termination effect を追加し、failure detector unavailable 判定時の idempotent emission を tests で保証する。
3. std watcher task が effect を actor-core event stream へ publish する。
4. remote deployment watcher / daemon が `AddressTerminated` を購読し、authority-bound state cleanup と pending response failure propagation を行う。
5. two-node または targeted integration test で remote node failure から address termination publish と local DeathWatch notification が両方起きることを検証する。
6. 実装後に `docs/gap-analysis/remote-gap-analysis.md` を更新する。

rollback は active change を削除することで行う。pre-release のため compatibility alias や legacy event path は追加しない。

## Open Questions

初回 proposal 時点の未解決事項はない。event payload の最終フィールド名は既存 event stream payload の authority / reason 命名に合わせて実装時に確定する。
