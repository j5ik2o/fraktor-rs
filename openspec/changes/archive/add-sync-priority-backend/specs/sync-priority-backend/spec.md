## ADDED Requirements

### Requirement: PriorityMessage に基づき優先度を判定する
`SyncPriorityBackend<T>` は `T: PriorityMessage` を要求しなければならない (MUST)。`offer` 時には `PriorityMessage::get_priority()` が返す `Option<i8>` を読み取り、`None` の場合はバックエンド既定の優先度に置き換えなければならない (MUST)。`poll` は数値が大きい優先度ほど先に返し、同一優先度内では FIFO を維持しなければならない (MUST)。

#### Scenario: PriorityMessage の優先度が poll 順序を決定する
- Given 容量 `4`、ポリシー `DropOldest` の優先度バックエンドがある
- And `PriorityMessage` を実装した要素 `A(priority = Some(1))`, `B(priority = Some(5))`, `C(priority = None)` が存在する
- When それぞれを任意の順序で `offer` する
- Then `poll` を 3 回呼び出すと `B`, `C`, `A` の順に取り出され、`C` は既定優先度（中間値）として扱われる

### Requirement: PriorityMessage の最小優先度を参照できる
`SyncPriorityBackend<T>` は最小優先度レベルにある要素への参照を `peek_min` で返さなければならない (MUST)。返却時に要素を削除してはならない (MUST)。

#### Scenario: peek_min は最小優先度を返す
- Given 上記と同じバックエンドに `A(priority = Some(1))`, `B(priority = Some(5))`, `C(priority = None)` が格納されている
- When `peek_min` を呼び出す
- Then `Some(&A)` が返り、その後 `poll` を呼ぶと最初に `B` が返る
