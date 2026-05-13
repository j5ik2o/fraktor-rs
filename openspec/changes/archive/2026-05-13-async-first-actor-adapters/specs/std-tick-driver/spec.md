## MODIFIED Requirements

### Requirement: TickDriverKind は non_exhaustive で Std と Tokio variant を持つ

`TickDriverKind` に `#[non_exhaustive]` を付与し、`Std`、`Tokio`、`Embassy` variant を含めなければならない（MUST）。これにより std / Tokio / Embassy の環境別 driver を event stream metrics と snapshot で区別できなければならない（MUST）。下流 crate は将来の variant 追加に備えて wildcard arm を持たなければならない。

#### Scenario: TickDriverKind に Std、Tokio、Embassy が含まれる

- **GIVEN** 本 change が適用された状態
- **WHEN** `TickDriverKind` の variant を列挙する
- **THEN** `Auto`, `Manual`, `Std`, `Tokio`, `Embassy` の variant が存在する
- **AND** `#[non_exhaustive]` により下流 crate の `match` 文には wildcard arm が必須となる

#### Scenario: EmbassyTickDriver は Embassy kind を返す

- **GIVEN** `EmbassyTickDriver` が生成されている
- **WHEN** `kind()` が呼ばれる
- **THEN** `TickDriverKind::Embassy` が返る
- **AND** provision 後の `TickDriverProvision::kind` も `TickDriverKind::Embassy` である
