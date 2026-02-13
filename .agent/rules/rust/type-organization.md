# fraktor-rs 型配置ルール

## 原則

**1つの公開型につき1つのファイルを作成する（type-per-file-lint で機械的に強制）。**

ただし、以下の判定フローに従い、例外として同居を許可する場合がある。

## lint との関係

現在の `type-per-file-lint` はこの例外基準を認識しない（全公開型に分離を強制する）。同居が妥当と判断した場合は **人間に相談し、lint エラーへの対処方針を確認すること**。

## 判定フロー

```
1. この型は公開型（pub struct / pub trait / pub enum）か？
   ├─ No → 同居可（プライベート型は制約なし）
   └─ Yes → 次へ

2. 以下の除外対象に該当するか？
   - エラー型（*Error, *Failure）→ 常に独立ファイル
   - Shared/Handle 型 → 常に独立ファイル
   - テスト対象となる型 → 常に独立ファイル
   - ドメインプリミティブ（newtype）→ 常に独立ファイル
   ├─ 該当 → 独立ファイルに分離（例外不可）
   └─ 非該当 → 次へ

3. 以下の同居条件をすべて満たすか？
   a) 型が ≤20行（※計測基準を参照）
   b) 親型のフィールド・メソッド引数・戻り値としてのみ使われている
   c) 他のモジュールから直接参照されない（mcp__serena__find_referencing_symbols で確認）
   d) 同居先ファイルが同居後も 200行 を超えない
   ├─ すべて Yes → 同居可
   └─ 1つでも No → 独立ファイルに分離
```

## 除外対象の理由

| 型の種類 | 理由 |
|----------|------|
| エラー型（`*Error`, `*Failure`） | 独自の `From` / `Display` / `Error` 実装が伸びる |
| Shared/Handle 型 | 独自の同期責務・ライフサイクル責務を持つ |
| テスト対象となる型 | `<name>/tests.rs` との紐づけが曖昧になる |
| ドメインプリミティブ（newtype） | 独立した型安全性を提供する単位 |

## 同居条件の補足

### a) ≤20行の計測基準

以下をすべて含めて20行以下であること：
- `///` doc コメント
- `#[derive(...)]` 等の属性マクロ
- 型定義本体
- 関連する `impl` ブロック（ある場合）

### b) 「親型のフィールド・メソッド引数・戻り値としてのみ使われている」の確認方法

`mcp__serena__find_referencing_symbols` で参照元を調査し、すべての参照が親型の定義内（フィールド型、メソッドシグネチャ）に限定されていること。

## コード例

```rust
// ✅ 同居可: TickDriverConfig (親型) + TickMetricsMode (≤20行, フィールド型としてのみ使用)
// tick_driver_config.rs

/// Configuration for tick driver.
pub struct TickDriverConfig {
    kind: TickDriverKind,
    metrics_mode: TickMetricsMode,
}

/// Metrics publishing strategy (used only within TickDriverConfig).
pub enum TickMetricsMode {
    AutoPublish { interval: Duration },
    OnDemand,
}
```

```rust
// ❌ 同居不可: エラー型は除外対象（ステップ2で判定）
// tick_driver_error.rs

/// Errors during tick driver operation.
pub enum TickDriverError {
    AlreadyRunning,
    NotStarted,
    ConfigInvalid(String),
}

impl fmt::Display for TickDriverError { /* ... */ }
impl std::error::Error for TickDriverError {}
```

```rust
// ❌ 同居不可: ドメインプリミティブは除外対象（ステップ2で判定）
// tick_driver_id.rs

/// Unique identifier for a tick driver instance.
pub struct TickDriverId(u64);
```

## 禁止パターン

- 「関連しているから」という理由だけでの型の集約（判定フロー3の条件をすべて確認すること）
- 200行超のファイルへの型の追加
- 除外対象（エラー型・Shared型・Handle型・ドメインプリミティブ）の同居
- lint の `#[allow]` による type-per-file-lint の無効化（人間の許可なしで）

根拠: `claudedocs/actor-module-overengineering-analysis.md`（Phase 1-4 の分析実績）
