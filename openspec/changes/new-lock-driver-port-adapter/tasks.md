## 1. Lock Provider 境界の固定

- [ ] 1.1 `ActorSystemConfig` と `ActorSystemSetup` に `with_lock_provider(...)` を追加し、default config が `BuiltinSpinLockProvider` を seed する
- [ ] 1.2 `ActorLockProvider` trait と `MailboxSharedSet` を `kernel/system/lock_provider/` 配下に定義し、`MessageDispatcherShared` / `ExecutorShared` / `ActorRefSenderShared` を直接 materialize する factory メソッドを生やす。`*LockCell` のような中間 newtype は導入せず、`*Shared` 内部の lock backend は consumer module 側の private trait (`DispatcherLocked` 等) に閉じる。`RuntimeMutex<T, D>` のような public generic は導入しない
- [ ] 1.3 第 1 段階の対象を mutex 系 hot path に限定し、非 hot path の `RuntimeMutex` / `RuntimeRwLock` caller を移行対象から外す

## 2. Dispatcher / Bootstrap Wiring

- [ ] 2.1 `DefaultDispatcherConfigurator` / `BalancingDispatcherConfigurator` / `PinnedDispatcherConfigurator` が `ActorLockProvider` を束縛するように更新する
- [ ] 2.2 `SystemState::build_from_config`、`ActorSystem::bootstrap`、`ActorCell` wiring が provider から `MessageDispatcherShared` / `ExecutorShared` / `ActorRefSenderShared` を materialize するように切り替える
- [ ] 2.3 `Dispatchers::resolve` と public dispatcher API が nongeneric のままであることを確認する

## 3. Hot Path 移行

- [ ] 3.1 `MessageDispatcherShared` を provider ベースの opaque lock surface へ移行する
- [ ] 3.2 `ExecutorShared` と `ActorRefSenderShared` を同じ provider family で構築できるようにする
- [ ] 3.3 `Mailbox::new` / `Mailbox::new_sharing` が `MailboxSharedSet` を受け取り、既存 run / enqueue / cleanup semantics を維持したまま hot path lock を provider ベースへ移行する
- [ ] 3.4 `mailbox-runnable-drain` capability の MODIFIED Requirements と constructor signature 変更が実装と一致することを確認する
- [ ] 3.5 `DispatcherSender` / `ActorCell` の必要最小限の追従だけで hot path を閉じる

## 4. Actor Std Helper と検証

- [ ] 4.1 `actor-adaptor-std` に debug provider helper と std provider helper を追加する
- [ ] 4.2 debug provider を使った same-thread 再入 tell の panic 観測 test を追加する
- [ ] 4.3 default spin fallback と system-scoped override の構成 test を追加する
- [ ] 4.4 同一プロセス内の 2 actor system が独立した provider family を使えることを確認する
- [ ] 4.5 `openspec validate new-lock-driver-port-adapter --strict` を通す
