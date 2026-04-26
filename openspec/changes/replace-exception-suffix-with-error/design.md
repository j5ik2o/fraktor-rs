## 設計判断

### 新名

| 旧名 | 新名 | 採用理由 |
|------|------|----------|
| `ThrowableNotSerializableException` | `ThrowableNotSerializableError` | `Throwable` 接頭辞は Pekko 元概念識別 (元 throwable の代替 payload という役割) のため保持。`*Error` で Rust 慣習に揃える |
| `DurableStateException` | `DurableStateError` | Pekko `DurableStateException.scala` を直接参照する identity 識別子として `DurableState` を保持。`*Error` 接尾辞で慣習合わせ |

### Pekko 互換性との整合

両型とも Pekko 側に同名の class が存在する (`org.apache.pekko.remote.serialization.ThrowableNotSerializableException` / `org.apache.pekko.persistence.state.exception.DurableStateException`)。Rust 側で `*Error` に改名することで JVM 識別子と完全一致しなくなるが、fraktor-rs は wire 互換 (`SerializerId` / manifest など) のレベルで Pekko 互換を取る方針であり、Rust 識別子そのものを Pekko に合わせる意図はない。`Throwable` / `DurableState` 接頭辞を保持することで Pekko 由来であることは引き続き辿れる。

### `Throwable` 接頭辞を残す根拠

`ThrowableNotSerializableError` における `Throwable` は「元 throwable (Pekko / Java の Throwable インスタンス) を serialize できなかったことを示す代替 payload」という具体的な役割を識別する語として機能する。`NotSerializableError` (既存、別物) は serializer が serialize 不能と判定した汎用エラー型であり、本型と責務が異なる。`Throwable` を落とすと両者を識別できなくなるため保持する。

将来「Java/JVM 概念依存の名前を Rust 概念に置き換える」検討を行う場合は、`SerializationFallbackPayload` 等への再 rename を別 change で扱う余地がある (本 change の Non-Goals)。

### テスト関数名の扱い

`throwable_not_serializable_exception_preserves_original_message_and_class_name` のようなテスト関数名 (現在 2 件) は型名 rename と同時に `throwable_not_serializable_error_*` に揃える。テスト関数名は型名と直結するため、片方だけ残すと grep ナビゲーションで noise になる。tasks.md で同時改名を明記する。

`durable_state_exception/tests.rs` 内のテスト関数 (`fn xxx_failed_*` など) は型名を含まない命名のため改名不要。

### gap analysis 表記の更新範囲

`docs/gap-analysis/remote-gap-analysis.md` 内で `ThrowableNotSerializableException` を参照している箇所:
- L111 (Pekko 側参照): `ThrowableNotSerializableException.scala:22` という Pekko ファイル名はそのまま (Pekko の識別子)。fraktor-rs 対応列の表記を「未対応」→「対応済み (新名 `ThrowableNotSerializableError`)」に変更。
- L164 (Phase 1 trivial/easy 表): `ThrowableNotSerializableException` 相当 → `ThrowableNotSerializableError` 相当に更新、もしくは Phase 1 から外す (実装済みのため)。

`DurableStateException` は `docs/gap-analysis/remote-gap-analysis.md` には登場しないため gap analysis 側の更新は ThrowableNotSerializable 側のみで完結する。

### dylint との関係

現状 `ambiguous-suffix-lint` は `Manager` / `Util` / `Facade` / `Service` / `Runtime` / `Engine` を対象としており、`Exception` は含まれていない (`.agents/rules/rust/naming-conventions.md`)。本 change で `*Exception` を一掃した後、再発防止として `Exception` を ambiguous suffix lint の禁止リストに追加するかは別途検討する (本 change のスコープ外、Non-Goals)。

### takt 変換ルール文書の更新

調査の結果、Scala/Pekko → Rust の命名変換ルール文書 2 箇所に `*Exception` → `*Error` の対応が **欠落** していた:

- `.takt/facets/policies/fraktor-coding.md` 「Pekko → Rust 変換ルール」セクション (line 31-35): `Scala trait → Rust trait`、`Scala implicit → ジェネリクス`、`sealed trait → enum` のみ列挙され、`*Exception` 型 → `*Error` 型の対応が無い
- `.takt/facets/knowledge/pekko-porting.md` 「命名規約」表 (line 62-68): `camelCase → snake_case` (メソッド)、`PascalCase → PascalCase` (型) のみ列挙され、`*Exception` → `*Error` の対応が無い

両者とも本 change の本質的な動機 (Java/Scala 由来 `*Exception` を Rust 慣習の `*Error` に揃える) を新規 porting 作業に伝播するための 1 次資料であるため、本 change の tasks に記載追加を含める。これにより以後の Pekko porting で `*Exception` 型がそのまま Rust 側に再導入される事故を防ぐ。

`.agents/rules/rust/naming-conventions.md` の禁止サフィックス表への追加は **dylint `ambiguous-suffix-lint` の拡張とセットで扱うべき** (機械強制の伴わないルール追加だけだと inconsistent になる) ため、本 change では触らず Non-Goals とする。

### 移行戦略

- 後方互換 alias (例: `pub use ThrowableNotSerializableError as ThrowableNotSerializableException;`) は提供しない。CLAUDE.md「後方互換は不要 (破壊的変更を恐れずに最適な設計を追求すること)」「リリース状況: まだ正式リリース前の開発フェーズ」方針に基づき、新名のみを残す。
- 各 rename は「ファイル mv → 型名置換 → import 更新」の順序で 1 型ずつ完結させる (依存最小)。両型は別モジュール / 別 crate に独立しているため相互依存はなく、順序入れ替えも可能。

## 影響範囲

| crate | rename 対象ファイル | 参照更新ファイル |
|-------|-------------------|-----------------|
| `fraktor-actor-core-rs` | `serialization/throwable_not_serializable_exception.rs` | `serialization.rs` (mod / pub use)、`serialization/error/tests.rs` |
| `fraktor-persistence-core-rs` | `core/durable_state_exception.rs`、`core/durable_state_exception/` (tests dir) | `core.rs` (mod / pub use)、`core/durable_state_store.rs`、`core/durable_state_store_registry.rs`、`core/durable_state_store_registry/tests.rs` |
| `docs/` | (なし) | `docs/gap-analysis/remote-gap-analysis.md` |
