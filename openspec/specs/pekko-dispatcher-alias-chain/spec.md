# pekko-dispatcher-alias-chain Specification

## Purpose
Pekko `Dispatchers.lookupConfigurator` (`Dispatchers.scala:159-198`) の **alias chain resolution** (max depth `MaxDispatcherAliasDepth = 20`) と等価な dispatcher id 解決機構を fraktor-rs に提供する契約。config 値が `STRING` (= alias) なら別の id へ再帰 lookup し、`OBJECT` / entry に到達した時点でそれを使う仕様を、typed `Dispatchers::register(id, configurator)` + `register_alias(alias, target)` の 2 API で等価に表現する。HOCON `type = "..."` 文字列ベースの **dynamic loading** (`Dispatchers.configuratorFrom` L263-291) は JVM `DynamicAccess` reflection 依存のため `n/a` として scope 確定し、typed `register` API で等価責務を果たす。change `pekko-dispatcher-alias-chain` (2026-04-23 archive, PR #1644) で確立。

**残存 divergence**: AC-M2 の本 change は alias chain 契約のみを確立し、fraktor-rs 独自の primary entry id (`"default"`) は Pekko 原典 (`"pekko.actor.default-dispatcher"`) に合わせていない。DP-M1 として gap-analysis 第18版に登録済、別 change `pekko-dispatcher-primary-id-alignment` で対応予定。

## Requirements
### Requirement: `Dispatchers` は alias chain を最大 20 深さで解決する

fraktor-rs は `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers.rs` の `Dispatchers` に alias chain resolution 機構を提供し、以下の契約をすべて満たさなければならない (MUST):

- `Dispatchers::register_alias(alias, target) -> Result<(), DispatchersError>` で、`alias` id が `target` id を指す間接参照を登録する。
- `Dispatchers::MAX_ALIAS_DEPTH: usize = 20` (Pekko `Dispatchers.MaxDispatcherAliasDepth` (`Dispatchers.scala:146`) と同値) を定数として公開する。
- `Dispatchers::resolve(id)` は、lookup の **前に** alias chain を最大 `MAX_ALIAS_DEPTH` 段まで辿る:
  - 辿った id が `aliases` に存在する間、次の target へ進む
  - `aliases` に無くなった時点で止まり、その id を `entries` から lookup する
  - 辿った段数が `MAX_ALIAS_DEPTH` を超えた場合は `Err(DispatchersError::AliasChainTooDeep { start, depth: MAX_ALIAS_DEPTH })` を返す (Pekko `ConfigurationException("Didn't find a concrete dispatcher config after following N aliases, is there a loop in your config? ...")` 相当)
- alias と entry が同 id で登録されることは許さない (MUST NOT):
  - `register(id, configurator)` 呼び出し時、`id` が既に `aliases` にある場合は `Err(DispatchersError::AliasConflictsWithEntry(id))`
  - `register_alias(alias, target)` 呼び出し時、`alias` が既に `entries` にある場合は `Err(DispatchersError::AliasConflictsWithEntry(alias))`
- ただし `register_or_update(id, configurator)` は last-writer-wins セマンティクスを取り、既存 alias が同 id で存在した場合は **黙って除去**してから entry を挿入する (builder API である `ActorSystemConfig::with_dispatcher_configurator` の infallible 合成要件のため)。この場合、以前 alias が指していた target id 側の entry は触らない
- alias の重複登録は `Err(DispatchersError::Duplicate(alias))` を返す (既存 `register` の Duplicate 判定と同形)。
- alias が **存在しない target** を指していた場合、resolve は alias を最後まで辿った後で `entries.get(last_target)` を試み、それが無ければ `Err(DispatchersError::Unknown(last_target))` を返す (alias の存在そのものは error にしない。target 不在の error は alias を辿り終わった後の lookup 段で現れる)。
- `resolve()` の各呼び出しは、alias chain の段数に関わらず `resolve_count` カウンタを **ちょうど 1 増やす** (既存 call-frequency contract と整合)。
- `Dispatchers::normalize_dispatcher_id()` 関数は **削除される**。既存の 2 Pekko id 特殊処理 (`pekko.actor.default-dispatcher` / `pekko.actor.internal-dispatcher` → `default`) は、`ensure_default_inline()` / `replace_default_inline()` / `ensure_default()` 内で alias 自動登録に移行する (`register_alias` 経由で重複時は silently 無視)。
- 同時に typed 層 (`modules/actor-core/src/core/typed/dispatchers.rs`) の `normalize_dispatcher_id` も **削除される**。typed `Dispatchers::lookup(DispatcherSelector::FromConfig(id))` は kernel `resolve(id)` に id を verbatim で渡し、alias chain 解決を single source of truth とする。これにより `register_or_update("pekko.actor.default-dispatcher", custom)` のようなユーザーによる Pekko id 上書きが typed 経路でも正しく反映される (kernel alias は register_or_update 時に wipe される)。

#### Scenario: 1 段の alias 解決は target entry を返す

- **GIVEN** `Dispatchers::new()` に `entry("default", configurator_X)` が register 済、続いて `register_alias("app.work", "default")` が成功
- **WHEN** `resolve("app.work")` を呼ぶ
- **THEN** `configurator_X` 由来の `Ok(MessageDispatcherShared)` が返る
- **AND** `resolve_call_count()` がちょうど 1 増える

#### Scenario: 多段 alias chain (MAX_ALIAS_DEPTH 以内) は末尾の entry を返す

- **GIVEN** entry `A` が登録済、`register_alias("B", "A")`, `register_alias("C", "B")`, `register_alias("D", "C")` がすべて成功
- **WHEN** `resolve("D")` を呼ぶ
- **THEN** `A` の configurator 由来の `Ok(MessageDispatcherShared)` が返る

#### Scenario: MAX_ALIAS_DEPTH ちょうどの alias chain は成功で解決される (境界値)

- **GIVEN** entry `alias_{MAX_ALIAS_DEPTH}` が登録済、`register_alias("alias_0", "alias_1")`, ..., `register_alias("alias_{MAX_ALIAS_DEPTH - 1}", "alias_{MAX_ALIAS_DEPTH}")` の 20 段 alias chain が成功
- **WHEN** `resolve("alias_0")` を呼ぶ
- **THEN** `alias_{MAX_ALIAS_DEPTH}` entry の `Ok(MessageDispatcherShared)` が返る (off-by-one を防ぐ境界値契約)

#### Scenario: alias chain が MAX_ALIAS_DEPTH を超えたら AliasChainTooDeep

- **GIVEN** `Dispatchers` に alias を 21 段連鎖登録: `alias_0 → alias_1 → ... → alias_21`、かつ `alias_21` は entry として登録されていない
- **WHEN** `resolve("alias_0")` を呼ぶ
- **THEN** `Err(DispatchersError::AliasChainTooDeep { start: "alias_0", depth: 20 })` が返る

#### Scenario: alias の cycle は AliasChainTooDeep として検知される

- **GIVEN** `register_alias("A", "B")` と `register_alias("B", "A")` がともに成功
- **WHEN** `resolve("A")` を呼ぶ
- **THEN** `Err(DispatchersError::AliasChainTooDeep { start: "A", depth: 20 })` が返る (cycle は明示的に検知せず depth over として扱う、Pekko `Dispatchers.scala:160-163` と同仕様)

#### Scenario: 存在しない target を指す alias は末尾 target で Unknown

- **GIVEN** `register_alias("work", "missing-dispatcher")` が成功、`missing-dispatcher` は entry として登録されていない
- **WHEN** `resolve("work")` を呼ぶ
- **THEN** `Err(DispatchersError::Unknown("missing-dispatcher"))` が返る (alias 自体の存在は error にならず、entry lookup 段で初めて Unknown が現れる)

#### Scenario: register 時に alias と entry の id 衝突を拒否する

- **GIVEN** `register_alias("foo", "default")` が成功済
- **WHEN** `register("foo", configurator)` を呼ぶ
- **THEN** `Err(DispatchersError::AliasConflictsWithEntry("foo"))` が返る
- **AND** 既存の alias entry は保持される

#### Scenario: register_or_update は既存 alias を黙って除去して entry を挿入する

- **GIVEN** `ensure_default_inline()` 実行後、`pekko.actor.default-dispatcher` alias が登録済
- **WHEN** `register_or_update("pekko.actor.default-dispatcher", custom_configurator)` を呼ぶ
- **THEN** 呼び出しは成功する (戻り値 unit)
- **AND** `resolve("pekko.actor.default-dispatcher")` は `custom_configurator` 由来の `Ok(MessageDispatcherShared)` を返す (alias が wipe され、entry が代わりに存在するため)
- **AND** `resolve("default")` は既存の default entry を返す (Pekko alias が指していた target 側は無影響)

#### Scenario: register_alias 時に entry と alias の id 衝突を拒否する

- **GIVEN** `register("foo", configurator)` が成功済
- **WHEN** `register_alias("foo", "default")` を呼ぶ
- **THEN** `Err(DispatchersError::AliasConflictsWithEntry("foo"))` が返る
- **AND** 既存の entry は保持される

#### Scenario: alias の重複登録は Duplicate を返す

- **GIVEN** `register_alias("foo", "default")` が成功済
- **WHEN** 再度 `register_alias("foo", "other")` を呼ぶ
- **THEN** `Err(DispatchersError::Duplicate("foo"))` が返る
- **AND** 既存の alias (`"foo" → "default"`) は保持される

#### Scenario: 既存 Pekko id 経路は ensure_default_inline 内部の alias で解決される

- **GIVEN** `Dispatchers::new()` に対して `ensure_default_inline()` を呼んだ直後
- **WHEN** `resolve("pekko.actor.default-dispatcher")` を呼ぶ
- **THEN** `default` entry の configurator 由来の `Ok(MessageDispatcherShared)` が返る (従来 `normalize_dispatcher_id` が担っていた経路と同一結果)
- **AND** `resolve("pekko.actor.internal-dispatcher")` も同様に `default` を返す

#### Scenario: typed Dispatchers facade は kernel alias chain を single source of truth として尊重する

- **GIVEN** `ActorSystemConfig::with_dispatcher_configurator("pekko.actor.default-dispatcher", custom_configurator)` で ActorSystem を構築 (register_or_update により `pekko.actor.default-dispatcher` の alias は wipe され、custom entry に置き換わる)
- **WHEN** typed `Dispatchers::lookup(DispatcherSelector::from_config("pekko.actor.default-dispatcher"))` を呼ぶ
- **THEN** `custom_configurator` 由来の `Ok(MessageDispatcherShared)` が返る
- **AND** typed facade 独自の normalize/remap ロジックでユーザー override が shadow されない (Bugbot Medium PR #1644 回帰防止)

#### Scenario: canonical_id は alias chain を辿った最終 id を返す (resolve counter は増やさない)

- **GIVEN** `register_alias("A", "B")` と `register("B", configurator)` が成功
- **WHEN** `canonical_id("A")` を呼ぶ
- **THEN** `Ok("B")` が返る
- **AND** `resolve_call_count()` は増えない (diagnostic 用途の別経路として分離される)
