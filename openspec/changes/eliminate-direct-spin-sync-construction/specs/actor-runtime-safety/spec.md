## ADDED Requirements

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
