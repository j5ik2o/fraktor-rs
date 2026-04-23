## Why

Pekko `Dispatchers.scala:159-198` `lookupConfigurator` は、dispatcher id の **alias chain resolution** を提供している: config 値が `STRING` なら別の id を指す alias として再帰 lookup (max depth `MaxDispatcherAliasDepth = 20`) し、`OBJECT` に到達した時点でそれを実 config として使う。これにより、ユーザーは `my-dispatcher = "pekko.actor.default-dispatcher"` のような文字列値で既存 dispatcher を参照できる。

fraktor-rs 現状 (`modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers.rs`):
- `Dispatchers::register()` / `register_or_update()` で静的に `(id, configurator)` を登録
- `resolve()` は `normalize_dispatcher_id` で hardcoded 2 要素 (`pekko.actor.default-dispatcher` / `pekko.actor.internal-dispatcher` → `default`) を特殊処理するのみ
- **任意の alias chain 連鎖解決は未実装**

gap-analysis 第17版時点で AC-M2 は残存 medium の 1 つとして特定されている。

## Scope decision: alias chain only, HOCON dynamic loading は n/a

Pekko `Dispatchers` の該当コード (`Dispatchers.scala:159-292`) には 2 つの責務が混在している:

1. **Alias chain resolution** (`lookupConfigurator` L159-198): config 値が `STRING` なら別 id への再帰 lookup
2. **HOCON-driven dynamic loading** (`configuratorFrom` L263-291): HOCON の `type = "Dispatcher" / "PinnedDispatcher" / <FQN>` を読んで `DynamicAccess` (JVM reflection) で configurator を動的生成

fraktor-rs は (2) を **n/a として明示的に対象外**とする:

- fraktor-rs は HOCON を採用していない (プロジェクトは no_std-first / typed builder API 指向)。HOCON パーサ追加は `modules/actor-core` のコア依存を肥大化させ、no_std 境界を侵食する
- Rust には JVM の `DynamicAccess` 相当の reflection が無い (trait object を動的 instantiate する機構が無い)
- 既存 `Dispatchers::register()` は typed configurator を直接受け取るため、HOCON + reflection よりも型安全で機能的に同等以上の責務を果たしている

したがって本 change の scope は **(1) alias chain resolution** に限定する。alias chain は fraktor-rs の typed API でも有用で、ユーザーは `register()` で実 configurator を登録したうえで、別 id から alias で参照できる。

## What Changes

- **新規メソッド** (`Dispatchers`):
  - `register_alias(alias: impl Into<String>, target: impl Into<String>) -> Result<(), DispatchersError>`: alias id → target id の登録
  - `MAX_ALIAS_DEPTH: usize = 20` (Pekko `MaxDispatcherAliasDepth` 同値 const)
- **新規 private メソッド**: `follow_alias_chain(&self, id: &str) -> Result<String, DispatchersError>` で depth 上限付きの再帰解決
- **`resolve()` の拡張**: lookup 前に `follow_alias_chain` を通す。既存 `normalize_dispatcher_id` の hardcoded 2 Pekko id の特殊処理は、`ensure_default_inline()` 内で alias 登録に移行する (resolve 経路を単一化)
- **`DispatchersError` に 2 variant 追加**:
  - `AliasChainTooDeep { start: String, depth: usize }`: Pekko `ConfigurationException("Didn't find ... after N aliases")` 相当
  - `AliasConflictsWithEntry(String)`: alias id と既存 entry id の衝突 (Pekko にはない concern だが、fraktor-rs は entry / alias を別 map で保持するため別途必要)
- **テスト**: `dispatchers/tests.rs` に alias chain 8 ケース追加 (1段 / 多段 / depth over / cycle / alias to missing / alias vs entry 衝突 / resolve_count 増分 / 既存 Pekko id 経路)
- **gap-analysis 更新**: 第18版として AC-M2 を done 化、HOCON dynamic loading 部分を n/a rationale 付きで明記、残存 medium を 3 件 (AC-M4b deferred, FS-M1, FS-M2) に更新

## Capabilities

### New Capabilities
- `pekko-dispatcher-alias-chain`: Pekko `Dispatchers.lookupConfigurator` の **alias chain resolution** (max depth 20) と等価な dispatcher id 解決機構を提供する契約。HOCON dynamic loading 部分は scope outside (JVM reflection 依存のため n/a 確定)。

### Modified Capabilities
<!-- 該当なし: 既存の dispatcher lookup capability は alias 概念を spec 化していない (内部実装詳細として扱われていた)。本 change で新規 capability として alias chain の契約を確立する。 -->

## Impact

**影響を受けるコード**:
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers.rs`:
  - `aliases: HashMap<String, String, RandomState>` フィールド追加
  - `register_alias()` / `follow_alias_chain()` メソッド追加
  - `resolve()` 内で alias 解決を lookup 前に追加
  - `ensure_default_inline()` / `replace_default_inline()` で 2 Pekko id を alias 登録に変更
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers_error.rs`:
  - `AliasChainTooDeep` / `AliasConflictsWithEntry` variant 追加 + Display 対応
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers/tests.rs`:
  - alias chain テスト 8 ケース追加
- `docs/gap-analysis/actor-gap-analysis.md`:
  - 第18版 entry 追加、AC-M2 done 化、サマリー表更新

**影響を受ける公開 API 契約**:
- `Dispatchers::register_alias(...)` 新設 (additive、破壊的変更なし)
- `DispatchersError` の variant 追加 (non-exhaustive enum のため additive)
- `Dispatchers::resolve()` 意味的拡張: 従来は `entries` 直接 lookup + hardcoded normalize のみだったが、alias chain を先に解決するようになる。既存呼び出しで alias が登録されていない場合は挙動不変 (既存テストすべて pass)

**HOCON / dynamic loading を対象外とする理由** (gap-analysis に明記):
- HOCON パーサ導入は no_std-first 方針と矛盾
- JVM reflection 相当の機構が Rust に無く、typed `register()` API が同等以上の責務を果たす
- Pekko で HOCON の `type = "..."` を使う場合、fraktor-rs では `Dispatchers::register(id, configurator)` を呼ぶことで等価な結果が得られる (ユーザーコード側で pure Rust コンストラクタを明示的に書く)

## Non-goals

- HOCON / `config` crate / reflection ベースの dynamic loading (scope decision 参照)
- `hasDispatcher(id)` 相当の API (現状 `resolve().is_ok()` で代替可能、必要性が生じたら別 change で追加)
- alias の **removal** (`unregister_alias`): alias は bootstrap 時のみ登録される想定。必要性が出たら別 change で検討
- Pekko `Dispatchers.config(id)` 相当の config 合成 (`idConfig(id).withFallback(...).withFallback(defaultDispatcherConfig)`): HOCON 前提の機能であり、fraktor-rs の typed `DispatcherConfig` では不要
