# actor-runtime-safety Specification

## Purpose
TBD - created by archiving change resolve-bugbot-and-coderabbit-major-issues. Update Purpose after archive.
## Requirements
### Requirement: Mailbox 構築は policy と queue の不変条件を保持する

actor runtime は `MailboxPolicy` と実際の `MessageQueue` の挙動が一致する経路だけで mailbox を構築し、bounded queue の操作は非同期化されていない time-of-check/time-of-use の隙間に依存してはならない。この mailbox 構築は不変条件を MUST 保持する。restart 中の mailbox suspend/resume は `fault_recreate` と `finish_recreate` で対称に扱われ、`finish_recreate` の `mailbox().resume()` は子が全員終了した後にのみ実行される (MUST)。

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

#### Scenario: restart 中の mailbox suspend は子終了まで維持される

- **WHEN** `fault_recreate(cause)` が実行され `set_children_termination_reason` が `true` を返す
- **THEN** `mailbox().is_suspended()` は子が全員終了し `finish_recreate` が実行されるまで真を返す
- **AND** `finish_recreate` 内の `mailbox().resume()` 呼び出しでのみ false に戻る

### Requirement: typed actor の再起動は interceptor の正しさを保つ

typed actor runtime は supervision 下で restart-safe を保ち、interceptor や deferred initialization から作られた behavior でも正しく再起動できなければならない。この再起動は interceptor の正しさを MUST 保持する。

#### Scenario: restart 後に intercepted behavior が再生成される

- **WHEN** intercepted behavior を使う supervised typed actor が restart 後に再び `Started` を受け取る
- **THEN** runtime は one-shot initialization に起因する panic や failure なしに intercepted behavior state を再生成または保持する

#### Scenario: supervisor strategy の参照は lock-safe である

- **WHEN** runtime code が restart や failure handling 中に supervision strategy state を読む
- **THEN** query のためだけに write access を要求しない lock-safe な read path で参照する

### Requirement: stash の観測は runtime lock 下で user callback を実行しない

typed stash buffer は caller が渡した predicate、equality check、iteration callback を実行する前に actor cell の内部 lock を解放しなければならない。runtime lock 下で user callback を実行してはならず、この観測は MUST lock-free callback evaluation を守る。

#### Scenario: `contains` は snapshot 後に評価される

- **WHEN** caller が typed message の存在を stash で確認する
- **THEN** stash buffer は actor cell lock の外で equality comparison を呼ぶ前に、対象 message を snapshot する

#### Scenario: `exists` と `foreach` は snapshot 後に評価される

- **WHEN** caller が stashed message を観測する predicate または iteration callback を渡す
- **THEN** stash buffer は lock 解放後にだけ callback を呼び出す

### Requirement: router と registration の挙動は実際の runtime 契約と一致する

actor runtime は、名前と効果が実際に提供する保証と一致する routing / registration behavior だけを公開しなければならない。公開挙動は実際の runtime 契約と MUST 一致しなければならない。

#### Scenario: consistent hashing は stable affinity を提供する

- **WHEN** group router が consistent-hash routing mode を公開し、routee set が変化する
- **THEN** key に対応する routee の選択は単純な `hash % routee_count` の再割当てではなく、consistent-hashing algorithm に従ってだけ変化する

#### Scenario: top-level registration の失敗は spawn 済み state を rollback する

- **WHEN** top-level actor registration または receptionist registration が spawn 中に失敗する
- **THEN** runtime は部分的に生成された state を rollback し、orphaned receptionist や top-level registration を残さない

#### Scenario: dispatcher selector は意図した blocking dispatcher を解決する

- **WHEN** props が registry-backed configuration を通じて blocking dispatcher を選択する
- **THEN** selector と registry lookup は同じ dispatcher id と executor semantics を解決する

### Requirement: debug deadlock detection に対する構築漏れを actor runtime に残してはならない

actor runtime は、debug 用 lock family に切り替えたときに再入や lock order 問題の観測漏れを残してはならない（MUST NOT）。runtime safety を検証したい production path に hard-coded `SpinSync*` 構築や fixed-family helper alias が残っていてはならない（MUST NOT）。

#### Scenario: actor runtime の production path は debug family へ切り替え可能である
- **WHEN** debug lock family を使う actor system で runtime safety を検証する
- **THEN** actor runtime の production path は debug family で構築される
- **AND** same-thread 再入や lock order 問題が hard-coded backend または fixed-family helper alias によって観測不能にならない

#### Scenario: 直 backend 構築または固定 driver 指定は runtime safety regression として扱われる
- **WHEN** actor runtime の production path に allow-list 外の direct `SpinSync*::new`、固定 `SpinSync*` driver 指定、または fixed-family helper alias が追加される
- **THEN** CI はそれを runtime safety regression として失敗させる
- **AND** debug deadlock detection の適用範囲が縮小したまま merge されない

### Requirement: termination 観測 API は低レベル shared future 操作を利用者へ強制してはならない

actor runtime は、termination 観測のために `with_read(|f| f.is_ready())` と `thread::yield_now()` のような低レベル shared future 操作を利用者へ強制してはならない。公開 termination API は runtime backend に依存しない安全な待機契約を MUST 提供しなければならない。

#### Scenario: public termination API は busy wait を前提にしない
- **WHEN** caller が `when_terminated()` 系 API の使い方を確認する
- **THEN** public contract だけで同期または非同期の安全な待機が完結する
- **AND** sample や public documentation は busy wait loop を唯一の正解として示さない

#### Scenario: ホスト実行モデルの差が termination 観測契約を壊さない
- **WHEN** caller が同期 `main` または非同期 `main` から actor system termination を観測する
- **THEN** caller は `TerminationSignal` を起点にした公開契約で待機できる
- **AND** core 利用者は `ActorFutureShared` などの low-level future primitive を直接扱わない
- **AND** 同期待機が必要な場合も `Blocker` port 経由で platform 依存を隔離する

