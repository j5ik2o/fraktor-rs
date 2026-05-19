# Design: pekko-dispatcher-primary-id-alignment

## 前提: 直前 change `pekko-dispatcher-alias-chain` で確立された機構

本 change は `Dispatchers` に alias chain resolution が既に入っていることが前提:

- `Dispatchers::resolve(id)` は lookup 前に alias chain を 0〜20 段辿る
- `register_or_update(id, configurator)` は既存 alias を wipe して entry を insert (last-writer-wins)
- `register(id)` / `register_alias(alias, target)` は strict で `AliasConflictsWithEntry` / `Duplicate` を返す
- `ensure_default` 系は primary entry 登録後に `register_pekko_default_aliases` を呼んで 2 Pekko id を alias 登録

本 change はこの機構を活かして primary id を flip する。

## Decision 1: primary id flip + legacy `"default"` の完全退役 (DP-M1)

**Before**:
```
entries:  "default" (primary), "pekko.actor.default-blocking-io-dispatcher"
aliases:  "pekko.actor.default-dispatcher" → "default"
          "pekko.actor.internal-dispatcher" → "default"
```

**After**:
```
entries:  "pekko.actor.default-dispatcher" (primary), "pekko.actor.default-blocking-io-dispatcher"
aliases:  "pekko.actor.internal-dispatcher" → "pekko.actor.default-dispatcher"
          (legacy "default" alias は登録しない)
```

### legacy `"default"` を完全退役する理由

`"default"` は Pekko / Akka のどちらにも存在しない fraktor-rs 独自の造語である。後方互換 alias として残すと:

- Pekko 互換性の主張が曖昧になる (「Pekko id だけでも `"default"` でも解決できる」という冗長な契約)
- ユーザーコード内で 2 種類の綴りが混在し、code review / search 時の混乱を招く
- `"default"` が残存している限り、将来的に Pekko 側が `"default"` を何らかの意味で導入した場合の衝突リスクがある

fraktor-rs は開発フェーズで後方互換不要 (CLAUDE.md 明記) のため、本 change で完全退役する。callsite は fraktor-rs リポジトリ内 56 箇所のみで、機械的置換で対応可能。

### 実装:

```rust
pub const DEFAULT_DISPATCHER_ID: &str = "pekko.actor.default-dispatcher";
// (値のみ flip、symbol 名は不変。internal callers は再 compile 時に自動追従)

const PEKKO_INTERNAL_DISPATCHER_ID: &str = "pekko.actor.internal-dispatcher";

fn register_internal_dispatcher_alias(&mut self) {
  // internal-dispatcher のみを primary にエイリアス (legacy "default" は登録しない)
  Self::register_alias_if_absent(&mut self.aliases, &self.entries, PEKKO_INTERNAL_DISPATCHER_ID, DEFAULT_DISPATCHER_ID);
}

fn register_alias_if_absent(
  aliases: &mut HashMap<String, String, RandomState>,
  entries: &HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>, RandomState>,
  alias: &str,
  target: &'static str,
) {
  if entries.contains_key(alias) {
    return;
  }
  aliases.entry(alias.to_owned()).or_insert_with(|| target.to_owned());
}
```

### 旧 change との差分

直前 change (`pekko-dispatcher-alias-chain`) の `register_alias_if_absent` は target が常に `DEFAULT_DISPATCHER_ID` 固定だった (helper 内でハードコード)。本 change では target 引数を受け取る形に拡張し、将来的に複数 primary への alias 登録にも再利用可能にする。旧 change の `register_pekko_default_aliases` (複数 alias 登録) は、flip 後は internal-dispatcher 1 件のみになるため `register_internal_dispatcher_alias` にリネーム。

### Callsite migration (fraktor-rs 内 56 箇所)

主要箇所は `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_sender/tests.rs` の 56 個のテストセットアップで、以下のパターンを機械的置換する:

```rust
// 旧
let settings = DispatcherConfig::new("default", ...);
config.with_dispatcher_configurator("default", configurator_handle)
state.resolve_dispatcher("default")

// 新 (推奨: const symbol 参照)
let settings = DispatcherConfig::new(DEFAULT_DISPATCHER_ID, ...);
config.with_dispatcher_configurator(DEFAULT_DISPATCHER_ID, configurator_handle)
state.resolve_dispatcher(DEFAULT_DISPATCHER_ID)
```

const symbol 参照に統一することで、将来 DEFAULT_DISPATCHER_ID の値が再度変わっても追従する。

## Decision 2: Mailbox primary id flip は alias なしの直接置換 (MB-P1)

**Before**: `const DEFAULT_MAILBOX_ID: &str = "default"` (private), `Mailboxes::ensure_default` が `"default"` 下に primary entry を登録
**After**: `const DEFAULT_MAILBOX_ID: &str = "pekko.actor.default-mailbox"` (private)、`Mailboxes::ensure_default` が新 id 下に登録

### Alias 機構を追加しない理由

Mailboxes registry の現状:
- `entries: HashMap<String, MailboxConfig>` 単体、alias 機構なし
- `DEFAULT_MAILBOX_ID` は private const で、参照は 3 箇所 (mailboxes.rs 2 + tests 1) のみ
- ユーザーは `Props::mailbox_id()` で任意 id を指定可能だが、未指定時は `props.mailbox_config()` を inline で使用 (registry lookup 不要)
- registry lookup は `ActorCell::create` で `mailbox_id` が Some の場合のみ発生

以上から、**`"default"` を legacy alias として保持する価値は低い** (ユーザーが意図的に `props.mailbox_id("default")` している箇所が無い限り破壊的でない)。

### 将来的な alias 機構追加について

Mailboxes に alias chain resolution を追加すれば Dispatchers と対称になるが、以下を考慮して本 change では見送る:

- Dispatchers の alias は Pekko 互換 (`pekko.actor.default-dispatcher` が Pekko reference.conf の key) という明確な動機がある
- Mailboxes の Pekko 原典 reference.conf にも `pekko.actor.default-mailbox` / `pekko.actor.default-control-aware-dispatcher-mailbox` 等の key はあるが、fraktor-rs では MailboxConfig を typed builder で構築する convention が確立しており、id 経由の lookup は稀
- 必要性が生じたら `pekko-mailbox-alias-chain` として別 change で追加可能

## Decision 3: typed 層は kernel const を直接参照 (DP-TC1)

**Before**:
```rust
const REGISTERED_DEFAULT_DISPATCHER_ID: &str = "default";

pub fn lookup(&self, selector: &DispatcherSelector) -> Result<MessageDispatcherShared, DispatchersError> {
  let id: &str = match selector {
    | DispatcherSelector::Default | DispatcherSelector::SameAsParent => REGISTERED_DEFAULT_DISPATCHER_ID,
    ...
  };
  ...
}
```

**After**:
```rust
use crate::core::kernel::dispatch::dispatcher::DEFAULT_DISPATCHER_ID;

pub fn lookup(&self, selector: &DispatcherSelector) -> Result<MessageDispatcherShared, DispatchersError> {
  let id: &str = match selector {
    | DispatcherSelector::Default | DispatcherSelector::SameAsParent => DEFAULT_DISPATCHER_ID,
    ...
  };
  ...
}
```

### Trade-off

- typed → kernel への依存が増えるが、**typed が kernel の primary id を知らないと整合性が取れない** ことは既定 (typed facade は kernel を単純に wrap する層)
- `REGISTERED_DEFAULT_DISPATCHER_ID` を消すことで「値が 2 箇所で定義されている」問題が解消

## Decision 4: 公開 API / rustdoc の更新で legacy `"default"` の終焉を明記

本 change 以降、`"default"` という id は fraktor-rs のどこにも存在しない (const / entry / alias / rustdoc の全てで廃止)。rustdoc には Pekko 原典を直接参照する形で primary id の意味を記述する:

```
/// Primary identifier for the default dispatcher entry in [`Dispatchers`].
///
/// Corresponds 1:1 to Pekko `Dispatchers.DefaultDispatcherId` in
/// `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatchers.scala:160-164`.
///
/// Historically fraktor-rs used `"default"` as the primary id, but change
/// `pekko-dispatcher-primary-id-alignment` (2026-04-23) flipped to match
/// Pekko. The `"default"` token is no longer registered as an alias or
/// entry; callers must use [`DEFAULT_DISPATCHER_ID`] (or the raw string
/// `"pekko.actor.default-dispatcher"` if a literal is required for
/// external configuration).
pub const DEFAULT_DISPATCHER_ID: &str = "pekko.actor.default-dispatcher";
```

同様の doc を `DEFAULT_MAILBOX_ID` にも追加する。

## Risks and mitigations

### Risk 1: ensure_default 系の内部 dead-lock / register 順序

`ensure_default` は `"pekko.actor.default-dispatcher"` を entry に insert した後 `register_pekko_default_aliases` を呼ぶ。`register_pekko_default_aliases` は `"default"` alias を登録するが、この alias target (`"pekko.actor.default-dispatcher"`) が entries に存在することを前提としている。

**Mitigation**: `register_alias_if_absent` は alias target の存在検証をしない (alias は lazy 解決で resolve() 時にチェックされる)。entries insert → alias insert の順で記述すれば良く、循環参照や順序依存は発生しない。

### Risk 2: 既存 test の legacy `"default"` callers が breaking

56 箇所の `"default"` string literal callers のうち、多くは:
- `DispatcherConfig::new("default", ...)` で configurator 生成
- `config.with_dispatcher_configurator("default", ...)` で register_or_update 経由登録
- `state.resolve_dispatcher("default")` で lookup

**Mitigation**: 本 change では **legacy alias を追加せず完全退役** するため、全 callsite を `DEFAULT_DISPATCHER_ID` symbol に機械的置換する (Phase 3.x)。置換後も挙動は等価 (symbol の値が Pekko id を指すため)。

sed / rustfmt ベースの一括置換で対応可能:
```bash
# 置換例 (人間が rtk grep で確認後に実行)
sed -i '' 's/"default"/DEFAULT_DISPATCHER_ID/g' modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_sender/tests.rs
# ただし "default" は他の意味で使われている可能性があるため、文脈ごとに人間がレビューする
```

### Risk 3: Mailboxes `"default"` の破壊的変更

MB-P1 で mailbox registry primary id が `"default"` から外れるため、`Mailboxes::resolve("default")` は `Err(Unknown)` を返すようになる。現状 callers は 3 箇所 (すべて mailboxes.rs / mailboxes/tests.rs 内) で、flip に合わせて定数参照を更新する。

**Mitigation**: rtk grep で `"default"` 文字列が Mailboxes 経路で使われていないことを再確認する (Phase 1.3)。

### Risk 4: DEFAULT_DISPATCHER_ID の値依存テスト

一部テストで `assert_eq!(id, "default")` のような値直接比較があるかもしれない。

**Mitigation**: Phase 1.2 で `assert_eq!.*"default"` を rtk grep で全列挙し、flip 後の期待値に更新する。

## Non-goals (再掲)

- Mailboxes への alias chain resolution 追加
- DC-P1 / REG-P1 の deprecated marker 化 (別 change)
- DEFAULT_BLOCKING_DISPATCHER_ID の変更 (既に Pekko id)
- `core/typed/dispatchers.rs::DEFAULT_DISPATCHER_ID` / `INTERNAL_DISPATCHER_ID` の定数値変更 (既に Pekko id)
