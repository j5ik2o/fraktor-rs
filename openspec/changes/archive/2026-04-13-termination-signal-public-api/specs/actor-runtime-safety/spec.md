## ADDED Requirements

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
