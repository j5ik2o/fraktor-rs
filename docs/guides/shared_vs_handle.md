# 共有ラッパーとハンドル命名・分離ガイド

## 目的
- 内部可変性を `&mut self` の純粋なロジックに集約しつつ、共有が必要な場合だけ同期ラッパーを被せる方針を明文化する。
- `*Shared` と `*Handle` の役割を明確にし、命名の一貫性と最適化余地を確保する。

## 基本ポリシー
- **ロジック本体は `Xyz`構造体**
  - フィールドは所有権ベースで持ち、可変メソッドは極力 `&mut self`。読み取り専用メソッドは`&self`。
  - 同期や共有の責務は持たない。
- **共有ラッパーは `XyzShared`構造体**
  - 典型実装: `ArcShared<ToolboxMutex<Xyz, TB>>` を内包。
  - 外向き API は `&self` でロックを取り、`Xyz` の `&mut self` メソッドを呼ぶだけの薄い層に留める。
  - ガードやロックを外部に返さない（ロック区間をメソッド内に閉じる）。
- **管理責務を持つ場合は `XyzHandle`構造体**
  - ライフサイクル制御（起動・停止・リソース解放）、複数構成要素の束ね、監視・メトリクスなど「単なるロック以上」の責務を持つ場合は `*Handle` を選ぶ。
  - `Handle` からは必要に応じて `XyzShared` や複数のリソースを操作する。

## 推奨 API 例
```rust
pub struct Xyz { /* state */ }

impl Xyz {
    pub fn do_something(&mut self, arg: T) -> Result<()> { /* logic */ }
}

#[derive(Clone)]
pub struct XyzShared<TB: RuntimeToolbox> {
    inner: ArcShared<ToolboxMutex<Xyz, TB>>,
}

impl<TB: RuntimeToolbox> XyzShared<TB> {
    pub fn do_something(&self, arg: T) -> Result<()> {
        self.inner.lock().do_something(arg)
    }
}
```

## 命名のチェックリスト
- **薄い同期ラッパー**: `*Shared`
- **ライフサイクル/管理責務**: `*Handle`
- **所有権一意・同期不要**: `*` のまま（`ArcShared` やロックを持たない）

## 既存コードの移行手順
1. `Xyz` にロジックを集約し、可変操作を行うメソッドは`&mut self` 化できる範囲を洗い出す。
2. `XyzShared` を「ロックして呼ぶだけ」に薄くする。複雑な処理が入っていれば `Xyz` へ移す。
3. 複合的な管理責務がある場合は `XyzHandle` に切り出し、`Shared`/`Handle` の役割を分ける。
4. テストは可能なら `Xyz` 単体（ロックなし）で書き、`Shared` 経由のパスは最小限に留める。
5. ロック順序が必要な場合は `XyzShared` 内の実装で明示し、ガードを外に渡さない。

## 最適化の考え方
- 共有が不要な場面では `Xyz` を直接使うことで `Arc`/ロックのコストをゼロにできる。
- ホットパスではロック粒度を小さくするため、`Xyz` に「まとめて処理する」メソッドを追加し、`Shared` 側は単一ロックで呼び出す。

