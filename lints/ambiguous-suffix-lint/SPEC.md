# ambiguous-suffix-lint 仕様

## 目的

- 曖昧なサフィックス（Manager, Util, Facade, Service, Runtime, Engine）を持つ公開型を検出し、より責務が明確な命名への変更を促す。
- 型の名前だけで責務・境界・依存方向が推測できる状態を維持する。

## ルール

- 公開型（`pub struct` / `pub enum` / `pub trait`）の名前が禁止サフィックスで終わる場合は違反とみなす。
- `#[allow(ambiguous_suffix::ambiguous_suffix)]` で明示的に無効化されている場合のみ例外とする。
- ビルド生成物や `tests/` ディレクトリ配下、`*_tests.rs`・`tests.rs` は対象外とする。
- マクロ展開で生成された型定義は対象外とする。
- 非公開型（`pub` なし）は対象外とする。
- サフィックスと完全一致する名前（例: `Service` 単体）は対象外とする。

## 禁止サフィックス

| サフィックス | 問題 | 代替案 |
|------------|------|--------|
| Manager | 「Xxxに関することを全部やる箱」になる | Registry, Coordinator, Dispatcher, Controller |
| Util | 「設計されていない再利用コード」 | 具体的な動詞を含む名前 |
| Facade | 責務の境界が不明確 | Gateway, Adapter, Bridge |
| Service | 層や責務が未整理 | Executor, Scheduler, Evaluator, Repository, Policy |
| Runtime | 何が動くのか不明 | Executor, Scheduler, EventLoop, Environment |
| Engine | 実行体の責務が不明確 | Executor, Evaluator, Processor, Pipeline |

## 推奨される修正手順

1. 対象の型が担う責務を一文で定義する。
2. 責務に合った具体的な代替名をサフィックスの代替案から選ぶ。
3. 外部API/フレームワーク由来の名前で変更が困難な場合は `#[allow(ambiguous_suffix::ambiguous_suffix)]` で明示的に許可する。

## 診断メッセージ指針

- 違反箇所では型定義をハイライトし、検出されたサフィックスと代替案を `help` で提示する。
- `note` には判定基準（名前から責務が推測可能か）を案内する。
- `AI向けアドバイス` で責務定義 → 命名 → allow属性 の手順を補足する。
