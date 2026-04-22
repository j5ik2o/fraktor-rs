## 1. 事前確認

- [x] 1.1 ベースライン記録: `cargo test --workspace` および `cargo test --workspace --features test-support` 両方 pass を確認
- [x] 1.2 `Grep "feature = \"test-support\"" modules/actor-core/src/` で 0 件であることを確認 (step05 完了状態の検証)
- [x] 1.3 `Grep 'fraktor-actor-core-rs.*test-support' --include='Cargo.toml' modules/ showcases/` で全 8 ファイル列挙し、design.md の表と一致することを確認

## 2. Phase 1 — 下流 dev-dep の `actor-core features=["test-support"]` 削除 (8 ファイル)

> redundant な dev-dep 行ごと削除する (Decision 3)。例外: `showcases/std/Cargo.toml` は `[dependencies]` のため行は残し `features = ["test-support"]` のみ削除。

- [x] 2.1 `modules/actor-adaptor-std/Cargo.toml`: dev-dep `fraktor-actor-core-rs = { workspace = true, features = ["test-support"] }` 行ごと削除（前後の説明コメントも整合させる）
- [x] 2.2 `modules/cluster-core/Cargo.toml`: dev-dep 行ごと削除
- [x] 2.3 `modules/cluster-adaptor-std/Cargo.toml`: dev-dep 行ごと削除
- [x] 2.4 `modules/persistence-core/Cargo.toml`: dev-dep 行ごと削除
- [x] 2.5 `modules/remote-adaptor-std/Cargo.toml`: dev-dep 行ごと削除
- [x] 2.6 `modules/stream-core/Cargo.toml`: dev-dep 行ごと削除
- [x] 2.7 `modules/stream-adaptor-std/Cargo.toml`: dev-dep 行ごと削除
- [x] 2.8 `showcases/std/Cargo.toml`: prod dep の `features = ["test-support"]` のみ削除（行は残す）
- [x] 2.9 各 crate ごとに `cargo test -p <crate>` で pass 確認

## 3. Phase 2 — 下流 crate の `test-support` feature 定義 forward 削除 (4 ファイル)

> Decision 2 に従い、`"fraktor-actor-core-rs/test-support"` の forward を削除する。feature 自体は残す。

- [x] 3.1 `modules/actor-adaptor-std/Cargo.toml:17`: `test-support = ["fraktor-actor-core-rs/test-support"]` → `test-support = []`
- [x] 3.2 `modules/cluster-core/Cargo.toml:17`: `test-support = ["fraktor-actor-core-rs/test-support"]` → `test-support = []`
- [x] 3.3 `modules/cluster-adaptor-std/Cargo.toml:18`: `test-support = ["fraktor-cluster-core-rs/test-support", "fraktor-actor-core-rs/test-support"]` → `test-support = ["fraktor-cluster-core-rs/test-support"]`
- [x] 3.4 `modules/remote-adaptor-std/Cargo.toml:17`: `test-support = ["fraktor-actor-core-rs/test-support"]` → `test-support = []`
- [x] 3.5 `cargo test --workspace` で pass 確認 (この段階で actor-core 側はまだ `test-support = []` を持つので動く)

## 4. Phase 3 — actor-core 本体から削除

- [x] 4.1 `modules/actor-core/Cargo.toml:19` の `test-support = []` 行を削除
- [x] 4.2 同 Cargo.toml 内の 8 個の `[[test]] required-features = ["test-support"]` 行を削除（`[[test]]` 本体は残す）
- [x] 4.3 `cargo test -p fraktor-actor-core-rs` で pass 確認
- [x] 4.4 `cargo test --workspace` で pass 確認

## 5. Phase 4 — 全体検証

- [x] 5.1 `Grep "^test-support" modules/actor-core/Cargo.toml` で 0 件確認
- [x] 5.2 `Grep 'required-features.*test-support' modules/actor-core/Cargo.toml` で 0 件確認
- [x] 5.3 `Grep 'fraktor-actor-core-rs.*features.*test-support' --include='Cargo.toml' modules/ showcases/` で 0 件確認
- [x] 5.4 `Grep '"fraktor-actor-core-rs/test-support"' --include='Cargo.toml' modules/ showcases/` で 0 件確認
- [x] 5.5 `cargo build --workspace --no-default-features` で pass 確認
- [x] 5.6 `cargo build --workspace --all-features` で pass 確認 (actor-core/test-support がないので `--all-features` が actor-core に余分な feature を要求しないことを確認)
- [x] 5.7 `cargo test --workspace` で pass 確認
- [x] 5.8 `cargo test -p fraktor-actor-core-rs --features test-support` は **エラーになる想定** (`the package 'fraktor-actor-core-rs' does not contain this feature: test-support`) — 確認のみ。これは期待動作。なお `cargo test --workspace --features test-support` は他 crate (actor-adaptor-std, cluster-core, cluster-adaptor-std, remote-adaptor-std) に空の `test-support` feature が残るため通る
- [x] 5.9 `./scripts/ci-check.sh dylint` で lint pass
- [x] 5.10 `./scripts/ci-check.sh ai all` で全 CI 緑

## 6. spec / docs 整合

- [x] 6.1 `openspec validate step06-remove-actor-core-test-support-feature --strict` で artifact 整合確認
- [x] 6.2 `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 1 全体を「解消済み」に更新（責務 A/B/C + feature 削除すべて完了）
- [x] 6.3 step07 / step08 proposal の前提が変わらないことを確認（test-support 関連の言及があれば「解消済み」へ）

## 7. コミット・PR

> 本 change は **artifacts PR** (proposal/design/specs/tasks のみ) と **implementation PR** (本セクション) を分けて運用するか、変更が小さいため同一 PR で進めるかは判断する。design.md / spec の delta が小さいため同一 PR でも可。

- [ ] 7.1 ブランチ作成: `step06-remove-actor-core-test-support-feature` (artifacts と impl を同 PR にする場合) または artifacts と `-impl` を分ける
- [ ] 7.2 論理単位での commit (Phase 1 を crate ごとに分けるか、一括にするかは判断)
- [ ] 7.3 push + PR 作成（base: main、title prefix `chore(actor-core):` または `refactor(actor-core):`）
- [ ] 7.4 CI 全 pass + レビュー対応 + マージ
- [ ] 7.5 archive (`/opsx:archive` または skill 経由)
