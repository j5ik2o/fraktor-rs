## Phase 1: 参照確認と影響範囲調査

- [x] 1.1 Pekko `Dispatchers.scala:146,159-198` (MaxDispatcherAliasDepth + lookupConfigurator) を読み、alias chain 契約の行単位対応を `Read` で確認
- [x] 1.2 既存 `dispatchers.rs::normalize_dispatcher_id` の実装 (L192-199) を確認し、Pekko id → `default` マッピングの 2 エントリを alias 登録で再現できることを検証
- [x] 1.3 `rtk grep "normalize_dispatcher_id" --glob "*.rs"` で外部参照を列挙。参照があれば migration 方針を確定 (public API 削除か wrapper 保持か)
- [x] 1.4 `rtk grep "DispatchersError" --glob "*.rs"` で既存 error 利用箇所を列挙。新 variant 追加時に match が非網羅になるコードがあれば把握
- [x] 1.5 既存 `dispatchers/tests.rs` を読み、`normalize_dispatcher_id` に依存するテスト / 既存 resolve 経路を依存しているテストを列挙

## Phase 2: DispatchersError variant の追加

- [x] 2.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers_error.rs` に variant を追加:
  - `AliasChainTooDeep { start: String, depth: usize }`
  - `AliasConflictsWithEntry(String)`
- [x] 2.2 `Display` impl に対応 arm を追加 (英語、既存 variant と統一):
  - `AliasChainTooDeep { start, depth }` → `"alias chain starting at \`{start}\` exceeded max depth {depth} (possible cycle or excessive aliasing)"`
  - `AliasConflictsWithEntry(id)` → `"id \`{id}\` is registered as both an alias and an entry"`
- [x] 2.3 既存テストと match 網羅性確認のみで移行可能な形に調整

## Phase 3: Dispatchers alias chain 実装

- [x] 3.1 `Dispatchers` struct に `aliases: HashMap<String, String, RandomState>` フィールドを追加。`Default` / `new` / `Clone` impl を追随更新
- [x] 3.2 `Dispatchers::MAX_ALIAS_DEPTH: usize = 20` const を pub で追加。rustdoc に Pekko `Dispatchers.scala:146` 対応を明記 (英語)
- [x] 3.3 `register_alias(&mut self, alias, target)` メソッドを追加:
  - `self.entries.contains_key(&alias)` → `Err(AliasConflictsWithEntry(alias))`
  - `self.aliases.entry(alias)` の `Occupied` → `Err(Duplicate(alias))`
  - `Vacant` → insert、`Ok(())`
- [x] 3.4 `register()` に衝突チェックを追加:
  - `self.aliases.contains_key(&id)` → `Err(AliasConflictsWithEntry(id))` を先頭で判定
  - 既存 Occupied / Vacant 分岐はそのまま
- [x] 3.4.1 `register_or_update()` は last-writer-wins セマンティクスに変更:
  - 戻り値は引き続き `()` (builder API `ActorSystemConfig::with_dispatcher_configurator` の infallible 合成のため)
  - `self.aliases.remove(&id)` で既存 alias を wipe してから `self.entries.insert(id, configurator)`
  - rustdoc で semantics を明示
- [x] 3.5 `follow_alias_chain(&self, id: &str) -> Result<String, DispatchersError>` private helper を追加 (design Decision 6 の for loop 実装)
- [x] 3.6 `resolve(&self, id)` を書き換え:
  - 冒頭で `self.resolve_count.fetch_add(1, Ordering::Relaxed)` (既存、位置そのまま)
  - `let resolved = self.follow_alias_chain(id)?;` を alias 解決段として追加
  - `self.entries.get(&resolved).map(...).ok_or_else(|| DispatchersError::Unknown(resolved))` で lookup
  - 既存 `normalize_dispatcher_id` 呼び出しは削除

## Phase 4: normalize_dispatcher_id の削除と ensure_default_inline の alias 登録

- [x] 4.1 `Dispatchers::normalize_dispatcher_id()` 関数と関連 const (`PEKKO_DEFAULT_DISPATCHER_ID` / `PEKKO_INTERNAL_DISPATCHER_ID`) を削除。const は private だったので API 影響なし (要確認)
- [x] 4.2 `ensure_default()` / `ensure_default_inline()` / `replace_default_inline()` 内で、`default` entry 登録直後に alias 2 件を追加登録:
  - `register_alias("pekko.actor.default-dispatcher", DEFAULT_DISPATCHER_ID)`
  - `register_alias("pekko.actor.internal-dispatcher", DEFAULT_DISPATCHER_ID)`
  - 既に登録済の場合 (`Duplicate`) は silently 無視 (`let _ = ...;` + 無視理由コメント) または `Entry::Vacant` 経由で重複を自然に回避するヘルパー
  - 実装方針: `ensure_default` は idempotent であるべきなので、重複エラーを無視するのが自然。`let _ = self.register_alias(...); // idempotent: duplicate is expected on repeat calls` のコメント付きで OK (CLAUDE.md `ignored-return-values` ルールに従い comment で rationale を明示)
- [x] 4.3 `replace_default_inline` では既存 blocking dispatcher alias の挙動を維持する (既存 `ArcShared::ptr_eq` による判定はそのまま、本 change のスコープ外)

## Phase 5: テスト追加

- [x] 5.1 `dispatchers/tests.rs` を確認し、`normalize_dispatcher_id` 直接呼び出しテストがあれば `resolve("pekko.actor.default-dispatcher")` 経由テストに書き換え
- [x] 5.2 spec Scenario 9 件に 1:1 対応するテストを `dispatchers/tests.rs` に追加:
  - 1 段 alias 解決
  - 多段 alias chain (3 段程度で OK)
  - MAX_ALIAS_DEPTH 超 → `AliasChainTooDeep`
  - cycle → `AliasChainTooDeep` (depth over として)
  - alias to missing target → `Unknown(target)`
  - `register(id)` + pre-registered `register_alias(id)` → `AliasConflictsWithEntry`
  - `register_alias(id)` + pre-registered `register(id)` → `AliasConflictsWithEntry`
  - alias の重複 → `Duplicate`
  - `ensure_default_inline` 後の Pekko id resolve
- [x] 5.3 既存 resolve call-frequency contract テストが通ることを確認 (alias chain 1 回 resolve で `resolve_call_count` が 1 増えること、depth 超過や Unknown でも 1 増えること)

## Phase 6: CI 検証

- [x] 6.1 `rtk cargo test -p fraktor-actor-core-rs --lib` で alias chain + 既存 regression がすべて通ること
- [x] 6.2 `rtk cargo test -p fraktor-actor-core-rs --tests` でインテグレーションテスト pass 確認
- [x] 6.3 `./scripts/ci-check.sh ai all` を実行し exit 0 を確認
- [x] 6.4 clippy / rustdoc / dylint (type-per-file / module-wiring / ambiguous-suffix 等) で新規警告ゼロ

## Phase 7: gap-analysis 更新

- [x] 7.1 `docs/gap-analysis/actor-gap-analysis.md` のサマリーテーブルに第18版 entry を追加:
  - `内部セマンティクスギャップ数 (第18版、AC-M2 完了反映後)` — `3+（high 0 / medium 3 / low 約 11）`
  - 残存 medium: `AC-M4b (deferred), FS-M1, FS-M2`
- [x] 7.2 AC-M2 行を done 化:
  - `✅ **完了 (change `pekko-dispatcher-alias-chain`)** — alias chain resolution (MAX_ALIAS_DEPTH = 20 Pekko parity)` プレフィックス
  - `備考` カラムに HOCON dynamic loading を scope outside (n/a rationale) と明記
  - 最終列を `~~medium~~ done` に
- [x] 7.3 Phase A3 セクションの「完了済み」リストに AC-M2 を追加
- [x] 7.4 Phase A3 セクションの「残存 medium 4 件」を「残存 medium 3 件: AC-M4b (deferred), FS-M1, FS-M2」に更新
- [x] 7.5 第17版時点の履歴記述末尾に第18版の追記を追加 ("HOCON dynamic loading は JVM reflection 依存のため対象外、alias chain のみ parity 達成" を明記)
- [x] 7.6 対象外 (`n/a`) セクションに **HOCON-based dispatcher dynamic loading** を追加:
  - `com.typesafe.config` / `ConfigFactory` / `DynamicAccess` 依存の型動的生成
  - fraktor-rs は typed `Dispatchers::register()` API で等価な責務を果たす

## Phase 8: PR 発行とレビュー対応

- [ ] 8.1 branch `impl/pekko-dispatcher-alias-chain` を切って PR 発行、base は main
- [ ] 8.2 PR 本文に以下を含める:
  - Pekko `Dispatchers.scala:146,159-198` との対応表
  - **公開 API 追加** (additive):
    - `Dispatchers::register_alias(alias, target)`
    - `Dispatchers::MAX_ALIAS_DEPTH: usize = 20`
    - `DispatchersError::AliasChainTooDeep { start, depth }` / `AliasConflictsWithEntry(id)`
  - **公開 API 削除**:
    - `Dispatchers::normalize_dispatcher_id()` (代替: alias 自動登録)
  - **挙動変更**: `resolve()` は lookup 前に alias chain を 0〜20 段辿る。既存コードで alias を登録していない場合は挙動不変
  - **scope outside**: HOCON dynamic loading (`type = "..."` 文字列からの動的 instantiation)、`DynamicAccess` reflection — いずれも JVM 依存で Rust 側は typed `register()` で等価な責務を提供
  - gap-analysis AC-M2 done 化、第18版 medium 4 → 3
- [ ] 8.3 レビュー対応: CodeRabbit / Cursor Bugbot の指摘が来た場合は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve
- [ ] 8.4 マージ後、別 PR で change をアーカイブ + main spec を `openspec/specs/pekko-dispatcher-alias-chain/spec.md` に sync
