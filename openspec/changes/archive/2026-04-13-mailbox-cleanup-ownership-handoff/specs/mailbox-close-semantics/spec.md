## MODIFIED Requirements

### Requirement: Mailbox cleanup SHALL be owned by exactly one finalizer rather than detach-side lock serialization

`Mailbox` の close correctness は、detach 側が `user_queue_lock` を取得して direct cleanup する前提に固定されていてはならない（MUST NOT）。close request 後の cleanup は、mailbox finalizer として選出された 1 主体だけが実行しなければならない（MUST）。

この change における authoritative boundary は「cleanup owner の一意性」であり、Phase II の `user_queue_lock` 前提は移行対象である。

#### Scenario: detach-side direct cleanup is no longer mandatory
- **WHEN** `Mailbox::become_closed_and_clean_up` またはそれに相当する detach path を確認する
- **THEN** detach caller が常に `user_queue_lock` の下で direct cleanup を完結する設計にはなっていない
- **AND** cleanup ownership は finalizer election により決まる

#### Scenario: close request after fast path does not require detach-side drain to preserve correctness
- **GIVEN** producer が user enqueue fast path を通過した後に close request が立つ
- **WHEN** finalizer ownership handoff が有効である
- **THEN** close correctness は detach caller の direct drain ではなく finalizer の terminal cleanup により保たれる
- **AND** 後続 change で producer path の outer lock を再評価できる状態になる

