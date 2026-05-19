## 1. 事前確認

- [x] 1.1 `cargo test --workspace` ベースライン記録
- [x] 1.2 `Grep "feature = \"test-support\"" modules/cluster-core/src/ modules/cluster-adaptor-std/src/ modules/remote-adaptor-std/src/` で 0 件確認
- [x] 1.3 削除対象の dep entry 棚卸し: `Grep 'fraktor-cluster-core-rs.*test-support|fraktor-cluster-adaptor-std-rs.*test-support|fraktor-remote-adaptor-std-rs.*test-support' --include='Cargo.toml' modules/ showcases/` で 4 件 (showcases:20, 21、cluster-adaptor-std:18, 37) 確認

## 2. Phase 1 — 下流 dep entry のクリーンアップ

> Decision 2 と 3 に従う。showcases/std は行残し features 削除、cluster-adaptor-std dev-dep は行ごと削除。

- [x] 2.1 `showcases/std/Cargo.toml:20`: `fraktor-cluster-core-rs = { workspace = true, features = ["test-support"], optional = true }` → `fraktor-cluster-core-rs = { workspace = true, optional = true }`
- [x] 2.2 `showcases/std/Cargo.toml:21`: `fraktor-cluster-adaptor-std-rs = { workspace = true, features = ["test-support"], optional = true }` → `fraktor-cluster-adaptor-std-rs = { workspace = true, optional = true }`
- [x] 2.3 `modules/cluster-adaptor-std/Cargo.toml:37`: dev-dep `fraktor-cluster-core-rs = { workspace = true, features = ["test-support"] }` 行ごと削除 (prod dep と同等になるため)
- [x] 2.4 `cargo test -p fraktor-showcases-std` で pass 確認
- [x] 2.5 `cargo test -p fraktor-cluster-adaptor-std-rs` で pass 確認

## 3. Phase 2 — feature 定義削除

- [x] 3.1 `modules/cluster-core/Cargo.toml:17` `test-support = []` 行を削除
- [x] 3.2 `modules/cluster-adaptor-std/Cargo.toml:18` `test-support = ["fraktor-cluster-core-rs/test-support"]` 行を削除
- [x] 3.3 `modules/remote-adaptor-std/Cargo.toml:17` `test-support = []` 行を削除
- [x] 3.4 `cargo test --workspace` で pass 確認

## 4. Phase 3 — 全体検証

- [x] 4.1 `Grep "^test-support" --include='Cargo.toml' modules/` で `actor-adaptor-std/Cargo.toml:17` のみヒットすることを確認 (= dead 3 件削除済み)
- [x] 4.2 `Grep 'fraktor-cluster-core-rs.*test-support|fraktor-cluster-adaptor-std-rs.*test-support|fraktor-remote-adaptor-std-rs.*test-support' --include='Cargo.toml' modules/ showcases/` で 0 件確認
- [x] 4.3 `cargo build --workspace --no-default-features` pass 確認
- [x] 4.4 `cargo test --workspace` pass 確認 (本 change 前後で同件数)
- [x] 4.5 `cargo test -p fraktor-cluster-core-rs --features test-support` は **エラーになる想定** — 確認のみ。期待動作
- [x] 4.6 `cargo test -p fraktor-cluster-adaptor-std-rs --features test-support` は **エラーになる想定** — 確認のみ
- [x] 4.7 `cargo test -p fraktor-remote-adaptor-std-rs --features test-support` は **エラーになる想定** — 確認のみ
- [x] 4.8 `./scripts/ci-check.sh dylint` で lint pass
- [x] 4.9 `./scripts/ci-check.sh ai all` で全 CI 緑

## 5. spec / docs 整合

- [x] 5.1 `openspec validate step09-remove-dead-downstream-test-support-features --strict` で artifact 整合確認
- [x] 5.2 `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 1 セクションに「step09 で下流 3 crate の dead test-support も完全クローズ」と追記

## 6. コミット・PR

- [x] 6.1 ブランチ作成: `step09-remove-dead-downstream-test-support-features`
- [x] 6.2 論理単位での commit
- [x] 6.3 push + PR 作成 (base: main、title prefix `chore(actor-*):` または `refactor(actor-*):`)
- [x] 6.4 CI 全 pass + レビュー対応 + マージ
- [x] 6.5 archive
