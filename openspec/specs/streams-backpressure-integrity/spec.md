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

