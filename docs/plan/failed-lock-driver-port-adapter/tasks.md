## 1. utils-core に lock-driver seam を導入する

- [x] 1.1 `modules/utils-core/src/core/sync/` に `LockDriver` / `RwLockDriver` trait を追加する
- [x] 1.2 `LockDriver` / `RwLockDriver` の最小形を GAT ベースの static-dispatch 契約として定義する
- [x] 1.3 factory seam (`LockDriverFactory` / `RwLockDriverFactory`) を追加する
- [x] 1.4 no_std builtin driver として `SpinSyncMutex` / `SpinSyncRwLock` を新 trait に乗せる
- [x] 1.5 `RuntimeMutex` / `RuntimeRwLock` を alias から driver 差し替え可能な port surface へ昇格する
- [x] 1.6 `RuntimeMutex<T>` / `RuntimeRwLock<T>` は default-driver surface として維持し、既存 caller が一括書き換え不要であることを確認する
- [x] 1.7 `NoStdMutex<T>` が `RuntimeMutex<T>` の変更に追従することを確認する
- [x] 1.8 `RuntimeMutex` / `RuntimeRwLock` の port 化を workspace-wide な定義変更として適用し、旧 alias-to-SpinSync 定義だけを置き換える

## 2. utils-adaptor-std に std adapter driver を追加する

- [x] 2.1 `DebugSpinSyncMutex` / `DebugSpinSyncRwLock` を再導入する
- [x] 2.2 `StdSyncMutex` / `StdSyncRwLock` を追加する
- [x] 2.3 debug/std driver に対応する factory を追加する
- [x] 2.4 poison を driver 実装側で吸収し、caller 側 contract に露出させない
- [x] 2.5 `test-support` 等の feature 境界を整理し、debug driver を明示的に選択できるようにする

## 3. actor-core Phase V-A hot path を genericize する

- [ ] 3.1 `ActorRefSenderShared` を lock driver factory で差し替え可能にする
- [ ] 3.2 `DispatcherSender` が上記 genericization に追従し、2 phase send の再入安全性を維持する
- [ ] 3.3 `MessageDispatcherShared` を lock driver factory で差し替え可能にする
- [ ] 3.4 `ExecutorShared` を lock driver factory で差し替え可能にする
- [ ] 3.5 `Mailbox` を lock driver factory で差し替え可能にする
- [ ] 3.6 `ActorCell` が上記 genericization に追従し、mailbox / sender / dispatcher の wiring を維持する
- [ ] 3.7 public API (`ActorSystem`, typed system, `ActorRef`) に driver generic parameter を漏らさないことを確認する
- [ ] 3.8 driver family を bootstrap / configurator 境界で 1 つ固定し、public surface を nongeneric のまま維持する
- [ ] 3.9 上記 hot path で inline executor / re-entrant tell 経路の既存意味論を維持する

## 4. deadlock 検知の価値を actor-core 単独で成立させる

- [ ] 4.1 actor-core test target で `DebugSpinSyncMutex` を hot path に差し込める configuration を追加する
- [ ] 4.2 actor-core test target で same-thread 再入 tell が検知される regression test か example を追加する
- [ ] 4.3 `StdSyncMutex` を std driver 候補として選択できる test configuration を追加する
- [ ] 4.4 `SpinSyncMutex` / `DebugSpinSyncMutex` / `StdSyncMutex` の 3 種が caller から factory 経由で選べることを確認する

## 5. Phase V-B/C は後続 work として切り離す

- [ ] 5.1 `SharedMessageQueue` を含む Phase V-B 対象をこの change に含めないことを確認する
- [ ] 5.2 actor-core secondary wrapper (`EventStreamShared`, `ActorRefProviderShared`, `RemoteWatchHookShared`, `SerializationExtensionShared`, `SchedulerShared`) は follow-up として明記する
- [ ] 5.3 cluster / persistence / stream への caller migration はこの change に含めないことを確認する

## 6. OpenSpec / docs / follow-up 整理

- [ ] 6.1 `utils-lock-driver-port` capability の spec delta を追加する
- [ ] 6.2 actor-core hot path に限定した success criteria が proposal / design / tasks で整合していることを確認する
- [ ] 6.3 `openspec validate lock-driver-port-adapter --strict` を通す
