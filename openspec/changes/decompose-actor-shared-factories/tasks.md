## 1. Port 契約の分割

- [x] 1.1 `ActorSharedFactory` を `ExecutorSharedFactory`、`MessageDispatcherSharedFactory`、`SharedMessageQueueFactory` などの個別 trait に分割する
- [x] 1.2 各 trait のメソッド名を `create` に統一する
- [x] 1.3 builtin / debug / std 実装に必要な個別 trait 実装を追加する

## 2. 利用側 wiring の差し替え

- [x] 2.1 dispatcher configurator / executor factory / balancing shared queue を個別 Port ベースへ移行する
- [x] 2.2 `ActorRef` / ask / sender 経路を `ActorRefSenderSharedFactory` ベースへ移行する
- [x] 2.3 event stream / subscriber helper を `EventStreamSharedFactory` / `EventStreamSubscriberSharedFactory` ベースへ移行する
- [x] 2.4 `ActorCell::create` の runtime-owned state 構築を actor-cell 系 Port 群へ移行する

## 3. 廃止と検証

- [x] 3.1 旧 `ActorSharedFactory` 依存と関連 naming を削除する
- [x] 3.2 test double を個別 Port 単位へ整理し、既存テストを更新する
- [x] 3.3 `./scripts/ci-check.sh ai all` を実行し、Port 分割の影響を検証する
