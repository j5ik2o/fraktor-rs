## ADDED Requirements

### Requirement: TAKT artifact は構造妥当性を保つ

リポジトリで管理する TAKT の piece、instruction、output contract は TAKT parser にとって構造妥当であり、schema の意味を変えるような壊れた indentation や code fence の入れ子を含んではならない。

#### Scenario: movement routing rule は sibling field として並ぶ

- **WHEN** movement が `output_contracts` と `rules` を定義する
- **THEN** 両方の field は正しい sibling indentation にあり、各 `next` entry は対応する rule item の下に入る

#### Scenario: output contract template は code fence を壊さない

- **WHEN** output contract が fenced template の中に example code block を埋め込む
- **THEN** template 全体が 1 つの文書として parse 可能なように、衝突しない fence delimiter を使う

### Requirement: TAKT instruction inventory は一貫して wiring される

リポジトリで管理する TAKT instruction file は、active な piece から参照されるか、tree から削除されるかのどちらかでなければならない。

#### Scenario: orphan instruction file を残さない

- **WHEN** TAKT instruction file が `.takt/facets/instructions` に存在する
- **THEN** 少なくとも 1 つの active piece がそれを参照するか、dead configuration として削除される

### Requirement: AI mode の cargo 実行は guard wrapper を経由する

CI helper script は AI mode のすべての `cargo` 実行経路を共有 guard wrapper 経由にし、timeout と hang-suspect protection を一貫して適用しなければならない。

#### Scenario: example 実行も AI mode では guard される

- **WHEN** `./scripts/ci-check.sh ai all` または同等の AI mode の example path が `cargo` を実行する
- **THEN** その実行は `cargo` を直接呼ばず、共有 guard wrapper を通る
