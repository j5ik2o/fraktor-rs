---
name: creating-fraktor-modules
description: fraktor-rsの規約（7つのDylint lint）に沿った新規モジュール・型の雛形を生成する。no_std/std分離、テスト配置、FQCN import、1ファイル1型を自動適用。トリガー：「新しいモジュールを作りたい」「型を追加したい」「ファイル構造を作って」「new module」「モジュール追加」「構造体を新設」等の新規コード作成リクエスト時に使用。
---

# fraktor-rs 新規モジュール作成

fraktor-rs の lint ルールに準拠した新規モジュール・型の雛形を一括生成する。

## Workflow

### 1. ヒアリング

以下を確認する：

- **型名**: PascalCase で（例: `TickDriverConfig`）
- **種別**: struct / trait / enum
- **配置先クレート**: actor / utils / remote / cluster / streams / persistence
- **レイヤー**: core（no_std）/ std / 両方
- **TB ジェネリクス**: `RuntimeToolbox` が必要か（core 側は通常必要）
- **親モジュール**: 既存モジュールの下か、新規トップレベルか

### 2. 既存パターンの確認

`mcp__serena__find_symbol` や `mcp__serena__get_symbols_overview` で、同じレイヤー・同じ種類の既存実装を2-3個確認する。

確認項目：
- ファイル構造（foo.rs + foo/ の有無）
- import パターン（crate:: 始まり）
- derive マクロの使い方
- エラー型の配置

### 3. ファイル構造生成

```
modules/<crate>/src/core/<parent>/
├── <module_name>.rs        # 型定義
└── <module_name>/
    └── tests.rs            # 単体テスト
```

#### 型定義ファイル（`<module_name>.rs`）

```rust
use crate::path::to::dependency::Dependency;

/// Brief description of the type (replace with actual description).
pub struct <TypeName>Generic<TB: RuntimeToolbox> {
    // fields
}

impl<TB: RuntimeToolbox> <TypeName>Generic<TB> {
    /// Creates a new instance (replace with actual description).
    pub fn new(/* args */) -> Self {
        Self { /* fields */ }
    }
}
```

#### テストファイル（`<module_name>/tests.rs`）

```rust
#[cfg(test)]
mod tests {
    use crate::path::to::<TypeName>Generic;

    // NoStdToolbox or StdToolbox for testing
    type <TypeName> = <TypeName>Generic<crate::core::toolbox::NoStdToolbox>;

    #[test]
    fn test_new() {
        let _instance = <TypeName>::new(/* args */);
    }
}
```

### 4. 親モジュールの更新

親の `<parent>.rs` に `pub mod <module_name>;` を追加する。

配置ルール（module-wiring-lint 準拠）：
- `pub mod` 宣言はファイル冒頭に配置
- `pub use` で再エクスポートする場合は末端モジュールのみ

### 5. TB ジェネリクスの判断

```
RuntimeToolbox が必要か？
├─ core 層で ArcShared / ToolboxMutex / Timer を使う → Yes（Generic サフィックス付与）
├─ std 層のみで使う → No（具体型で定義、StdToolbox 固定可）
└─ データ型のみ（状態なし） → No（ジェネリクス不要）
```

### 6. lint 準拠チェックリスト

生成後に以下を確認し、違反があれば該当箇所を修正する：

- [ ] 1ファイル1公開型（type-per-file-lint） → 違反時: 公開型を別ファイルに分離
- [ ] mod.rs 不使用（mod-file-lint） → 違反時: mod.rs を `<parent_name>.rs` にリネーム
- [ ] テストは `<name>/tests.rs`（tests-location-lint） → 違反時: テストを専用ファイルに移動
- [ ] import は `crate::` 始まり（module-wiring-lint） → 違反時: 相対パスを FQCN に変更
- [ ] `use` 宣言はファイル冒頭（use-placement-lint） → 違反時: `use` をファイル先頭に移動
- [ ] rustdoc は英語（rustdoc-lint） → 違反時: `///` コメントを英語に書き換え
- [ ] core 内に `#[cfg(feature = "std")]` なし（cfg-std-forbid-lint） → 違反時: std 依存を std 層に移動

## 使用例

### 例1: core 層に新しい trait を追加

**リクエスト**: 「MessageSerializer trait を serialization モジュールに追加したい」

**生成ファイル**:
- `modules/actor/src/core/serialization/message_serializer.rs`
- `modules/actor/src/core/serialization/message_serializer/tests.rs`

**親モジュール更新**: `serialization.rs` に `pub mod message_serializer;` 追加

### 例2: std 層に Tokio 依存の型を追加

**リクエスト**: 「TokioTransport を std/transport に追加したい」

**生成ファイル**:
- `modules/remote/src/std/transport/tokio_transport.rs`
- `modules/remote/src/std/transport/tokio_transport/tests.rs`

**注意**: std 層なので TB ジェネリクス不要、`StdToolbox` 固定。

## 参照ドキュメント

- `.kiro/steering/structure.md`: ディレクトリ構造の全体像と命名規則。配置先に迷った場合に参照
- `.kiro/steering/tech.md`: 技術スタックと lint 設定の詳細。lint エラーの原因調査時に参照
- `docs/guides/module_wiring.md`: FQCN import パターンの詳細ガイド。ステップ2・4 で import 記述に迷った場合に参照

## 出力ガイドライン

- 既存の類似モジュールのパターンに必ず合わせること
- 不要な derive マクロを付けないこと（既存パターンに従う）
- エラー型が必要な場合は別ファイルに分離
- テンプレートの rustdoc コメント（`///`）は実際の型に合わせた具体的な説明に置き換えること
