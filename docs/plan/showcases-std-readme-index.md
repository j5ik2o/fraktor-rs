# showcases/std README インデックス追加計画

## 概要

`showcases/std` に、日本語のサンプルコードインデックス用 `README.md` を追加する。
既存の `showcases/std/Cargo.toml` の `[[example]]` 定義を正とし、各 example の目的、実行コマンド、完了/未完了状態が一覧で分かる構成にする。
概念名として `classic` は使わず、該当領域は `kernel` と表記する。

## 実装内容

- `showcases/std/README.md` を新規作成する。
- README は `fraktor-showcases-std` crate に集約された実行可能サンプルの一覧として記述する。
- 実行方法として、通常 example と `advanced` feature 必須 example のコマンドを分けて明記する。
- インデックス表は `状態 / サンプル / 対象領域 / 内容 / 実行コマンド` の列で構成する。
- 完了扱いは `showcases/std/Cargo.toml` に `[[example]]` として登録済み、かつ対応する `showcases/std/<name>/main.rs` が存在するものに限定する。
- `classic_logging` と `classic_timers` は example 名は既存のまま載せるが、説明・対象領域では `kernel` API のサンプルとして記述する。
- 未完了扱いは、現時点で example 未登録だが網羅観点として将来追加すべき候補を載せる。

## インデックス対象

完了として掲載する example は以下の 16 件にする。

- `getting_started`
- `request_reply`
- `state_management`
- `child_lifecycle`
- `timers`
- `routing`
- `stash`
- `serialization`
- `stream_pipeline`
- `stream_authoring_apis`
- `classic_logging`（対象領域は `kernel`）
- `classic_timers`（対象領域は `kernel`）
- `typed_event_stream`
- `typed_receptionist_router`
- `remote_lifecycle`
- `persistent_actor`

`remote_lifecycle` と `persistent_actor` は `advanced` feature 必須として、実行コマンドを `cargo run -p fraktor-showcases-std --features advanced --example <name>` にする。
それ以外は `cargo run -p fraktor-showcases-std --example <name>` にする。

未完了候補として、既存 README / OpenSpec で示唆されている `remote_messaging`、`cluster_membership`、`persistence_effector` を掲載する。

## 検証

- `rtk git diff -- showcases/std/README.md docs/plan/showcases-std-readme-index.md` で意図しない変更がないことを確認する。
- ソースコードは編集しないため、`./scripts/ci-check.sh ai all` は実行しない。
- Rust ソースや `Cargo.toml` を編集した場合のみ、最後に `rtk ./scripts/ci-check.sh ai all` を実行して完了を待つ。

## 前提

- `Cargo.toml` の `[[example]]` 定義を、現時点の完了済みサンプル一覧の唯一の正とする。
- README 追加のみを対象とし、サンプルコード本体や Cargo 設定は変更しない。
- `classic` は既存 example 名に残っている場合のみ文字列として扱い、README 上の概念説明では `kernel` を使う。
- `CHANGELOG.md` は編集しない。
