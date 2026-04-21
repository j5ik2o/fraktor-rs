## 1. 事前調査と確認

- [x] 1.1 各クレート `[dev-dependencies]` 確認完了: `actor-adaptor-std:38`/`persistence-core:28` は `critical-section` 直接記述あり、`cluster-core:30`/`cluster-adaptor-std:36-39`/`remote-adaptor-std:37-40`/`stream-core:29`/`stream-adaptor-std:26` は `actor-core/test-support` 経由のみ
- [x] 1.2 `showcases/std/Cargo.toml:9-10, 19-20` で 4 クレートの `test-support` 有効化を確認
- [x] 1.3 `remote-core` の test-support 空定義の参照は `remote-adaptor-std/Cargo.toml:18, :37` のみ
- [x] 1.4 `actor-core/Cargo.toml:51-86` の 8 integration test の `required-features = ["test-support"]` は `test-support` feature 自体が残るため動作不変

## 2. ダウンストリームクレートの dev-dependencies 修正

- [x] 2.1 `cluster-core/Cargo.toml` の `[dev-dependencies]` に `critical-section = { workspace = true, features = ["std"] }` 追加
- [x] 2.2 `cluster-adaptor-std/Cargo.toml` の `[dev-dependencies]` に追加
- [x] 2.3 `remote-adaptor-std/Cargo.toml` の `[dev-dependencies]` に追加
- [x] 2.4 `stream-core/Cargo.toml` の `[dev-dependencies]` に追加
- [x] 2.5 `stream-adaptor-std/Cargo.toml` の `[dev-dependencies]` に追加
- [x] 2.6 `actor-adaptor-std`、`persistence-core` は既に直接記述済みのため変更不要を確認

## 3. showcases/std の修正

- [x] 3.1 `showcases/std/Cargo.toml` の `[dependencies]` に `critical-section = { workspace = true, features = ["std"] }` を追加

## 4. 中間ビルド検証（actor-core 修正前）

- [x] 4.1 5 ダウンストリームクレートで `cargo build` 成功（cluster-core/cluster-adaptor-std/remote-adaptor-std/stream-core/stream-adaptor-std）
- [x] 4.2 `cargo build` 全ターゲット成功

## 5. actor-core の整理

- [x] 5.1 `actor-core/Cargo.toml` の `test-support` を `[]` に変更（impl 関連削除）
- [x] 5.2 `actor-core/Cargo.toml` の `[dependencies]` から `critical-section` 行を完全削除
- [x] 5.3 `[dev-dependencies]` の `critical-section` は維持
- [x] 5.4 `cargo build -p fraktor-actor-core-rs --no-default-features` 成功
- [x] 5.5 `cargo test -p fraktor-actor-core-rs --features test-support` 成功（CI 全 pass で確認）
- [x] 5.6 `cargo tree --no-default-features --depth 1` で `critical-section` direct dep に出現せず（`portable-atomic v1.13.1` 経由 transitive のみ、dev-deps に `critical-section v1.2.0`）

## 6. remote-core の整理

- [x] 6.1 `remote-core/Cargo.toml` の `test-support = []` 行削除
- [x] 6.2 `remote-adaptor-std/Cargo.toml` の `test-support` 配列から `fraktor-remote-core-rs/test-support` 削除
- [x] 6.3 `remote-adaptor-std/Cargo.toml` の `fraktor-remote-core-rs` features 削除
- [x] 6.4 `cargo build` 成功

## 7. spec の整合確認

- [x] 7.1 `openspec validate retire-actor-core-test-support-critical-section-impl --strict` valid
- [x] 7.2 spec delta 視認確認、apply 時に既存 Requirement が MODIFIED 内容で置換される形

## 8. 全体 CI 確認

- [x] 8.1 `./scripts/ci-check.sh ai all` 成功（EXIT=0、全 pass）
- [x] 8.2 失敗なし
- [ ] 8.3 ユーザー指示によりコミット・PR 作成へ進行

## 9. ドキュメント更新

- [x] 9.1 `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 更新（責務 A 部分退役完了を記録）
