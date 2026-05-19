## 1. 事前確認

- [x] 1.1 `cargo test -p fraktor-actor-core-rs` ベースライン pass を確認 (rename 前)
- [x] 1.2 `cargo test -p fraktor-persistence-core-rs` ベースライン pass を確認 (rename 前)
- [x] 1.3 `rg -n 'pub (struct|enum|trait|type) \w+Exception\b' modules/ src/` で対象が以下 2 件のみであることを再確認
  - `modules/actor-core/src/core/kernel/serialization/throwable_not_serializable_exception.rs:7` の `ThrowableNotSerializableException`
  - `modules/persistence-core/src/core/durable_state_exception.rs:11` の `DurableStateException`
- [x] 1.4 `rg -n '\bThrowableNotSerializableException\b' modules/ src/ openspec/specs/` で参照が `actor-core` 内 4 箇所 (定義 1 + impl 1 + pub use 1 + tests 3) に閉じることを確認
- [x] 1.5 `rg -n '\bDurableStateException\b' modules/ src/ openspec/specs/` で参照が `persistence-core` 内のみ (定義 / impl / pub use / store / registry / tests の合計 18 箇所程度) に閉じることを確認

## 2. Phase 1 — `ThrowableNotSerializableException` → `ThrowableNotSerializableError`

依存関係: 単一 crate (`fraktor-actor-core-rs`) 内に閉じている。`pub use` 経由で公開されているが workspace 外の参照は無し。

- [x] 2.1 ファイルリネーム: `git mv modules/actor-core/src/core/kernel/serialization/throwable_not_serializable_exception.rs modules/actor-core/src/core/kernel/serialization/throwable_not_serializable_error.rs`
- [x] 2.2 リネーム後ファイルの中身を更新:
  - 構造体名 `ThrowableNotSerializableException` → `ThrowableNotSerializableError`
  - `impl ThrowableNotSerializableException` → `impl ThrowableNotSerializableError`
  - 冒頭 `//!` モジュール doc は内容に変化がなければ据え置き
- [x] 2.3 `modules/actor-core/src/core/kernel/serialization.rs` を更新:
  - `mod throwable_not_serializable_exception;` → `mod throwable_not_serializable_error;`
  - `pub use throwable_not_serializable_exception::ThrowableNotSerializableException;` → `pub use throwable_not_serializable_error::ThrowableNotSerializableError;`
- [x] 2.4 `modules/actor-core/src/core/kernel/serialization/error/tests.rs` を更新:
  - L5 の import: `ThrowableNotSerializableException,` → `ThrowableNotSerializableError,`
  - L93 の `ThrowableNotSerializableException::new(...)` → `ThrowableNotSerializableError::new(...)`
  - L101 の `ThrowableNotSerializableException::new(...)` → `ThrowableNotSerializableError::new(...)`
  - 同ファイル内のテスト関数名 `throwable_not_serializable_exception_preserves_original_message_and_class_name` → `throwable_not_serializable_error_preserves_original_message_and_class_name`
  - 同ファイル内のテスト関数名 `throwable_not_serializable_exception_is_cloneable_value_payload` → `throwable_not_serializable_error_is_cloneable_value_payload`
- [x] 2.5 `cargo build -p fraktor-actor-core-rs` 通過確認
- [x] 2.6 `cargo test -p fraktor-actor-core-rs` pass 確認 (rename 済みテスト関数 2 件含む)

## 3. Phase 2 — `DurableStateException` → `DurableStateError`

依存関係: 単一 crate (`fraktor-persistence-core-rs`) 内に閉じている。Phase 1 とは独立。

- [x] 3.1 ファイル / ディレクトリリネーム:
  - `git mv modules/persistence-core/src/core/durable_state_exception.rs modules/persistence-core/src/core/durable_state_error.rs`
  - `git mv modules/persistence-core/src/core/durable_state_exception modules/persistence-core/src/core/durable_state_error` (tests ディレクトリ)
- [x] 3.2 リネーム後ファイル `modules/persistence-core/src/core/durable_state_error.rs` を更新:
  - `pub enum DurableStateException` → `pub enum DurableStateError`
  - `impl DurableStateException` → `impl DurableStateError`
  - `impl Display for DurableStateException` → `impl Display for DurableStateError`
  - その他 `Self` を使っていない自己参照箇所すべて
- [x] 3.3 `modules/persistence-core/src/core/durable_state_error/tests.rs` (旧 `durable_state_exception/tests.rs`) を更新:
  - L3 の `use crate::core::durable_state_exception::DurableStateException;` → `use crate::core::durable_state_error::DurableStateError;`
  - L7 / L14 / L21 / L28 / L35 / L42 の `DurableStateException::*` 呼び出し全てを `DurableStateError::*` に
- [x] 3.4 `modules/persistence-core/src/core.rs` を更新:
  - `pub use durable_state_exception::DurableStateException;` 周辺の `mod` / `pub use` を `durable_state_error::DurableStateError` に書き換え
- [x] 3.5 `modules/persistence-core/src/core/durable_state_store.rs` を更新:
  - `use crate::core::durable_state_exception::DurableStateException;` → `use crate::core::durable_state_error::DurableStateError;`
  - `Pin<Box<dyn Future<Output = Result<T, DurableStateException>> + Send + 'a>>` → `... DurableStateError ...`
- [x] 3.6 `modules/persistence-core/src/core/durable_state_store_registry.rs` を更新:
  - L15 の import: `durable_state_exception::DurableStateException,` → `durable_state_error::DurableStateError,`
  - L41 の戻り値型 `Result<(), DurableStateException>` → `Result<(), DurableStateError>`
  - L44 の `DurableStateException::provider_already_registered(...)` → `DurableStateError::provider_already_registered(...)`
  - L57 の戻り値型 `Result<Box<dyn DurableStateStore<A>>, DurableStateException>` → `... DurableStateError>`
  - L59 の `DurableStateException::provider_not_found(...)` → `DurableStateError::provider_not_found(...)`
- [x] 3.7 `modules/persistence-core/src/core/durable_state_store_registry/tests.rs` を更新:
  - L16 の import (`durable_state_exception::DurableStateException,`) → `durable_state_error::DurableStateError,`
  - L25 の type alias の `DurableStateException` → `DurableStateError`
  - L121 / L129 の `DurableStateException::*` バリアント参照 → `DurableStateError::*`
- [x] 3.8 `cargo build -p fraktor-persistence-core-rs` 通過確認
- [x] 3.9 `cargo test -p fraktor-persistence-core-rs` pass 確認

## 4. Phase 3 — gap analysis 同期

- [x] 4.1 `docs/gap-analysis/remote-gap-analysis.md` L111 表行 (Pekko `ThrowableNotSerializableException.scala:22` 行) の「fraktor-rs 対応」列を「未対応」→「対応済み (新名 `ThrowableNotSerializableError`)」に更新。「備考」列も「例外型追加で閉じる小さい差分」→「対応済み。Rust 慣習に合わせ `*Error` 命名」に更新
- [x] 4.2 `docs/gap-analysis/remote-gap-analysis.md` L164 「Phase 1: trivial / easy」表から `ThrowableNotSerializableException` 相当 行を削除 (実装済みのため Phase 1 残存項目から外す)。Phase 1 が空テーブルになる場合は表ごと削除し、見出しも整理
- [x] 4.3 `docs/gap-analysis/remote-gap-analysis.md` のサマリー (L41-48 付近) の `easy gap` カウントを 1 件減らす (現 `1` → `0`)
- [x] 4.4 `rg -n 'ThrowableNotSerializableException' docs/` で fraktor-rs 側を指す残存記述が無いことを確認 (Pekko 側のクラス名としての参照は許容)

## 5. Phase 4 — takt 変換ルール文書の追記

`*Exception` → `*Error` の Pekko → Rust 命名変換ルールが `.takt/facets/` の 2 つの 1 次資料に欠落しているため、再発防止として追記する。

- [x] 5.1 `.takt/facets/policies/fraktor-coding.md` の「Pekko → Rust 変換ルール」セクション (L31-35) に 1 行追加:
  - 既存項目: `Scala trait 階層 → Rust trait + 合成` / `Scala implicit → ジェネリクス` / `sealed trait + case classes → enum`
  - 追加項目: `Java/Scala の *Exception 型 → Rust の *Error 型 (例: ThrowableNotSerializableException → ThrowableNotSerializableError)`
- [x] 5.2 `.takt/facets/knowledge/pekko-porting.md` の「命名規約」表 (L62-68) に 1 行追加:
  - 既存行: `camelCase メソッド` / `PascalCase 型`
  - 追加行: `| *Exception 型 | *Error 型 | DurableStateException → DurableStateError, ThrowableNotSerializableException → ThrowableNotSerializableError |`
- [x] 5.3 追記後、両ファイルが既存 markdown フォーマット (見出しレベル / 表構造) を壊していないことを確認
- [x] 5.4 `rg -n '\*Exception\b' .takt/facets/` で他に同種の漏れが無いことを念のため再確認

## 6. Phase 5 — final-ci

- [x] 6.1 ワークスペース全体の参照漏れ最終確認: `rg -n '\bThrowableNotSerializableException\b|\bDurableStateException\b' modules/ src/ docs/ .takt/facets/ openspec/specs/ openspec/changes/` の出力が、本 change 自身の proposal/design/tasks 内記述および Pekko 側 `.scala` ファイル名の言及だけになっていることを確認
- [x] 6.2 `./scripts/ci-check.sh ai all` を実行して fmt / clippy / no-std / doc / unit-test / integration-test 全て pass
- [x] 6.3 マージ後、別 PR で本 change を archive + main spec sync (本 change は specs delta を持たないため sync 対象は無し)

## 7. レビュー対応 / 後追い

- [x] 7.1 PR レビュー対応 (CodeRabbit / Cursor Bugbot 指摘は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve)
- [x] 7.2 マージ後、`/opsx:archive replace-exception-suffix-with-error` で archive
