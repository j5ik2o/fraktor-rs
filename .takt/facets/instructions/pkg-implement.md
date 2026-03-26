# パッケージ構造の実装（移行）

Report Directory 内の **`plan.md`**・**`structure-design.md`** を一次情報として、**当該 Phase のみ** ファイル移動とモジュール宣言・import の更新を行う。

Piece Context に示された Report Directory 内のファイルのみ参照すること。他のレポートディレクトリは検索・参照しない。

## 原則

- **Phase をまたがない**（plan に書かれた Phase のみ）
- 各ステップのあと **コンパイル可能な状態** を維持する（可能な範囲で）
- **ロジック変更は最小**。パッケージ移行が主目的のとき、挙動変更は plan に明示されていない限り行わない
- fraktor-rs 規約: `core`/`std` 境界、1 ファイル 1 公開型、Dylint、`mod.rs` 不使用

## やること

1. `plan.md` の手順に従い、ファイル移動・`mod` 追加／削除・`pub use` を実施する
2. ワークスペース全体で **旧パス参照を grep** し、import を更新する
3. 公開 API を変える場合は、タスク／plan の意図に沿っているか確認する（無断で公開範囲を広げない）

## ビルド・テスト（必須）

**やらないこと:**

- `cargo` を直接実行しない

**必ず `./scripts/ci-check.sh` 経由:**

1. 変更したクレートに応じて Dylint を実行する（例: `./scripts/ci-check.sh ai dylint -m stream` またはタスク指定のモジュール）
2. `./scripts/ci-check.sh ai std`（またはタスク・plan で指定された範囲）

`ci-check.sh` を複数同時に実行しない。完了を待ってから次を実行する。

成功ログを `coder-decisions.md` の「実行結果」に記録する。

## 必須出力（見出しを含める）

## 作業結果

- {要約}

## 変更内容

- {ファイル移動・mod 変更の要約}

## ビルド結果

- {ci-check のコマンドと結果}

## テスト結果

- {同上}

**Scope / Decisions:** `refactoring-implement` と同様に、`coder-scope.md` / `coder-decisions.md` の契約に従う。

## ルール評価用

- 実装完了 → 「実装完了」
- 実装未着手（レポートのみ）→ 「実装未着手（レポートのみ）」
- 判断できない → 「判断できない、情報不足」
- ユーザー入力が必要 → 「ユーザー入力が必要」（この場合は plan へ戻るルールがある）
