## MODIFIED Requirements

### Requirement: ActorSystem の spawn は fire-and-forget で完了する
ActorSystem / ActorContext が子アクターを spawn する際、SystemMessage::Create の enqueue に成功した時点で処理を完了し、`pre_start` の結果を同期的に待ってはならない（MUST NOT）。`pre_start` などライフサイクル勘定は EventStream や Supervisor 経由で観測する。

#### Scenario: enqueue 成功で即時 ChildRef を返す
- **GIVEN** 親アクターが `ctx.spawn_child(props)` を呼び出す
- **WHEN** SystemMessage::Create が mailbox に enqueue される
- **THEN** spawn 呼び出しは即座に `ChildRef` を返し、親コンテキストの処理をブロックしない

#### Scenario: ライフサイクル失敗は後続経路で観測する
- **GIVEN** 子アクターの `pre_start` がエラーを返す
- **WHEN** Supervisor が Failure/LifecycleEvent を受け取る
- **THEN** 失敗は EventStream や監督戦略を通じて通知され、spawn 呼び出し側では追加の ACK を待たない

### Requirement: enqueue 失敗のみが spawn エラーとなる
Create SystemMessage の enqueue が拒否された場合のみ `SpawnError` を返し、その他のライフサイクル失敗は Supervisor 経路で扱う（MUST）。

#### Scenario: Mailbox が満杯なら SpawnError を返す
- **GIVEN** 子アクターの System mailbox が満杯で `SystemMessage::Create` を受け付けられない
- **WHEN** ActorSystem が enqueue を試みる
- **THEN** spawn は `SpawnError::InvalidProps("create system message delivery failed")` を返し、セル登録をロールバックする

#### Scenario: enqueue 成功後の失敗は supervisor が処理する
- **GIVEN** Create の enqueue が成功して ChildRef が返却された
- **WHEN** その後の `pre_start` が panic などで失敗する
- **THEN** 失敗は Supervisor 戦略や EventStream で扱われ、spawn 呼び出しに retroactive なエラーを返さない
