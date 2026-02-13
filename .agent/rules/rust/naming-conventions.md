# fraktor-rs 命名規約

## 原則

**名前は責務・境界・依存方向を最小限の語で符号化する。曖昧な名前は設計が未完成であることを示す。**

## 禁止サフィックス（ambiguous-suffix-lint で機械的に強制）

| サフィックス | 問題 | 代替案 |
|--------------|------|--------|
| Manager | 「全部やる箱」になる | Registry, Coordinator, Dispatcher, Controller |
| Util | 設計されていない再利用コード | 具体的な動詞を含む名前（例: DateFormatter） |
| Facade | 責務の境界が不明確 | Gateway, Adapter, Bridge |
| Service | 層や責務が未整理 | Executor, Scheduler, Evaluator, Repository, Policy |
| Runtime | 何が動くのか不明 | Executor, Scheduler, EventLoop, Environment |
| Engine | 実行体の責務が不明確 | Executor, Evaluator, Processor, Pipeline |

### 例外

- 外部 API / OSS / フレームワーク由来の名称は `#[allow(ambiguous_suffix::ambiguous_suffix)]` で明示的に許可

### 判定フロー

```
1. 禁止サフィックスを含むか？
   ├─ No → OK
   └─ Yes → 次へ

2. この名前だけで責務を一文で説明できるか？
   ├─ Yes → 外部API由来なら #[allow] で許可
   └─ No → 代替案テーブルから具体名を選ぶ
```

## Shared / Handle 命名

| サフィックス | 用途 | 条件 |
|--------------|------|------|
| `*Shared` | 薄い同期ラッパー | `ArcShared<ToolboxMutex<T>>` を内包するだけ |
| `*Handle` | ライフサイクル / 管理責務 | 起動・停止・リソース解放・複数構成要素の束ね |
| サフィックスなし | 所有権一意・同期不要 | `ArcShared` やロックを持たない |

### 詳細

- `*Shared` は `SharedAccess` 準拠の `with_read` / `with_write` に API を絞る
- `*Handle` も基本は `with_write` / `with_read` を提供し、複合操作をまとめる
- 管理対象が複数の場合は `*HandleSet` / `*Context` で「束ね役」であることを明示
- 詳細は `docs/guides/shared_vs_handle.md` を参照

## 責務別命名パターン

| 責務 | 推奨パターン |
|------|------------|
| データ保持・管理 | `*Registry`, `*Catalog`, `*Index`, `*Table`, `*Store` |
| 選択・分岐・方針 | `*Policy`, `*Selector`, `*Router` |
| 仲介・調停・制御 | `*Coordinator`, `*Dispatcher`, `*Controller` |
| 生成・構築 | `*Factory`, `*Builder` |
| 変換・適合 | `*Adapter`, `*Bridge`, `*Mapper` |
| 実行・評価 | `*Executor`, `*Scheduler`, `*Evaluator` |

## ファイル・ディレクトリ・型の命名

| 対象 | 規約 | 例 |
|------|------|-----|
| ファイル | `snake_case.rs` | `actor_cell.rs` |
| ディレクトリ | `snake_case/` | `actor_cell/` |
| 型 / trait | `PascalCase` | `ActorCell` |
| クレート | `fraktor-<domain>-rs` | `fraktor-actor-rs` |
| Cargo features | `kebab-case` | `tokio-executor` |
| TB ジェネリクス付き | `*Generic` サフィックス | `ActorCellGeneric<TB>` |

## ドキュメント言語

- rustdoc（`///`, `//!`）→ 英語
- それ以外のコメント・Markdown → 日本語

## 最終チェック

「この名前だけ見て、何に依存してよいか分かるか？」

分からないなら、その名前はまだ設計途中である。
