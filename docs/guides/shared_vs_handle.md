# 共有ラッパーとハンドル命名・分離ガイド

## 目的
- 内部可変性を `&mut self` の純粋なロジックに集約しつつ、共有が必要な場合だけ同期ラッパーを被せる方針を明文化する。
- `*Shared` と `*Handle` の役割を明確にし、命名の一貫性と最適化余地を確保する。

## 基本ポリシー
- **ロジック本体は `Xyz`構造体**
  - フィールドは所有権ベースで持ち、可変メソッドは極力 `&mut self`。読み取り専用メソッドは`&self`。
  - 同期や共有の責務は持たない。
- **共有ラッパーは `XyzShared`構造体**
  - 典型実装: `ArcShared<ToolboxMutex<Xyz<TB>, TB>>` を内包。
- 外向き API は `SharedAccess` 準拠の `with_read` / `with_write` に絞り、ロックを隠蔽する。
  - ガードやロックを外部に返さない（ロック区間をメソッド内に閉じる）。
- **管理責務を持つ場合は `XyzHandle`構造体**
  - ライフサイクル制御（起動・停止・リソース解放）、複数構成要素の束ね、監視・メトリクスなど「単なるロック以上」の責務を持つ場合は `*Handle` を選ぶ。
  - `Handle` も基本は `with_write`/`with_read` を提供し、複合操作をまとめて実行する。

## 推奨 API 例
```rust
pub struct XyzGeneric<TB: RuntimeToolbox> { /* state */ }

impl<TB: RuntimeToolbox> XyzGeneric<TB> {
    pub fn do_something(&mut self, arg: T) -> Result<()> { /* logic */ }
    pub fn snapshot(&self) -> Snapshot { /* read-only */ }
}

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

// 呼び出し側例
xyz_shared.with_write(|x| x.do_something(arg))?;
let snap = xyz_shared.with_read(|x| x.snapshot());
```

## 命名のチェックリスト
- **薄い同期ラッパー**: `*Shared`
- **ライフサイクル/管理責務**: `*Handle`
- **所有権一意・同期不要**: `*` のまま（`ArcShared` やロックを持たない）

## 既存コードの移行手順
1. `Xyz` にロジックを集約し、可変操作を行うメソッドは`&mut self` 化できる範囲を洗い出す。
2. `XyzShared` / `XyzHandle` は `with_read` / `with_write` に統一し、個別メソッド乱立を避ける。複雑な処理が入っていれば `Xyz` へ移す。
3. 複合的な管理責務がある場合は `XyzHandle` に切り出し、`Shared`/`Handle` の役割を分ける。
4. テストは可能なら `Xyz` 単体（ロックなし）で書き、`Shared` 経由のパスは最小限に留める。
5. ロック順序が必要な場合は `XyzShared` 内の実装で明示し、ガードを外に渡さない。

## 最適化の考え方
- 共有が不要な場面では `Xyz` を直接使うことで `Arc`/ロックのコストをゼロにできる。
- ホットパスではロック粒度を小さくするため、`Xyz` に「まとめて処理する」メソッドを追加し、`Shared` 側は単一ロックで呼び出す。
