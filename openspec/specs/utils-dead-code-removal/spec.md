# utils-dead-code-removal Specification

## Purpose
`modules/utils` の公開 API から未使用の共有・同期補助型を排除し、workspace で実際に使われている最小構成だけを維持する。

## Requirements
### Requirement: 未使用の共有・同期補助型は公開 API に存在しない
`modules/utils` は、workspace 内で未使用になった共有・同期補助型を公開 API として提供してはならない（MUST NOT）。`RcShared`, `StaticRefShared`, `SharedFactory`, `SharedFn`, `AtomicFlag`, `AtomicState`, `InterruptPolicy`, `CriticalSectionInterruptPolicy`, `NeverInterruptPolicy`, `AsyncMutexLike`, `SpinAsyncMutex`, `MpscBackend` は公開 API から除外されていなければならない（MUST）。

#### Scenario: 未使用型を import できない
- **WHEN** workspace の任意の crate から `modules/utils` の公開 API を通じて未使用型を import しようとする
- **THEN** その型は解決できず、コンパイル時に参照できない

#### Scenario: 残る型は workspace で使われるものだけ
- **WHEN** `modules/utils` の公開 API 一覧を確認する
- **THEN** workspace で参照されている共有・同期補助型だけが残っている
