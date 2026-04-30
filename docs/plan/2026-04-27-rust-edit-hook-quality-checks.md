# Rust 編集後フックの品質チェック拡張

更新日: 2026-04-27

## 目的

Rust ファイル編集後の自動フックが `dylint` のみを実行しており、`cargo fmt` の未実行や `clippy` エラーを後段で見落としやすい状態になっている。

この変更では、既存の `scripts/ci-check.sh` は改修せず、フック側の責務だけを見直して、Rust 編集直後に `fmt` / `dylint` / `clippy` を自動実行する。

あわせて、ファイル名やメッセージも `dylint` 専用前提から、Rust 品質チェック全体を表す名前へ整理する。

## 方針

1. `.agents/hooks/run_dylint_after_rust_edit.py` を、責務に即した新しい名前へ改名する。
2. フックが起動するコマンドを `./scripts/ci-check.sh ai fmt dylint clippy` に変更する。
3. docstring、失敗メッセージ、ロックファイル名、hook のステータスメッセージを `dylint` 専用表現から更新する。
4. `.claude/settings.json` と `.codex/config.toml` の参照先を新ファイル名へ追従させる。

## 変更しないもの

- `scripts/ci-check.sh` の既存コマンド定義
- Rust ファイル編集の検出ロジック
- `target/.ci-check.lock` / `target/.ci-check.coordination.lock` による排他制御方針

## 確認

実装後は、対象ファイルの整合確認に加えて、最後に `./scripts/ci-check.sh ai all` を実行して全体確認する。
