## 1. Runtime 境界の固定

- [x] 1.1 `ActorSystemConfig` と `ActorSystemSetup` に runtime lock provider の設定面を追加し、default config が builtin spin provider を seed する
- [x] 1.2 actor runtime 用 provider 契約、opaque cell、`MailboxLockSet` を定義し、`RuntimeMutex<T, D>` のような public generic を導入しない方針をコード構造へ反映する
- [x] 1.3 第 1 段階の対象を mutex 系 hot path に限定し、非 hot path の `RuntimeMutex` / `RuntimeRwLock` caller を移行対象から外す

## 2. Dispatcher / Bootstrap Wiring

- [x] 2.1 `DefaultDispatcherConfigurator` / `BalancingDispatcherConfigurator` / `PinnedDispatcherConfigurator` が runtime lock provider を束縛するように更新する
- [x] 2.2 `SystemState::build_from_config`、`ActorSystem::bootstrap`、`ActorCell` wiring が provider から `MessageDispatcherShared` / `ExecutorShared` / `ActorRefSenderShared` を materialize するように切り替える
- [x] 2.3 `Dispatchers::resolve` と public dispatcher API が nongeneric のままであることを確認する

## 3. Hot Path 移行

- [x] 3.1 `MessageDispatcherShared` を provider ベースの opaque lock surface へ移行する
- [x] 3.2 `ExecutorShared` と `ActorRefSenderShared` を同じ provider family で構築できるようにする
- [x] 3.3 `Mailbox::new` / `Mailbox::new_sharing` が `MailboxLockSet` を受け取り、既存 run / enqueue / cleanup semantics を維持したまま hot path lock を provider ベースへ移行する
- [x] 3.4 `mailbox-runnable-drain` capability の MODIFIED Requirements と constructor signature 変更が実装と一致することを確認する
- [x] 3.5 `DispatcherSender` / `ActorCell` の必要最小限の追従だけで hot path を閉じる

## 4. Std Helper と検証

- [x] 4.1 `utils-adaptor-std` に debug provider helper と std provider helper を追加する
- [x] 4.2 debug provider を使った same-thread 再入 tell の panic 観測 test を追加する
- [x] 4.3 default spin fallback と system-scoped override の構成 test を追加する
- [x] 4.4 同一プロセス内の 2 actor system が独立した provider family を使えることを確認する
- [x] 4.5 `openspec validate new-lock-driver-port-adapter --strict` を通す
