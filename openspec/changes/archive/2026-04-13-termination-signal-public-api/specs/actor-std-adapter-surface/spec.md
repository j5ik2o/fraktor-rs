## ADDED Requirements

### Requirement: std adapter は termination blocking 用の `Blocker` 実装を提供しなければならない

std adapter は、core の `Blocker` port 契約を満たす termination 用 blocking 実装を提供しなければならない。これにより同期 std アプリケーションは busy wait なしで actor system termination を待機できなければならない。

#### Scenario: std adapter から `Blocker` 実装を取得できる
- **WHEN** 利用者が std 環境で actor system termination を同期的に待ちたい
- **THEN** `fraktor_actor_adaptor_rs::std` 配下から `Blocker` 契約を満たす型または helper に到達できる
- **AND** caller は `thread::yield_now()` ループを自前で書かなくてよい

#### Scenario: std adapter の blocking 実装は core の termination 契約と整合する
- **WHEN** std adapter の `Blocker` 実装を使って `TerminationSignal` の完了を待つ
- **THEN** actor system termination 後に待機は解除される
- **AND** 複数 observer が同じ `TerminationSignal` を観測しても終了状態が消費されない
