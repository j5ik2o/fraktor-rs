## ADDED Requirements

### Requirement: Mailbox::run は lock-free user queue backing でも既存 drain semantics を維持する

`Mailbox::run` は、通常 user queue の backing が lock-free MPSC queue に変わっても、既存の system-first drain、user throughput limit、throughput deadline、suspend gating、post-drain reschedule semantics を維持しなければならない (MUST)。

#### Scenario: lock-free backing でも system messages が先に drain される
- **WHEN** mailbox が lock-free user queue を持つ
- **AND** system messages と user envelopes の両方が queued されている
- **THEN** `Mailbox::run(throughput, throughput_deadline)` は user envelopes より先に system messages を drain しなければならない (MUST)
- **AND** user envelopes は既存どおり throughput / deadline / suspension に従って処理されなければならない (MUST)

#### Scenario: lock-free backing でも post-drain reschedule が維持される
- **WHEN** `Mailbox::run` 中に producer が lock-free user queue へ enqueue する
- **AND** enqueue が mailbox running state と競合する
- **THEN** drain 完了後に pending reschedule または queue 残量が観測され、dispatcher は既存どおり mailbox を再 schedule できなければならない (MUST)
