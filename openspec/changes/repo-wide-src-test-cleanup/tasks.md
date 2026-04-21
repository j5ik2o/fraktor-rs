## 1. Inventory

- [ ] 1.1 `src/` 配下に残っている `#[cfg(test)] mod tests;` / `tests.rs` / std 依存 test を棚卸しし、crate ごとに一覧化する
- [ ] 1.2 各候補を「そのまま integration test へ移せる」「private helper の切り出しが必要」「当面 `src` に残す」の 3 区分で分類する
- [ ] 1.3 repo-wide `dead_code` に引っかかる test-only helper / type / method を棚卸しし、配置整理で解消できるものを対応付ける

## 2. Cleanup Batches

- [ ] 2.1 no_std-sensitive な crate から 1 batch 目を選び、std 依存 test を `tests/` 配下へ移す
- [ ] 2.2 移設に伴って必要になる fixture / helper を `tests/` 側へ切り出し、production 公開面を広げずに参照を通す
- [ ] 2.3 同 batch 内で不要になった test-only helper / method を削除し、runtime semantics が変わっていないことを確認する
- [ ] 2.4 次の batch を同じ手順で進め、棚卸し一覧を更新する

## 3. Verification

- [ ] 3.1 各 batch ごとに対象 crate の `cargo test` / `ci-check` を実行し、移設前後でテスト意味論が変わっていないことを確認する
- [ ] 3.2 `src/` 配下の std 依存 test が減っていることと、対象 batch で `dead_code` が縮退していることを確認する
- [ ] 3.3 未着手の候補と残存理由を記録し、次 batch へ持ち越す backlog を明示する
