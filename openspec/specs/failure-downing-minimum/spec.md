## Purpose

Failure observation と downing decision の最小境界を定義し、failure detector / membership coordination が観測した availability state を、それだけで member departure input として扱わないことを固定する。

## Requirements

### Requirement: Failure observation は downing decision から分離される

Failure detector と membership coordination は remote member の availability observation を suspect / reachable / unreachable state として表現する SHALL。Failure observation は、それだけで member departure input として扱われない MUST。

#### Scenario: suspect observation は departure ではない

- **WHEN** failure detector が member authority を unavailable と判定する
- **THEN** membership coordination はその member を suspect または unreachable observation として表現する
- **AND** active topology から authority を削除しない

#### Scenario: recovered observation は active member を復帰させる

- **WHEN** suspect または unreachable として観測された member から heartbeat または availability signal が戻る
- **AND** その member に対する downing decision が未発行である
- **THEN** membership coordination はその member を reachable member として扱う
- **AND** member departure input を生成しない

#### Scenario: down decision 後の recover signal は departure を取り消さない

- **WHEN** downing strategy が authority を down する decision を返す
- **AND** その後に同じ authority から heartbeat または availability signal が戻る
- **THEN** cluster core は既出の down decision を維持する
- **AND** recover signal は member departure input を取り消さない

### Requirement: Downing decision は core-defined port で表現される

Cluster core は explicit down command または failure observation から member を down するかどうかを決める decision boundary を定義する SHALL。Downing strategy は `DowningInput::ExplicitDown` と `DowningInput::FailureObservation` に対して `DowningDecision::Down` / `DowningDecision::Keep` / `DowningDecision::Defer` を返せる MUST。Std adapter は detector implementation や runtime scheduling を供給しても、Grain runtime policy を所有しない MUST。

#### Scenario: explicit down は decision boundary を通る

- **WHEN** caller が authority に対して explicit down command を実行する
- **THEN** cluster core は `DowningInput::ExplicitDown` で core-defined downing decision boundary を呼び出す
- **AND** decision が down を許可した場合だけ member departure input を provider-neutral に生成する

#### Scenario: failure observation は strategy decision へ渡される

- **WHEN** membership coordination が configured threshold を超えた suspect または unreachable observation を持つ
- **THEN** cluster core は downing strategy が判断できる input としてその observation を扱う
- **AND** strategy decision が出るまでは Grain runtime の topology invalidation を実行しない

### Requirement: Member departure input は provider-neutral に適用される

Downing decision が member departure を選んだ場合、cluster core はその authority を provider-neutral departure input として Grain runtime contract へ渡す SHALL。Identity lookup、placement、activation、PID cache invalidation は failure detector implementation、phi value、SBR details を inspect しない MUST。

#### Scenario: down decision は departure input を生成する

- **WHEN** downing strategy が authority を down する decision を返す
- **THEN** cluster core はその authority に対する member departure input を生成する
- **AND** Grain runtime は stale activation と PID cache を invalidation contract に従って処理する

#### Scenario: keep または defer decision は topology を保持する

- **WHEN** downing strategy が authority に対して keep または defer 相当の decision を返す
- **THEN** cluster core は active topology からその authority を削除しない
- **AND** Grain runtime は member departure として扱わない

### Requirement: SBR と reachability matrix は対象外に保たれる

この capability は failure observation と downing decision の最小 contract だけを定義する SHALL。Split Brain Resolver behavior、reachability matrix semantics、quorum policy、rebalance、remembered entity recovery、in-flight drain は定義しない MUST。

#### Scenario: partition policy はこの capability で決めない

- **WHEN** network partition が複数 member の suspect または unreachable observation を発生させる
- **THEN** この capability は多数派、最小ロール、static quorum などの partition policy を定義しない
- **AND** 将来の SBR capability が decision policy を追加できる境界を残す
