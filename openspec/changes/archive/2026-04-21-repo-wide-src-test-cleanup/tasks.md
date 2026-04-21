## 1. Inventory

- [x] 1.1 `src/` 配下に残っている `#[cfg(test)] mod tests;` / `tests.rs` / std 依存 test を棚卸しし、crate ごとに一覧化する
- [x] 1.2 各候補を「そのまま integration test へ移せる」「private helper の切り出しが必要」「当面 `src` に残す」の 3 区分で分類する
- [x] 1.3 repo-wide `dead_code` に引っかかる test-only helper / type / method を棚卸しし、配置整理で解消できるものを対応付ける

## 2. Cleanup Batches

- [x] 2.1 no_std-sensitive な crate から 1 batch 目を選び、std 依存 test を `tests/` 配下へ移す
- [x] 2.2 移設に伴って必要になる fixture / helper を `tests/` 側へ切り出し、production 公開面を広げずに参照を通す
- [x] 2.3 同 batch 内で不要になった test-only helper / method を削除し、runtime semantics が変わっていないことを確認する
- [x] 2.4 次の batch を同じ手順で進め、棚卸し一覧を更新する

## 3. Verification

- [x] 3.1 各 batch ごとに対象 crate の `cargo test` / `ci-check` を実行し、移設前後でテスト意味論が変わっていないことを確認する
- [x] 3.2 `src/` 配下の std 依存 test が減っていることと、対象 batch で `dead_code` が縮退していることを確認する
- [x] 3.3 未着手の候補と残存理由を記録し、次 batch へ持ち越す backlog を明示する

## 4. Remaining Cleanup

> **Note**: 4.1 / 4.2 は当初「helper-heavy な test module を tests/ へ移す」と記述していたが、spec.md の移設根拠（可視性汚染 / dead_code 誘発）との対応付けが不十分であることが判明した。`#[test]` が多い・helper が多いといった症状は (a) テスト対象モジュールの責務肥大（production 側リファクタ課題）と (b) 共通テストユーティリティ未整備（テストロジック課題）の混在であり、この change の Goal / Non-Goal では切り分けて処理できない。よって本 change では未対応のまま残し、後続 change で整理する。
>
> - 引き継ぎ先候補: 別 change `complex-test-modules-refactor`（仮）で (a) production 責務分割と (b) `tests/fixtures` への共通 utility 抽出を扱う
> - 本 change の archive 条件は 1〜3 の完了とし、4 は後続 change への引き継ぎで閉じる

- [-] 4.1 ~~`actor-core` の helper-heavy な test module（`system_state_shared`, `mailbox/base`, `backoff_supervisor`, `typed delivery`, `behaviors`, routing builders）を fixture 分離して `tests/` へ移す~~ → 別 change へ持ち越し
- [-] 4.2 ~~`stream-core` の helper-heavy な test module（`source`, `materialization/actor_materializer`）を public surface または `tests/fixtures` 経由へ整理して `tests/` へ移す~~ → 別 change へ持ち越し
- [x] 4.3 残存候補（4.1 / 4.2 対象）が本 change の Goal / Non-Goal では解けないことを確認し、後続 change に引き継いで本 change を archive 可能な状態にする
