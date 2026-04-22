## Context

step06 archive 後 (2026-04-22) の workspace 状態:

```
$ rg "^test-support" --glob "Cargo.toml"
modules/cluster-core/Cargo.toml:17:test-support = []
modules/remote-adaptor-std/Cargo.toml:17:test-support = []
modules/cluster-adaptor-std/Cargo.toml:18:test-support = ["fraktor-cluster-core-rs/test-support"]
modules/actor-adaptor-std/Cargo.toml:17:test-support = []
```

4 つの test-support feature が残存。actor-adaptor-std だけが実用 (公開ゲート 4 箇所)、他 3 つは src 内で 1 箇所も使われていない dead code。

step06 の Non-Goals で「他 crate の test-support は独自責務がある可能性」として保留したが、archive 後の調査で **実際には独自責務がなかった** と確定したため、本 change で退役する。

### 全数棚卸し (step09 開始時点)

| カテゴリ | 場所 | 件数 | 内容 |
|---|---|---|---|
| ① 削除する feature 定義 | cluster-core (17), cluster-adaptor-std (18), remote-adaptor-std (17) | 3 | 全て空 or forward のみ |
| ② 削除する dep `features = ["test-support"]` | showcases/std (20, 21), cluster-adaptor-std (37) | 3 | 削除対象 feature への参照 |
| ③ 保持する actor-adaptor-std/test-support | actor-adaptor-std (17, 49, 54) | 1 feature + 2 required-features | 実用ゲート 4 箇所 |

### 重要な制約

- **actor-adaptor-std/test-support は touch しない**: `tick_driver.rs:4,13` (TestTickDriver re-export)、`std.rs:11`、`circuit_breakers_registry_id.rs:7` で実用。`required-features = ["test-support"]` も同 crate 内 [[test]] にあり、これらと整合
- **dev-cycle workaround との相互作用なし**: actor-core の `tests/*.rs` integration test は `actor-adaptor-std/test-support` 経由で TestTickDriver を取るが、cluster-core / cluster-adaptor-std / remote-adaptor-std の test-support は経路に登場しない
- **showcases/std の cluster-* dep は `optional = true`**: `advanced` feature 経由でのみ activate される。test-support 削除後も `optional = true` のまま、`features = ["test-support"]` だけ削除する

## Goals / Non-Goals

**Goals:**

- cluster-core / cluster-adaptor-std / remote-adaptor-std の dead `test-support` feature 退役
- 上記を参照する 3 つの dep entry の `features = ["test-support"]` 削除
- workspace 全体ビルドおよびテストが本 change の前後で同じ pass 率を維持
- `actor-test-driver-placement` capability に「下流 crate の test-support feature は実用ゲートを持つ場合のみ存在してよい」Scenario を追加 (MODIFIED)

**Non-Goals:**

- `actor-adaptor-std/test-support` の見直し (別 change)
- 他の dead feature の一掃 (test-support 以外、別スコープ)
- spec に「dead feature 一般禁止」のような広範ルールの追加 (本 change スコープ外、必要なら別 change)

## Decisions

### Decision 1: cluster-adaptor-std/test-support は forward 削除でなく定義ごと削除

```toml
# Before
test-support = ["fraktor-cluster-core-rs/test-support"]

# After (削除)
```

cluster-adaptor-std/src には `feature = "test-support"` ゲートが 0 件。forward 先 (cluster-core/test-support) も同時に消えるため、forward を `[]` に変えても意味がない。**定義ごと削除**する。

### Decision 2: showcases/std の cluster-* dep は features 指定だけ削除

```toml
# Before
fraktor-cluster-core-rs = { workspace = true, features = ["test-support"], optional = true }
fraktor-cluster-adaptor-std-rs = { workspace = true, features = ["test-support"], optional = true }

# After
fraktor-cluster-core-rs = { workspace = true, optional = true }
fraktor-cluster-adaptor-std-rs = { workspace = true, optional = true }
```

これらは `[dependencies]` (prod dep) に書かれており、`optional = true` で `advanced` feature 経由 activate される設計。**行ごとは消さず**、`features = ["test-support"]` のみ除去する。step06 の showcases/std と同じパターン (Decision 3 例外と整合)。

### Decision 3: cluster-adaptor-std の dev-dep は行ごと削除

```toml
# Before [dev-dependencies]
fraktor-cluster-core-rs = { workspace = true, features = ["test-support"] }

# After
(削除)
```

`features = ["test-support"]` を削除すると `{ workspace = true }` になり、prod dep `fraktor-cluster-core-rs = { workspace = true }` (line 21) と完全に同等になる。step06 の Decision 3 と同じ理由で **dev-dep 行ごと削除**。

### Decision 4: spec delta は MODIFIED でカバー

`actor-test-driver-placement` capability の既存 Requirement「actor-core では feature ゲート経由で内部 API の可視性を拡大してはならない」(step05/06 で追加・更新) に Scenario を追加 (MODIFIED):

> #### Scenario: 下流 crate の test-support feature は実用ゲートを持つ場合のみ存在してよい
> - **WHEN** 下流 crate の `Cargo.toml` に `test-support = [...]` feature 定義が存在する
> - **THEN** 当該 crate の `src/**/*.rs` に `#[cfg(feature = "test-support")]` または `#[cfg(all(test, feature = "test-support"))]` のような実用ゲートが少なくとも 1 件存在する
> - **AND** 「forward only (`test-support = ["other_crate/test-support"]`) で自身の src は使っていない」状態は許されない
> - **AND** 「空定義 (`test-support = []`) で自身の src は使っていない」状態も許されない

これにより「step09 で消した dead test-support の再侵入」が spec で機械的に検出可能になる。

## Risks / Trade-offs

- **[Risk] 下流 crate の test-support feature を消した後で第三者が必要だと気付く**:
  - Mitigation: 本 change の調査で 0 cfg gates と確認済み。万一必要になれば feature を復活させる差分は trivial (1 行追加)
- **[Trade-off] cluster-adaptor-std の dev-dep 行削除でテスト挙動が変わる懸念**:
  - 確認: dev-dep `fraktor-cluster-core-rs = { workspace = true, features = ["test-support"] }` を削除しても prod dep `fraktor-cluster-core-rs = { workspace = true }` (line 21) が同 crate を提供する。test-support feature が消えるが、それを使っている src 箇所がないので影響なし
- **[Risk] showcases/std の `advanced` feature が cluster-* test-support に依存していた可能性**:
  - 確認: `features = ["test-support"]` は cluster-* 自身の (現在 dead な) test-support を要求しているだけで、`advanced` feature 経由 activate される flow には影響なし

## Migration Plan

1. **Phase 1: 下流 dep entry のクリーンアップ** (順序依存なし)
   - `showcases/std/Cargo.toml:20-21` の cluster-* dep から `features = ["test-support"]` 削除 (行は残す)
   - `cluster-adaptor-std/Cargo.toml:37` dev-dep 行ごと削除
   - 各 crate ごとに `cargo test -p <crate>` で pass 確認
2. **Phase 2: feature 定義削除**
   - `cluster-core/Cargo.toml:17`、`cluster-adaptor-std/Cargo.toml:18`、`remote-adaptor-std/Cargo.toml:17` の `test-support = ...` 行を削除
   - `cargo test --workspace` で pass 確認
3. **Phase 3: 全体検証**
   - `cargo test --workspace` 全 pass
   - `cargo build --workspace --no-default-features` pass
   - `cargo build --workspace --all-features` (actor-core 単独で全 pass、aws-ecs 関連 pre-existing error は無関係)
   - 4 つの crate それぞれで `cargo test -p <crate> --features test-support` がエラーになる想定 (期待動作)
   - `./scripts/ci-check.sh ai all` 全 pass
4. **Phase 4: spec delta apply**
   - `actor-test-driver-placement` capability に MODIFIED で Scenario 追加
   - `openspec validate --strict` pass
5. **Phase 5: ドキュメント更新**
   - `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 1 のセクションに「step09 で完全クローズ」を追記
6. **Phase 6: コミット + PR**
   - 論理単位での commit
   - PR 作成 → レビュー → マージ
7. **Phase 7: archive**

ロールバックは git revert で完結する。

## Open Questions

- 将来 cluster-core 等で test-support 用途が再発生した場合は、dead feature を残すよりも別 change で feature を再導入する方針 (現時点で需要が読めないため YAGNI)
- 「forward only / 空 feature 一般禁止」のような広いルールを spec に書くべきかは別 change で判断 (本 change は test-support のみ)
