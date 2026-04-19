計画に従って実装してください。
Piece Contextに示されたReport Directory内のファイルのみ参照してください。他のレポートディレクトリは検索/参照しないでください。
Report Directory内のレポートを一次情報として参照してください。不足情報の補完が必要な場合に限り、Previous Responseや会話履歴を補助的に参照して構いません（Previous Responseは提供されない場合があります）。情報が競合する場合は、Report Directory内のレポートと実際のファイル内容を優先してください。

## 最重要方針

Pekko の契約意図を Rust / fraktor-rs の設計原則を壊さずに実装すること。
見た目だけ Pekko に似せる実装は失敗とみなす。

## タスク分解（team_leader として実行される場合）

このステップは team_leader モードで実行されます。計画レポート（`00-plan.md`）の
「並行実装マップ」セクションを参照し、以下のルールでサブタスクに分解してください。

### 分解ルール
1. 計画レポートの**並行グループ**に従ってサブタスクを分ける
2. **同一ファイルを変更するタスクは同一サブタスクに割り当てる**（ファイル競合回避）
3. 依存関係がある場合は依存元を先のサブタスクに含める
4. 各サブタスクの instruction には以下を含める:
   - 担当するタスクの一覧（計画レポートから引用）
   - 変更対象ファイルの一覧
   - 他のサブタスクとの境界（「このファイルは変更しない」等）
5. max_parts（最大3）を超える場合は、ファイル競合しないグループにまとめる

**重要**: 実装と同時に単体テストを追加してください。
- 新規作成したクラス・関数には単体テストを追加
- 既存コードを変更した場合は該当するテストを更新
- テストファイルの配置: プロジェクトの規約に従う
- ビルド確認は必須。実装完了後、ビルド（型チェック）を実行し、型エラーがないことを確認
- テスト実行は必須。ビルド成功後、必ずテストを実行して結果を確認
- ファイル名・設定キー名などの契約文字列を新規導入する場合は、定数として1箇所で定義すること

**実装完了条件（必須）:**
実装後に必ず以下を実行し、全チェックがパスすることを確認してからレポートに記録すること:
1. 変更範囲に対応する lint / 型チェック（例: `cargo clippy -p <crate> -- -D warnings`）
2. 変更範囲に対応する最小限のテスト（例: `cargo test -p <crate>`）
成功ログをcoder-decisionsレポートの「実行結果」セクションに含めること。

**Scope出力契約（実装開始時に作成）:**
```markdown
# 変更スコープ宣言

## タスク
{タスクの1行要約}

## 変更予定
| 種別 | ファイル |
|------|---------|
| 作成 | `src/example.ts` |
| 変更 | `src/routes.ts` |

## 推定規模
Small / Medium / Large

## 影響範囲
- {影響するモジュールや機能}
```

**Decisions出力契約（実装完了時、決定がある場合のみ）:**
```markdown
# 決定ログ

## 1. {決定内容}
- **背景**: {なぜ決定が必要だったか}
- **検討した選択肢**: {選択肢リスト}
- **理由**: {選んだ理由}
```

**実装完了前の自己チェック（必須）:**
ビルドとテストを実行する前に、以下を確認してください:
- 新しいパラメータ/フィールドを追加した場合、grep で呼び出し元から実際に渡されているか確認した
- `??`, `||`, `= defaultValue` を使った箇所で、フォールバックが本当に必要か確認した
- リファクタリングで置き換えたコード・エクスポートが残っていないか確認した
- タスク指示書にない機能を追加していないか確認した
- if/else で同一関数を呼び出し、引数の差異のみになっていないか確認した
- 新しいコードが既存の実装パターン（API呼び出し方式、型定義方式等）と一致しているか確認した
- wrapper / alias を追加しただけで互換 API を実装したことにしていないか確認した
- `ignore()` / `empty()` / `self` を返すだけの fallback を public API に露出していないか確認した
- no-op / placeholder のまま Pekko互換名を public にしていないか確認した
- `public API` と `internal implementation` の境界が悪化していないか確認した

**必須出力（見出しを含める）**
## 作業結果
- {実施内容の要約}
## 変更内容
- {変更内容の要約}
## ビルド結果
- {ビルド実行結果}
## テスト結果
- {テスト実行コマンドと結果}
## 実行結果
- {変更範囲 lint/型チェック と変更範囲テストの成功ログ}

## Fake Gap チェック（エビデンス必須）

各項目について以下 4 点を必ず出力する。エビデンスのない「なし」判定は無効で、レビュワーは REJECT する。

- **検出パターン**: 何を探したか（文言）
- **検索コマンド**: 実際に実行した grep / ripgrep コマンド（結果が再現できる形で）
- **実行結果**: マッチ件数とマッチ行の抜粋（0 件なら `0 件` と明記）
- **Pekko 参照との突合**: 該当 API について Pekko 側の Scala 実装（ファイル:行）を読み、契約が一致するかの判定

### 1. wrapper/alias 偽装

- 検出パターン: 新規公開メソッドが `self` / `self.existing_method(...)` / `self.map(|v| v)` のような既存メソッドへの単純委譲のみで終わっている
- 検索コマンド例: `rg -nB2 '^\s+self\.[a-z_]+\([^)]*\)\s*$' modules/{name}-core/src/core/dsl/`
- Pekko 突合: 該当 API の Pekko 参照を開き、セマンティクス（cancellation / timer / broadcast 等）が同一か確認する
- 判定出力例:
  ```
  ### 1. wrapper/alias 偽装
  検出パターン: self.existing_method(...) への単純委譲
  検索コマンド: rg -nB2 '^\s+self\.[a-z_]+\([^)]*\)\s*$' modules/stream-core/src/core/dsl/
  実行結果: 0 件
  Pekko 突合: 新規追加した `Flow::keep_alive` (flow.rs:2209) は KeepAliveLogic stage 経由で実装。Pekko `Flow.scala:3080` の maxIdle / injectedElem 契約と一致
  判定: なし
  ```

### 2. fallback / no-op 公開 API

- 検出パターン: 引数を捨てて `self` を返す / `let _ = ...; self` / 常に `Ok(self)` / 戻り値が `ignore()` や `empty()` に固定
- 検索コマンド例:
  - `rg -nB4 '^\s+self\s*$' modules/{name}-core/src/core/dsl/`
  - `rg -n 'let _ = .+;\s*$' modules/{name}-core/src/core/dsl/`
  - `rg -n '^\s+pub fn .*_\w+:' modules/{name}-core/src/core/dsl/` （公開 API の先頭 `_` 付き引数は no-op の強いシグナル）
- Pekko 突合: 該当 API の Pekko 仕様を開き、本実装がその意味を満たすかを行レベルで説明する
- 判定出力例:
  ```
  ### 2. fallback / no-op 公開 API
  検出パターン: let _ = sinks.into_iter().count(); self / 公開 API 引数の先頭 `_` 付き
  検索コマンド: rg -nB4 '^\s+self\s*$' modules/stream-core/src/core/dsl/
  実行結果: 0 件
  Pekko 突合: 新規 `Flow::also_to_all` (flow.rs:2854) は Broadcast(N+1) 合成を実行。Pekko `Flow.scala:3996` と同じ clone コスト
  判定: なし
  ```

### 3. public/internal 境界悪化

- 検出パターン: `impl/` 以下の内部型を `pub use` で再公開、`pub(crate)` → `pub` への昇格、`internal_*` / `*_inner` 型の露出
- 検索コマンド例:
  - `rg -n 'pub use .*impl::' modules/{name}-core/src/`
  - `git diff origin/main..HEAD -- modules/{name}-core/src/core/impl/ | rg '^\+pub '`
- 判定出力例:
  ```
  ### 3. public/internal 境界悪化
  検出パターン: impl 層の型を pub use で露出
  検索コマンド: rg -n 'pub use .*impl::' modules/stream-core/src/
  実行結果: 0 件（既存の KeepAliveLogic / SwitchMapLogic は pub(in crate::core) のまま）
  判定: なし
  ```

### あり と判定した場合

該当箇所の修正を本 implement ステップ内で完了させてから完了報告する。修正せずに `あり` のまま完了報告してはならない。既知スタブで本バッチ範囲外のものは plan ステップの `既知スタブ対応` に戻して繰り越す旨を明記する。
