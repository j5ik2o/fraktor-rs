## Context

`remote` は `remote-redesign` 後に core と std adaptor の責務分離が進み、`RemoteConfig` には handshake、ack-based redelivery、queue、lane、frame size、outbound restart などの Artery advanced settings の一部が型付き設定として入っている。一方で、Pekko Artery が持つ large-message、inbound restart、compression の設定 surface はまだ未定義であり、後続の remote delivery / compression 実装時に設定項目が場当たり的に増えるリスクがある。

この change は Pekko Artery との byte-compatible wire protocol を目指さない。現行の `core/wire` は Pekko の責務分割を参考にした fraktor-rs 独自 binary format として維持し、ここでは responsibility parity に必要な typed settings と std runtime の参照境界だけを定義する。

## Goals / Non-Goals

**Goals:**

- `RemoteConfig` が large-message destinations、outbound large-message queue size、inbound restart budget、compression settings を型付き設定として保持できる。
- large-message destinations は core 側で所有可能な no_std データとして表現し、std adaptor が宛先判定に使える契約にする。
- inbound restart budget は既存の outbound restart と対になる設定として定義し、std runtime が無限 restart にならないよう参照する。
- compression settings は後続 Phase3 の入力となる設定 surface に限定し、wire-level compression protocol の実装とは分離する。
- core は引き続き `std` に依存せず、`alloc` と `core` の範囲に閉じる。

**Non-Goals:**

- Pekko Artery TCP framing の byte compatibility。
- `AKKA` magic、stream id、Pekko protobuf control PDU、compression table の wire 表現。
- `ArteryMessageSerializer`、`DaemonMsgCreateSerializer`、payload serializer manifest の Pekko 互換実装。
- concrete remote `ActorRef` construction。
- `ActorIdentity` remote ActorRef restoration。
- `RemoteRouterConfig` runtime routee materialization。
- HOCON parser または JVM 設定モデルの導入。

## Decisions

1. **wire compatibility ではなく responsibility parity に固定する。**
   Pekko ノードとの相互運用を今回の目的にすると `tcp_transport/frame_codec.rs`、`core/wire/*`、serializer、compression protocol まで同時に再設計が必要になる。これは `advanced Artery settings` の設定 surface 追加より大きい change になるため、現行の fraktor-rs 独自 wire format は維持する。

2. **large-message は transport 実装ではなく設定 surface と宛先分類に限定する。**
   `RemoteConfig` は outbound large-message queue size と large-message destinations を保持する。actual large-message transport、payload fragmentation、compression、専用 lane の配送挙動は別 change に分離する。これにより、設定だけ先に型で固定しつつ、未実装の配送機能を暗黙に有効化しない。

3. **large-message destinations は no_std core の owned value object にする。**
   HOCON や regex engine に依存せず、`alloc` で所有できる型として表現する。runtime の判定に必要な最小契約だけを core に置き、TCP や actor system への接続は std adaptor 側に残す。

4. **inbound restart budget は既存 `RestartCounter` と同じ deadline-window 意味論に寄せる。**
   outbound loop には `RestartCounter` と `ReconnectBackoffPolicy` が存在する。inbound 側も `inbound_restart_timeout` と `inbound_max_restarts` を `RemoteConfig` から受け取り、同一の deadline-window budget として扱う。時刻入力は monotonic millis とし、wall clock には依存しない。

5. **compression settings は設定 surface のみを追加する。**
   compression table、advertisement、protobuf control PDU、wire encoding は Phase3 の protocol change で扱う。この change では std adaptor が設定を参照できる状態に留め、wire-level 圧縮を行わないことを spec に明示する。

## Risks / Trade-offs

- **Risk: compression settings が実装済み機能に見える。** → accessor と spec で wire-level compression は非対象と明示し、テストも設定保持に限定する。
- **Risk: large-message destinations の表現を Pekko HOCON に寄せすぎる。** → Rust の typed value object として定義し、HOCON parser や regex dependency は導入しない。
- **Risk: inbound restart budget の追加で runtime 責務が広がる。** → 既存 outbound restart と同じ `RestartCounter` パターンを再利用し、core state machine には async / tokio を入れない。
- **Risk: 既存 spec に `RemoteSettings` 名が残っている。** → この change では新規 requirement で現行 `RemoteConfig` 境界を明示する。旧 archived spec の名前整理は別途、必要な範囲で扱う。
