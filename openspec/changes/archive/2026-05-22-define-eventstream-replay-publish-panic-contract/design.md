## Context

`EventStreamShared` は `EventStream` の write lock を短く保持し、callback は lock 解放後に同期実行する。subchannel change では classifier による配送先 snapshot と buffered replay filtering が実装済みだが、次の 3 点は仕様化されていない。

- late subscriber が受ける replay と、同時に別 thread から publish される live event の順序
- `publish()` が return した時点で subscriber callback が完了済みかどうか
- `publish()` 中の subscriber callback が panic した場合に subscription を自動解除するかどうか

actor-core-kernel は no_std core であり、panic isolation を core contract に入れると `std::panic::catch_unwind` や unwind safety boundary が混入する。現行の `EventStreamSubscriberShared::notify` は subscriber lock の中で callback を直接呼ぶため、panic は呼び出し元に伝播する。

## Goals / Non-Goals

**Goals:**

- `subscribe_with_key` return 後には、登録時点で確定した replay snapshot が対象 subscriber へ同期通知済みであることを明文化する。
- `publish` return 後には、panic が発生しない限り、対象 subscriber callback が同期完了済みであることを明文化する。
- `publish()` 中の subscriber callback panic は catch せず、subscription を自動解除しない契約として固定する。
- 契約を rustdoc と targeted tests で検証可能にする。

**Non-Goals:**

- subscribe 中の並行 publish に対する strict `buffered -> live` ordering の実装。
- callback panic isolation、fail-fast policy registry、automatic unsubscribe policy の追加。
- 非同期 flush queue や background dispatcher の追加。
- `EventStreamSubscriber` trait の戻り値や error model の変更。

## Decisions

### 決定 1: replay は subscribe return 前の同期通知まで保証する

`EventStreamShared::subscribe_with_key` は write lock 中に subscriber registration と replay snapshot selection を行い、lock 解放後に snapshot を subscriber へ通知してから `EventStreamSubscription` を返す。このため、単一 thread / happens-before の範囲では、`subscribe_with_key` return 後に buffered replay は観測済みである。

`subscribe_no_replay` は replay snapshot を持たない live-only registration として扱い、本 change の replay 同期通知契約には含めない。既存どおり return 時点で subscription registration は完了しているが、buffered event の同期観測は `subscribe_with_key` / `subscribe` の契約である。

代替案として、登録前に replay を通知してから subscriber を追加する設計も考えられるが、replay と registration の間に publish された live event を落とすため採用しない。

### 決定 2: subscribe 中の並行 publish との厳密順序は保証しない

現行実装は event stream lock を callback 実行中に保持しない。これにより deadlock risk を抑える一方、subscriber registration 後かつ replay callback 完了前に別 thread が publish した live event は、subscriber lock の獲得順によって replay と interleave し得る。これを strict `buffered -> live` にするには per-subscriber pending queue や replay barrier が必要で、current use case に対して重い。

本 change では concurrent publish との厳密順序を保証しないことを仕様に書く。必要になった場合は、別 change で per-subscriber replay barrier を設計する。

### 決定 3: publish は同期 callback 契約を維持する

`EventStreamShared::publish` は write lock 中に event buffer と delivery target snapshot を確定し、lock 解放後に対象 subscriber を順に `notify` する。panic が発生しない限り、関数 return 時点で対象 subscriber の callback は完了済みである。

非同期 flush queue を導入すると publish latency は下がるが、event stream の観測契約が actor mailbox delivery と混ざり、既存 tests と診断 subscriber の扱いが複雑になるため採用しない。

### 決定 4: publish 中の subscriber panic は呼び出し元へ伝播し subscription lifecycle を変更しない

core 側で panic を catch しない。`publish()` 中に panic した callback 以降の subscriber への配送は保証しないが、panic した subscriber の subscription entry は自動削除しない。これは no_std core に std unwind boundary を持ち込まないための最小契約である。

panic 後も subscription が残る契約は、subscriber lock guard が unwind 経路でも drop される現行 lock backend に依存する。lock poisoning のような std-only policy は本 change では導入しない。

automatic unsubscribe は callback 実装者にとって便利に見えるが、panic と lifecycle policy を event stream が勝手に結合し、再現性の低い購読喪失を生むため採用しない。

## Risks / Trade-offs

- [Risk] subscribe 中の concurrent publish で live event が replay より先に callback される可能性が残る -> spec に非保証として明記し、strict ordering が必要な subscriber は subscribe 完了後に publisher を開始する運用にする。
- [Risk] panic した subscriber が残るため、後続 publish でも再 panic し得る -> subscriber 側の責務として panic しない callback を実装し、event stream は lifecycle を勝手に変更しない。
- [Risk] `publish` が同期 callback なので遅い subscriber が publisher をブロックする -> 既存 contract として維持し、非同期化は別 change で検討する。
