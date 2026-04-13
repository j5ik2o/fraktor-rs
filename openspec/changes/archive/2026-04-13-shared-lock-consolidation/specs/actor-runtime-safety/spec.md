## MODIFIED Requirements

### Requirement: Mailbox 構築は policy と queue の不変条件を保持する

actor runtime は `MailboxPolicy` と実際の `MessageQueue` の挙動が一致する経路だけで mailbox を構築し、bounded queue の操作は非同期化されていない time-of-check/time-of-use の隙間に依存してはならない。この mailbox 構築は不変条件を MUST 保持する。

#### Scenario: registry 経由 mailbox は解決済み policy を使う

- **WHEN** actor が registry で解決された mailbox selector または mailbox id から作成される
- **THEN** mailbox は解決済み mailbox configuration を queue policy、capacity、overflow behavior、instrumentation の唯一の真実源として使う

#### Scenario: 外部から渡された queue で不変条件を迂回できない

- **WHEN** 事前構築済み queue を使って mailbox を構築しようとする
- **THEN** runtime は不整合な policy/queue の組を拒否するか、constructor を内部に閉じてモジュール外から不整合な組を作れないようにする

#### Scenario: bounded queue 操作は同期される

- **WHEN** prepend、enqueue、metrics publication、user length の確認が bounded mailbox で並行実行される
- **THEN** runtime は overflow 判定の誤り、stale metrics、data race を生む未同期の queue state を観測しない

#### Scenario: 同期プリミティブの選択は SharedLock/SharedRwLock と ActorLockProvider に一本化される

- **WHEN** actor runtime 内の同期プリミティブ選択メカニズムを確認する
- **THEN** `RuntimeMutex` / `RuntimeRwLock` への直接参照は存在しない
- **AND** 同期プリミティブは `SharedLock<T>` / `SharedRwLock<T>` を通じて使用される
- **AND** ドライバの選択は `ActorLockProvider` 経由で行われる
- **AND** `ActorLockProvider` を経由しない no_std デフォルト使用は `SharedLock::new_with_driver::<SpinSyncMutex<T>>` / `SharedRwLock::new_with_driver::<SpinSyncRwLock<T>>` で行われる
