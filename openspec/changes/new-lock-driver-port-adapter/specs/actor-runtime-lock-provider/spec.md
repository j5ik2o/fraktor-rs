## ADDED Requirements

### Requirement: actor runtime hot path は system-scoped runtime lock provider で構築される

actor runtime hot path は、actor system ごとに束縛された runtime lock provider で構築されなければならない（MUST）。lock family の選択は実行時構成で行われ、`RuntimeMutex<T, D>` のような driver generic parameter や process-global mutable state に依存してはならない（MUST NOT）。

#### Scenario: system ごとに異なる provider を共存できる
- **WHEN** 同一プロセス内で 2 つの actor system が異なる runtime lock provider を設定して起動する
- **THEN** 各 system の hot path はそれぞれの provider で構築される
- **AND** 一方の provider 選択が他方の system の hot path 構成を変更しない

#### Scenario: public API は driver generic を公開しない
- **WHEN** `ActorSystem`、`ActorRef`、typed system、actor runtime hot path の public surface を確認する
- **THEN** それらの公開型に driver generic parameter は存在しない
- **AND** lock family の選択は runtime configuration 経由でのみ表現される

#### Scenario: provider dispatch は construction phase に閉じる
- **WHEN** actor runtime hot path の lock family 選択タイミングを確認する
- **THEN** provider の選択と materialization は configuration / bootstrap phase で完了する
- **AND** message hot path は provider の再解決を行わない
- **AND** hot path field に `RuntimeMutex<T, D>` のような generic driver parameter は保持されない

### Requirement: 第 1 段階は mutex 系 hot path に限定される

この capability の第 1 段階は mutex 系 hot path だけを対象にしなければならない（MUST）。非 hot path の `RuntimeMutex` / `RuntimeRwLock` caller と `RwLock` port 化を同時に要求してはならない（MUST NOT）。

#### Scenario: 非 hot path caller は既存 alias を使い続けられる
- **WHEN** hot path 以外の caller を確認する
- **THEN** それらは既存の `RuntimeMutex` / `RuntimeRwLock` alias を使い続けられる
- **AND** この change の適用だけでは workspace-wide な lock caller migration を要求されない

#### Scenario: hot path 対象は mutex 系 wrapper に限られる
- **WHEN** この capability の移行対象を確認する
- **THEN** 対象は `MessageDispatcherShared`、`ExecutorShared`、`ActorRefSenderShared`、`Mailbox` とその必要最小限の wiring に限られる
- **AND** `RuntimeRwLock` ベースの shared state は第 1 段階の必須対象ではない

#### Scenario: Mailbox は lock bundle で構築される
- **WHEN** `Mailbox` の provider integration を確認する
- **THEN** `Mailbox` は個別 lock を都度 resolve せず、同一 provider family で生成された `MailboxLockSet` を受け取る
- **AND** `run` / enqueue / cleanup の意味論は変化しない

### Requirement: std adapter は optional provider helper を提供する

std 環境では adapter が debug 用および std 用の runtime lock provider helper を提供しなければならない（MUST）。一方で core はそれら concrete 型へ依存してはならず、builtin spin provider だけで起動可能でなければならない（MUST）。

#### Scenario: std 環境で debug provider helper を選択できる
- **WHEN** std adapter の公開面を確認する
- **THEN** same-thread 再入検知に使える debug provider helper が存在する
- **AND** actor runtime はその helper から得た provider で hot path を構築できる

#### Scenario: std helper がなくても default provider で起動できる
- **WHEN** caller が std adapter helper を明示設定せずに actor system を起動する
- **THEN** actor runtime は builtin spin provider で hot path を構築できる
- **AND** core は std adapter の concrete 型名を参照しない

#### Scenario: debug provider は same-thread 再入を panic で観測できる
- **WHEN** caller が debug provider helper を明示選択した runtime で same-thread 再入 lock acquisition を起こす
- **THEN** runtime はその再入を panic として fail-fast で報告する
- **AND** tests は `catch_unwind` 等でこの panic を観測できる
