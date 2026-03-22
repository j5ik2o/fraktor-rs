# 戻り値握りつぶし監査レポート

更新日: 2026-03-22
対象: 本番コードのみ（テストコード除外）

## 概要

戻り値を捨てている箇所を静的探索し、`let _ = ...` と `.ok()` を中心に確認した。
加えて `clippy::let_underscore_must_use` を使い、`#[must_use]` な戻り値の握りつぶしも確認した。

結論として、重大な欠陥につながりうる箇所が複数ある。
特に以下の系統は危険度が高い。

- リモート通信やクラスタ通信での配送失敗の黙殺
- 永続化メッセージ送信失敗の黙殺
- stop / watch / terminated など制御メッセージ失敗の黙殺

## 重大な指摘

### 1. リモート受信フレームが飽和時に無言で破棄される

対象:
- [modules/remote/src/std/endpoint_transport_bridge/bridge.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/remote/src/std/endpoint_transport_bridge/bridge.rs):914

内容:
- `self.frame_sender.try_send(frame)` の戻り値を捨てている
- `Full` / `Closed` のどちらも検知しない

リスク:
- 受信フレームが静かに失われる
- ハンドシェイク、ACK、通常メッセージ、system message の欠落原因が追えない
- 負荷時にだけ発生するため、再現と調査が難しい

### 2. gossip の入出力エラーが無言で消える

対象:
- [modules/cluster/src/std/tokio_gossip_transport.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/cluster/src/std/tokio_gossip_transport.rs):71
- [modules/cluster/src/std/tokio_gossip_transport.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/cluster/src/std/tokio_gossip_transport.rs):79

内容:
- 受信した delta を `inbound_tx.try_send(...)` でキュー投入する際の失敗を握りつぶしている
- UDP 送信 `send_to(...).await` の I/O エラーも握りつぶしている

リスク:
- membership delta がロストしても検知できない
- クラスタ収束遅延、分断、ノード観測の不整合につながる
- 通信失敗がメトリクスにもログにも出ない

### 3. 永続化メッセージ送信失敗が黙殺される

対象:
- [modules/persistence/src/core/persistent_actor.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/persistence/src/core/persistent_actor.rs):140
- [modules/persistence/src/core/persistent_actor.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/persistence/src/core/persistent_actor.rs):147
- [modules/persistence/src/core/persistent_actor.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/persistence/src/core/persistent_actor.rs):158
- [modules/persistence/src/core/persistent_actor.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/persistence/src/core/persistent_actor.rs):166
- 契約定義: [modules/persistence/src/core/persistence_context.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/persistence/src/core/persistence_context.rs):525
- 契約定義: [modules/persistence/src/core/persistence_context.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/persistence/src/core/persistence_context.rs):540

内容:
- `send_write_messages` / `send_snapshot_message` は `PersistenceError::MessagePassing` を返す契約
- しかし呼び出し側で `let _ = ...` として失敗を捨てている

リスク:
- snapshot 保存・削除、journal 削除要求が配送されなくても呼び出し元が失敗を認識できない
- 「削除したつもり」「保存したつもり」の状態が発生する
- 永続化系の不具合はデータ整合性に直接影響するため危険度が高い

### 4. stop / terminated / watch 系システムメッセージ失敗が黙殺される

対象:
- [modules/actor/src/core/actor/actor_cell.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/actor/actor_cell.rs):280
- [modules/actor/src/core/actor/actor_cell.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/actor/actor_cell.rs):307
- [modules/actor/src/core/actor/actor_cell.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/actor/actor_cell.rs):315
- [modules/actor/src/core/actor/actor_cell.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/actor/actor_cell.rs):586
- [modules/actor/src/core/actor/actor_cell.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/actor/actor_cell.rs):680
- [modules/actor/src/core/actor/actor_context.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/actor/actor_context.rs):239
- [modules/actor/src/core/actor/actor_context.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/actor/actor_context.rs):276

内容:
- child stop
- watcher への `Terminated`
- watch 設置失敗時の child 停止

これらの失敗が記録も伝播もされずに消える。

リスク:
- child が止まった前提で後続処理が進む
- `Terminated` 未達で death watch 契約が崩れる
- `spawn_child_watched` 失敗時の cleanup 失敗により孤児 child が残る

## 重要だが設計判断の余地がある箇所

### EventStream への通知失敗の黙殺

対象:
- [modules/actor/src/core/event/stream/actor_ref_subscriber.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/event/stream/actor_ref_subscriber.rs):44

内容:
- コメントでも「黙殺する」と明記されている

評価:
- 意図的設計の可能性はある
- ただし mailbox full / closed を完全に捨てるため、可観測性は落ちる

### Scheduler からの配送失敗の黙殺

対象:
- [modules/actor/src/core/scheduler/scheduler_core.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/scheduler/scheduler_core.rs):513

内容:
- `receiver.tell(...)` の失敗を捨てている

評価:
- receiver 停止済みなら無害とみなす設計はありえる
- ただし timer 起因の欠落が観測不能になる

### Receptionist の watch / listing 配送失敗の黙殺

対象:
- [modules/actor/src/core/typed/receptionist.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/typed/receptionist.rs):56
- [modules/actor/src/core/typed/receptionist.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/typed/receptionist.rs):75
- [modules/actor/src/core/typed/receptionist.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/typed/receptionist.rs):78
- [modules/actor/src/core/typed/receptionist.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/typed/receptionist.rs):98
- [modules/actor/src/core/typed/receptionist.rs](/Users/j5ik2o/Sources/j5ik2o.github.com/j5ik2o/fraktor-rs/modules/actor/src/core/typed/receptionist.rs):191

評価:
- discovery 通知なので core 制御系よりは一段軽い
- ただし購読登録や listing 初期配信失敗を無視しているため、利用者視点では「登録したのに見えない」が起こりうる

## 観測結果

- テストコードを除外しても、本番コード側に `let _ = ...` が相当数残っている
- `clippy::let_underscore_must_use` でも多数の警告が出る
- 特に actor / cluster / remote / persistence の制御面で集中している

## まとめ

本件は単なるスタイル上の問題ではない。
戻り値の握りつぶしにより、配送失敗、停止失敗、監視失敗、永続化送信失敗が無言で消える箇所がある。

最優先で精査すべき対象は次の 4 系統である。

1. remote bridge の `try_send`
2. gossip transport の `try_send` / `send_to`
3. persistent actor の `send_write_messages` / `send_snapshot_message`
4. actor core の stop / watch / terminated 系 `send_system_message`
