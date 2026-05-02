# streams-backpressure-integrity Specification

## Purpose
TBD - created by archiving change resolve-bugbot-and-coderabbit-major-issues. Update Purpose after archive.
## Requirements
### Requirement: `Source::create` は非同期 producer の速度差を許容する

std streams API は `Source::create` の producer が非同期に値を流しても、producer が同期 polling より遅いという理由だけで失敗しないようにしなければならない。この API は非同期 producer の速度差を MUST 許容する。

#### Scenario: 遅い producer でも idle budget 超過で失敗しない

- **WHEN** background producer が `offer()` の間に観測可能な遅延を挟んで要素を送る
- **THEN** `Source::create` は `StreamError::WouldBlock` で恒久失敗するのではなく、非同期 contract に従って待機を継続する

### Requirement: source queue の backpressure は pending work と wake discipline を保持する

source queue の実装は pending offer を accept または terminal reject されるまで保持し、wake notification は state transition によって進捗可能になった場合にだけ送らなければならない。この backpressure は pending work と wake discipline を MUST 保持する。

#### Scenario: backpressure で pending offer を黙って捨てない

- **WHEN** source queue が backpressure mode で複数 offer を保留している
- **THEN** queue は overflow contract に従って各 offer を明示的に保持または reject し、accepted work を黙って破棄しない

#### Scenario: state 変化なしでは self-wake しない

- **WHEN** `QueueOfferFuture::poll` が進捗不能な状態で呼ばれる
- **THEN** poll result を変えられる state transition がない限り、自分自身を繰り返し wake しない

#### Scenario: 進捗可能になったら待機 task を wake する

- **WHEN** capacity が空く、または terminal state が pending offer の結果を変える
- **THEN** waiting task は wake notification を受け取り、その遷移は test で観測できる

### Requirement: async callback と timer の出力は途中の apply failure で失われない

graph interpreter は async callback や timer から取り出した出力を、その後の stage apply が continue または complete で失敗した場合でも失ってはならない。この出力は途中の apply failure でも MUST 保持される。

#### Scenario: apply failure が continue でも取り出した出力を保持する

- **WHEN** async または timer の出力を収集した後、後続の `apply` が continue disposition で失敗する
- **THEN** 収集済みの出力は破棄されず、後続の delivery に使える状態で保持される

#### Scenario: apply failure が complete でも terminal handling まで出力を保持する

- **WHEN** async または timer の出力を収集した後、後続の `apply` が stage を complete 側へ遷移させる
- **THEN** runtime は terminal handling が終わるまでその出力を保持し、取り返しのつかない形で失わせない

### Requirement: actor-backed stage は公開 API の契約どおりに振る舞う

actor-backed stream stage は、公開 API 名が示す delivery、acknowledgement、cancellation、terminal-state の契約をそのまま実装しなければならない。公開 API の契約どおりに MUST 振る舞わなければならない。

#### Scenario: `actor_ref` は要素を target actor へ転送する

- **WHEN** stage が `actor_ref` で構築される
- **THEN** 配送された要素は無視されずに target actor へ届く

#### Scenario: `actor_ref_with_backpressure` は acknowledgement を待つ

- **WHEN** stage が `actor_ref_with_backpressure` で構築される
- **THEN** 追加要素を accepted と見なす前に、公開された acknowledgement protocol を守って配送する

#### Scenario: graceful cancellation は handle を決定的に閉じる

- **WHEN** source queue または actor-backed source が graceful completion path で cancel される
- **THEN** 関連する handle と completion watcher は決定的に closed または completed 状態へ遷移する

### Requirement: island boundary は actor 分離後も backpressure と terminal signal を保持しなければならない

stream island boundary は、island が別 actor / 別 mailbox / 別 dispatcher で実行されても、要素、backpressure、completion、failure、cancellation を失ってはならない（MUST）。boundary full / empty を同期直実行の恒久 failure として扱ってはならない（MUST NOT）。

#### Scenario: boundary full は upstream island の pending として扱われる

- **GIVEN** upstream island actor が downstream boundary に要素を push している
- **AND** boundary capacity が満杯である
- **WHEN** upstream island actor が drive される
- **THEN** upstream island は要素を保持したまま pending になる
- **AND** 要素は drop されない
- **AND** downstream island が boundary を drain した後、upstream island は後続 drive で進捗できる

#### Scenario: boundary empty は downstream island の pending として扱われる

- **GIVEN** downstream island actor が boundary から pull している
- **AND** boundary が empty かつ open である
- **WHEN** downstream island actor が drive される
- **THEN** downstream island は failure ではなく pending として扱われる
- **AND** busy loop せず、次の drive command または boundary state transition まで待機する

#### Scenario: upstream completion は pending 要素の後に downstream completion になる

- **GIVEN** upstream island が completion に到達する
- **AND** boundary に未配送要素が残っている
- **WHEN** downstream island が boundary を drain する
- **THEN** downstream island は残り要素を受け取る
- **AND** 残り要素の後に completion を観測する

#### Scenario: upstream failure は downstream failure になる

- **GIVEN** upstream island actor が failure に到達する
- **WHEN** boundary が failure state に遷移する
- **THEN** downstream island actor は同じ materialized graph の failure として観測する
- **AND** downstream island は正常 completion として扱われない

#### Scenario: downstream cancellation は upstream island へ伝播する

- **GIVEN** downstream island actor が cancel される
- **WHEN** cancel が boundary に伝播する
- **THEN** upstream island actor は `Cancel(cause)` command を受け取る
- **AND** upstream island は新しい要素を boundary へ publish し続けない
- **AND** cancellation は boundary の data state だけで表現されず、materialized graph の control plane から upstream island actor へ配送される
- **AND** downstream cancellation を graph-wide `Shutdown` と同一視してはならない

> 前提: Pekko 互換仕様と Rust らしい設計の両立を、常に念頭に置いて判断する。

