## ADDED Requirements

### Requirement: Mailbox 構築は policy と queue の不変条件を保持する
actor runtime は `MailboxPolicy` と実際の `MessageQueue` の挙動が一致する経路だけで mailbox を構築し、bounded queue の操作は非同期化されていない time-of-check/time-of-use の隙間に依存してはならない。

#### Scenario: registry 経由 mailbox は解決済み policy を使う
- **WHEN** actor が registry で解決された mailbox selector または mailbox id から作成される
- **THEN** mailbox は解決済み mailbox configuration を queue policy、capacity、overflow behavior、instrumentation の唯一の真実源として使う

#### Scenario: 外部から渡された queue で不変条件を迂回できない
- **WHEN** 事前構築済み queue を使って mailbox を構築しようとする
- **THEN** runtime は不整合な policy/queue の組を拒否するか、constructor を内部に閉じてモジュール外から不整合な組を作れないようにする

#### Scenario: bounded queue 操作は同期される
- **WHEN** prepend、enqueue、metrics publication、user length の確認が bounded mailbox で並行実行される
- **THEN** runtime は overflow 判定の誤り、stale metrics、data race を生む未同期の queue state を観測しない

### Requirement: typed actor の再起動は interceptor の正しさを保つ
typed actor runtime は supervision 下で restart-safe を保ち、interceptor や deferred initialization から作られた behavior でも正しく再起動できなければならない。

#### Scenario: restart 後に intercepted behavior が再生成される
- **WHEN** intercepted behavior を使う supervised typed actor が restart 後に再び `Started` を受け取る
- **THEN** runtime は one-shot initialization に起因する panic や failure なしに intercepted behavior state を再生成または保持する

#### Scenario: supervisor strategy の参照は lock-safe である
- **WHEN** runtime code が restart や failure handling 中に supervision strategy state を読む
- **THEN** query のためだけに write access を要求しない lock-safe な read path で参照する

### Requirement: stash の観測は runtime lock 下で user callback を実行しない
typed stash buffer は caller が渡した predicate、equality check、iteration callback を実行する前に actor cell の内部 lock を解放しなければならない。

#### Scenario: `contains` は snapshot 後に評価される
- **WHEN** caller が typed message の存在を stash で確認する
- **THEN** stash buffer は actor cell lock の外で equality comparison を呼ぶ前に、対象 message を snapshot する

#### Scenario: `exists` と `foreach` は snapshot 後に評価される
- **WHEN** caller が stashed message を観測する predicate または iteration callback を渡す
- **THEN** stash buffer は lock 解放後にだけ callback を呼び出す

### Requirement: router と registration の挙動は実際の runtime 契約と一致する
actor runtime は、名前と効果が実際に提供する保証と一致する routing / registration behavior だけを公開しなければならない。

#### Scenario: consistent hashing は stable affinity を提供する
- **WHEN** group router が consistent-hash routing mode を公開し、routee set が変化する
- **THEN** key に対応する routee の選択は単純な `hash % routee_count` の再割当てではなく、consistent-hashing algorithm に従ってだけ変化する

#### Scenario: top-level registration の失敗は spawn 済み state を rollback する
- **WHEN** top-level actor registration または receptionist registration が spawn 中に失敗する
- **THEN** runtime は部分的に生成された state を rollback し、orphaned receptionist や top-level registration を残さない

#### Scenario: dispatcher selector は意図した blocking dispatcher を解決する
- **WHEN** props が registry-backed configuration を通じて blocking dispatcher を選択する
- **THEN** selector と registry lookup は同じ dispatcher id と executor semantics を解決する
