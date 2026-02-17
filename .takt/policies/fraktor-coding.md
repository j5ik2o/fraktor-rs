# fraktor-rs コーディングポリシー

## 原則

| 原則 | 基準 |
|------|------|
| Less is more / YAGNI | 要件達成に必要最低限の設計。「将来使うかも」は REJECT |
| 後方互換不要 | 破壊的変更を恐れず最適な設計を追求 |
| 一貫性 | 既存の実装パターンに従う。独自パターンの導入は REJECT |

## 構造ルール（Dylint lintで機械的強制）

以下の lint に違反する実装は REJECT:

| lint | 内容 |
|------|------|
| type-per-file | 1公開型 = 1ファイル |
| mod-file | mod.rsではなく型名.rsでモジュール定義 |
| module-wiring | モジュール配線の整合性 |
| tests-location | テストは `{name}/tests.rs` に配置 |
| use-placement | use文は関数内ではなくファイル先頭 |
| rustdoc | 公開型にはrustdoc（英語）必須 |
| cfg-std-forbid | coreモジュールでのstd依存禁止 |
| ambiguous-suffix | Manager/Util/Facade/Service/Runtime/Engine 禁止 |

## 可変性ポリシー

| ルール | 基準 |
|--------|------|
| 内部可変性 | デフォルト禁止。可変操作は `&mut self` で設計 |
| 共有型 | AShared パターンのみ許容（ArcShared + ToolboxMutex） |
| `&self` + 内部可変性 | 人間の許可なく使用は REJECT |

## CQS (Command-Query Separation)

| 種類 | シグネチャ |
|------|-----------|
| Query | `&self` + 戻り値あり |
| Command | `&mut self` + `()` or `Result<(), E>` |
| `&mut self` + 戻り値 | CQS違反。分離するか人間の許可が必要 |

## 命名規約

| 対象 | 規約 |
|------|------|
| ファイル | `snake_case.rs` |
| 型/trait | `PascalCase` |
| rustdoc | 英語 |
| コメント/Markdown | 日本語 |
| 禁止サフィックス | Manager, Util, Facade, Service, Runtime, Engine |

## Pekko参照実装からの変換ルール

| Pekko パターン | Rust パターン |
|----------------|--------------|
| `trait Actor` | `BehaviorGeneric<TB, M>` |
| `ActorRef[T]` | `TypedActorRefGeneric<TB, M>` |
| `implicit` | `TB: RuntimeToolbox` パラメータ |
| `sealed trait` + case classes | `enum` |
| `FiniteDuration` | `ticks: usize`（tickベースモデル） |

## テストポリシー

- 新規作成した型・関数には必ず単体テストを追加
- テストファイルは `{type_name}/tests.rs` に配置
- テスト実行は必須。実装完了後に `cargo test` で結果確認
- テストをコメントアウトしたり無視したりしない

## 禁止事項

- lint エラーを `#[allow]` で回避（人間の許可なし）
- `#![no_std]` の core モジュールで std 依存を導入
- 参照実装を読まずに独自設計を進める
- CHANGELOG.md の編集（GitHub Action が自動生成）
