# ambiguous-suffix-lint 仕様

## 目的

- 曖昧なサフィックス（Manager, Util, Facade, Service, Runtime, Engine）を持つ識別子名、モジュール名、ファイル名を検出し、より責務が明確な命名への変更を促す。
- すべての名前だけで責務・境界・依存方向が推測できる状態を維持する。

## ルール

- 型・trait・trait alias・type alias・関数・macro・const・static の名前が禁止サフィックスで終わる場合は違反とみなす。
- trait item / impl item（関連型・関連 const・method）の名前が禁止サフィックスで終わる場合は違反とみなす。
- generic parameter、関数引数、trait method 引数、ローカル変数、fn pointer parameter の名前が禁止サフィックスで終わる場合は違反とみなす。
- struct / union / enum variant field と enum variant の名前が禁止サフィックスで終わる場合は違反とみなす。
- `mod foo;` / `pub mod foo;` / `mod foo { ... }` の `foo` が禁止サフィックスで終わる場合は違反とみなす。
- `.rs` ファイルの stem（例: `association_runtime.rs` の `association_runtime`）が禁止サフィックスで終わる場合は違反とみなす。
- `#[allow(ambiguous_suffix::ambiguous_suffix)]` で明示的に無効化されている場合のみ例外とする。
- ビルド生成物や `tests/` ディレクトリ配下、`*_test.rs`・`*_tests.rs`・`tests.rs` は対象外とする。
- マクロ展開で生成された識別子は対象外とする。
- 非公開の型・関数・変数も対象とする。
- サフィックスと完全一致する名前（例: `Service` 単体）も違反とする。
- `use` で導入される外部名、`extern crate` 名、`extern` block 内の外部 item 名は外部API/フレームワーク由来の名前とみなし対象外とする。

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

1. 対象の名前が担う責務を一文で定義する。
2. 責務に合った具体的な代替名をサフィックスの代替案から選ぶ。
3. 外部API/フレームワーク由来の名前で変更が困難な場合は `#[allow(ambiguous_suffix::ambiguous_suffix)]` で明示的に許可する。

## 診断メッセージ指針

- 違反箇所では該当する識別子・モジュール宣言・該当ファイル内の先頭違反箇所をハイライトし、検出されたサフィックスと代替案を `help` で提示する。
- `note` には判定基準（名前から責務が推測可能か）を案内する。
- `AI向けアドバイス` で責務定義 → 命名 → allow属性 の手順を補足する。
