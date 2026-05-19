# fraktor-rs 命名規約

## 原則

**名前は責務・境界・依存方向を最小限の語で符号化する。曖昧な名前は設計が未完成であることを示す。**

## 参照実装の命名優先

**Apache Pekko（および protoactor-go）で確立されたドメイン用語は、プロジェクト内の命名規約より優先する。**

fraktor-rs はアクターフレームワークであり、Pekko / protoactor-go を参照実装としている。
`SupervisorStrategy`、`Behavior`、`Props` 等のドメイン用語は参照実装に合わせること。
責務別命名パターン（`*Policy` 等）との衝突が生じた場合は、参照実装の命名を採用する。

## 禁止サフィックス（ambiguous-suffix-lint で機械的に強制）

禁止サフィックスと責務別の代替案は `../avoiding-ambiguous-suffixes.md` を正とする。
Rust では `ambiguous-suffix-lint` により、この表に含まれるサフィックスを機械的に検出する。

### 例外

- 外部 API / OSS / フレームワーク由来の名称は `#[allow(ambiguous_suffix::ambiguous_suffix)]` で明示的に許可

### 判定フロー

```
1. 禁止サフィックスを含むか？
   ├─ No → OK
   └─ Yes → 次へ

2. この名前だけで責務を一文で説明できるか？
   ├─ Yes → 外部API由来なら #[allow] で許可
   └─ No → ../avoiding-ambiguous-suffixes.md の代替案から具体名を選ぶ
```

## Shared / Handle 命名

| サフィックス | 用途 | 条件 |
|--------------|------|------|
| `*Shared` | 薄い同期ラッパー | `ArcShared<SpinSyncMutex<T>>` を内包するだけ |
| `*Handle` | ライフサイクル / 管理責務 | 起動・停止・リソース解放・複数構成要素の束ね |
| サフィックスなし | 所有権一意・同期不要 | `ArcShared` やロックを持たない |

### 詳細

- `*Shared` は `SharedAccess` 準拠の `with_read` / `with_write` に API を絞る
- `*Handle` も基本は `with_write` / `with_read` を提供し、複合操作をまとめる
- 管理対象が複数の場合は `*HandleSet` / `*Context` で「束ね役」であることを明示
- 詳細は `docs/guides/shared_vs_handle.md` を参照

## ファイル・ディレクトリ・型の命名

| 対象 | 規約 | 例 |
|------|------|-----|
| ファイル | `snake_case.rs` | `actor_cell.rs` |
| ディレクトリ | `snake_case/` | `actor_cell/` |
| 型 / trait | `PascalCase` | `ActorCell` |
| クレート | `fraktor-<domain>-rs` | `fraktor-actor-rs` |
| Cargo features | `kebab-case` | `tokio-executor` |

## ドキュメント言語

- rustdoc（`///`, `//!`）→ 英語
- それ以外のコメント・Markdown → 日本語

## 最終チェック

「この名前だけ見て、何に依存してよいか分かるか？」

分からないなら、その名前はまだ設計途中である。
