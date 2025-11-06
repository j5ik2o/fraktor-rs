## ADDED Requirements
### Requirement: Typed Ask Response API
Typed レイヤーは `TypedAskResponseGeneric<R, TB>` を提供し、ask 応答の reply handle と future を型安全に扱えるようにしなければならない (MUST)。

#### Scenario: typed ask returns typed response handle
- **WHEN** `TypedActorRefGeneric<M, TB>::ask::<R>(message)`を呼び出す
- **THEN** `TypedAskResponseGeneric<R, TB>` が返り、`reply_to()` は `TypedActorRefGeneric<R, TB>` を提供する
- **AND** `R` 以外の型を送信しようとするとコンパイルエラーになる

#### Scenario: typed ask future yields R without downcast
- **GIVEN** `TypedAskResponseGeneric<R, TB>` を取得している
- **WHEN** `future()` から得た Future/ArcShared を `try_take()` 等で解決する
- **THEN** `R` が直接取得でき、`AnyMessageGeneric` への手動 downcast を要求しない

### Requirement: Typed Ask API Contract
Typed ask は `R` のコンパイル時制約を設け、返信が型不一致だった場合はランタイムで検出しなければならない (MUST)。

#### Scenario: typed ask enforces R bounds at compile time
- **WHEN** `TypedActorRefGeneric::ask::<R>` を呼び出す
- **THEN** `R` が `Send + Sync + 'static` を満たさない場合はコンパイルエラーとなる

#### Scenario: runtime mismatch surfaces as error
- **WHEN** 返信側が `R` 以外の型を返す
- **THEN** typed ask future は即座に型不一致エラーを示し、パニックせずに呼び出し側へ伝播する

### Requirement: Remove Untyped Ask Response
`AskResponseGeneric` を利用する API は削除され、typed ask のみが公開されなければならない (MUST)。

#### Scenario: only typed ask remains in public API
- **WHEN** crate の公開 API を参照する
- **THEN** `AskResponseGeneric` は re-export も含めて非公開となり、`TypedAskResponseGeneric` のみが提供される

### Requirement: Validation & Examples
新 API はテストとサンプルで検証されなければならない (MUST)。

#### Scenario: unit tests cover typed ask happy path and mismatch
- **WHEN** CI の単体テストを実行する
- **THEN** typed ask が期待通りに成功するケースと型不一致エラーを返すケースがカバーされる

#### Scenario: examples demonstrate typed ask usage
- **WHEN** `cargo run --example ...` で typed ask のサンプルを実行する
- **THEN** 未来のユーザーが reply handle の使い方と typed future からの値取得方法を確認できる
