---
name: designing-fraktor-shared-types
description: fraktor-rsの共有型設計を対話的に支援する。&mut self vs &self + 内部可変性の判断、Shared/Handleパターン選択、SharedAccessテンプレート生成を行う。トリガー：「共有型を作りたい」「Sharedパターン」「内部可変性」「&mut selfか&selfか」「ASharedを新設」「shared design」「共有ラッパー」等の共有型設計リクエスト時に使用。
---

# fraktor-rs 共有型設計

&mut self 原則に基づく共有型の設計・実装を対話的に支援する。

## Workflow

### 1. ヒアリング

対象の型について以下を確認する：

- **型名**: 何を表す型か
- **共有要件**: 複数箇所から参照される必要があるか
- **状態変更**: 可変メソッドが必要か
- **ホットパス**: 高頻度で呼ばれるか

### 2. 判定フロー

```
この型は共有される必要があるか？
├─ No → &mut self で設計（第1選択）→ 完了
└─ Yes → 次へ

状態変更メソッドが必要か？
├─ No → ArcShared<T> で共有（読み取り専用）→ 完了
└─ Yes → 次へ

管理責務（ライフサイクル制御、監視等）があるか？
├─ No → *Shared パターン（薄い同期ラッパー）
└─ Yes → *Handle パターン（管理責務付き）
```

### 3. テンプレート生成

判定結果に応じて以下のコードを生成する。

#### Shared パターン（薄い同期ラッパー）

```rust
/// Logic body for Xyz. Mutable operations use &mut self.
pub struct XyzGeneric<TB: RuntimeToolbox> {
    // state fields
}

impl<TB: RuntimeToolbox> XyzGeneric<TB> {
    /// Creates a new instance.
    pub fn new(/* args */) -> Self {
        Self { /* fields */ }
    }

    /// Mutates internal state (Command).
    pub fn do_something(&mut self, /* args */) -> Result<(), XyzError> {
        // logic
    }

    /// Returns a read-only snapshot (Query).
    pub fn snapshot(&self) -> XyzSnapshot {
        // read-only
    }
}
```

```rust
/// Shared wrapper for Xyz. Interior mutability is confined here.
#[derive(Clone)]
pub struct XyzSharedGeneric<TB: RuntimeToolbox> {
    inner: ArcShared<ToolboxMutex<XyzGeneric<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<XyzGeneric<TB>> for XyzSharedGeneric<TB> {
    fn with_read<R>(&self, f: impl FnOnce(&XyzGeneric<TB>) -> R) -> R {
        self.inner.with_read(f)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut XyzGeneric<TB>) -> R) -> R {
        self.inner.with_write(f)
    }
}
```

#### Handle パターン（管理責務付き）

Shared と同じ構造だが、ライフサイクル制御メソッド（start/stop/shutdown 等）を追加。

### 4. デッドロック回避チェック

生成したコードに対して以下を確認：

- ロック内で別のロックを取得していないか
- ロック内で EventStream への通知を行っていないか（ロック外に移動すべき）
- ガードやロックを外部に返していないか（メソッド内で完結すべき）
- with_write 内で長時間のI/Oを行っていないか

### 5. ファイル配置

- `xyz.rs`: ロジック本体（XyzGeneric）
- `xyz_shared.rs`: 共有ラッパー（XyzSharedGeneric）
- `xyz/tests.rs`: ロジック本体のテスト（ロックなしで書く）

## 使用例

### 例1: 新しい Registry を共有したい

**リクエスト**: 「NameRegistry を複数の ActorCell から参照したい」

**判定**:
- 共有必要? → Yes（複数 ActorCell から参照）
- 状態変更? → Yes（register/unregister）
- 管理責務? → No（単純な登録/検索）

**結果**: Shared パターンを適用
- `NameRegistryGeneric<TB>`: &mut self で register/unregister
- `NameRegistrySharedGeneric<TB>`: SharedAccess 実装

### 例2: 読み取り専用の Config

**リクエスト**: 「ActorSystemConfig を複数箇所で参照したい」

**判定**:
- 共有必要? → Yes
- 状態変更? → No（設定は不変）

**結果**: `ArcShared<ActorSystemConfig>` で直接共有。Shared 型は不要。

## 参照ドキュメント

- `.agent/rules/rust/immutability-policy.md`: 判定フローの根拠となる内部可変性ポリシー。ステップ2の判断に迷った場合に参照
- `docs/guides/shared_vs_handle.md`: Shared/Handle パターンの詳細な実装ガイド。テンプレート生成時に既存パターンを確認する際に参照
- `.agent/rules/rust/cqs-principle.md`: Command（&mut self）と Query（&self）の分離原則。テンプレートのメソッド設計時に参照

## 出力ガイドライン

- ロジック本体のテストはロックなしで書くことを推奨
- TB ジェネリクスが不要な場合は `Generic` サフィックスを省略
- 既存の類似 Shared 型を `mcp__serena__find_symbol` で検索し、パターンの一貫性を確認
