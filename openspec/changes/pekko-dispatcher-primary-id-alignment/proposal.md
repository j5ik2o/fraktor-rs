## Why

Pekko の原典では dispatcher / mailbox の primary id が Pekko-prefixed な文字列になっている:

- `Dispatchers.DefaultDispatcherId = "pekko.actor.default-dispatcher"` (`Dispatchers.scala:160-164`)
- `Mailboxes.DefaultMailboxId = "pekko.actor.default-mailbox"` (`Mailboxes.scala:58`)

fraktor-rs は歴史的経緯で **primary entry id が `"default"` (fraktor-rs 独自値)** のまま運用しており、Pekko 公開 id を alias で `"default"` にリダイレクトする構造になっている (change `pekko-dispatcher-alias-chain` で確立):

```
entries: {
  "default"                                       → primary (fraktor-rs legacy)
  "pekko.actor.default-blocking-io-dispatcher"    → primary (blocking; 既に Pekko id)
}
aliases: {
  "pekko.actor.default-dispatcher"    → "default"
  "pekko.actor.internal-dispatcher"   → "default"
}
```

この状態は gap-analysis 第18版で **DP-M1** として medium 登録済で、別 change で対応する旨が記録されている。加えて調査の結果、**MB-P1** (新規) として mailbox 側に対称な divergence があることが判明した (`DEFAULT_MAILBOX_ID = "default"` が private const で registry primary key になっている)。

本 change では primary id を Pekko 原典に合わせて flip し、`"default"` は legacy alias として後方互換を保つ。併せて typed 層の `REGISTERED_DEFAULT_DISPATCHER_ID` (DP-TC1、DP-M1 の derivative) も追随する。

## What Changes

### Dispatcher primary id flip (DP-M1)

- `pub const DEFAULT_DISPATCHER_ID: &str` の値を `"default"` → `"pekko.actor.default-dispatcher"` に flip (**symbol 名は不変**、internal callers は自動追従)
- `ensure_default` / `ensure_default_inline` / `replace_default_inline` が primary entry を新 id で登録する
- **legacy `"default"` alias は追加しない** (完全退役方針)。fraktor-rs 独自の短縮表記 `"default"` は Pekko にも Akka にも存在しないため、後方互換 alias を残すと Pekko 互換性の主張が曖昧になる
- `register_pekko_default_aliases` を `register_internal_dispatcher_alias` にリネームし、内容を `pekko.actor.internal-dispatcher` → `DEFAULT_DISPATCHER_ID` の 1 件のみに縮小 (`pekko.actor.default-dispatcher` は primary entry 自身なので alias 不要)
- **破壊的変更あり** — string literal `"default"` を直接書いている callsite (主に `dispatcher_sender/tests.rs` の 56 箇所) は全て `DEFAULT_DISPATCHER_ID` symbol または `"pekko.actor.default-dispatcher"` full string に置換する

### Mailbox primary id flip (MB-P1)

- `const DEFAULT_MAILBOX_ID: &str` (private) の値を `"default"` → `"pekko.actor.default-mailbox"` に flip
- `Mailboxes::ensure_default` は新 id 下に primary entry を登録する (Mailboxes registry には alias 機構が無いため、`"default"` を参照していた callers は追随更新が必要 — 3 参照のみ、すべて mailboxes 系 tests 内)

**注**: Mailboxes registry に alias 機構を追加するかは本 change の scope outside。callers が少ないため直接更新で十分。必要性が生じたら別 change で Dispatchers と同じ alias chain resolution を追加する。

### typed 層の primary id 追従 (DP-TC1)

- `core/typed/dispatchers.rs` の `REGISTERED_DEFAULT_DISPATCHER_ID = "default"` を削除
- `DispatcherSelector::Default` / `SameAsParent` 解決時は `Dispatchers::DEFAULT_DISPATCHER_ID` (kernel const) を直接参照する
- これにより kernel primary id flip が typed facade にも自動追従する

### Gap-analysis 更新

- 第19版として DP-M1 を done 化、新規 MB-P1 も同時 done 化
- 残存 medium 3 件 (AC-M4b deferred / FS-M1 / FS-M2) に更新

## Capabilities

### Modified Capabilities
- `pekko-dispatcher-alias-chain`: primary entry id の値を Pekko 原典に合わせて flip。alias chain resolution の契約は不変

### New Capabilities
<!-- 該当なし: Mailboxes の primary id flip は内部定数値変更のみで、公開契約の追加はない -->

## Impact

**影響を受けるコード**:

- `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers.rs`:
  - `DEFAULT_DISPATCHER_ID` の値変更
  - `register_pekko_default_aliases` のリネーム + 内容変更 (internal-dispatcher のみ alias)
  - `ensure_default` 系で `"default"` → `DEFAULT_DISPATCHER_ID` の legacy alias を登録
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers/tests.rs`:
  - `ensure_default_inline` 後の resolve テストの期待値更新
  - 新規テスト: legacy alias (`"default"` → primary) 経由の resolve
- `modules/actor-core/src/core/kernel/dispatch/mailbox/mailboxes.rs`:
  - `DEFAULT_MAILBOX_ID` の値変更
- `modules/actor-core/src/core/kernel/dispatch/mailbox/mailboxes/tests.rs`:
  - `DEFAULT_MAILBOX_ID` 参照テストの期待値更新
- `modules/actor-core/src/core/typed/dispatchers.rs`:
  - `REGISTERED_DEFAULT_DISPATCHER_ID` 削除、kernel の `DEFAULT_DISPATCHER_ID` を参照
- `modules/actor-core/src/core/typed/dispatchers/tests.rs`:
  - `default_dispatcher_id_matches_kernel_constant` テストで value 比較更新 (既に Pekko id を assert しているため差分なし)
- `docs/gap-analysis/actor-gap-analysis.md`:
  - 第19版 entry 追加、DP-M1 / MB-P1 done 化

**影響を受ける公開 API 契約**:

- `DEFAULT_DISPATCHER_ID` 定数の**値**が変わる (**symbol 名は不変**)。文字列値 `"default"` をハードコードしているユーザーコードは resolve 失敗するようになる (完全退役方針、legacy alias なし)
- `DEFAULT_MAILBOX_ID` は private const のため公開 API 影響なし
- typed `Dispatchers::DEFAULT_DISPATCHER_ID` / `INTERNAL_DISPATCHER_ID` は既に Pekko id なので変化なし

**挙動変更**:

- `Dispatchers::resolve("default")` は **`Err(DispatchersError::Unknown("default"))`** を返すようになる (従来は entry 直接 lookup で成功していた、または alias 経由で成功していた)。fraktor-rs 内の callsite は本 change で全て `DEFAULT_DISPATCHER_ID` symbol または Pekko full string に置換する
- `Dispatchers::resolve("pekko.actor.default-dispatcher")` は直接 entry lookup になる (alias chain を辿らない)
- `Dispatchers::resolve("pekko.actor.internal-dispatcher")` は引き続き alias 経由で primary entry に解決される (Pekko 互換性のため保持)
- `Mailboxes::resolve("default")` は **Err(Unknown("default"))** になる (Mailboxes に alias 機構がないため、かつ legacy alias も追加しない)

## Non-goals

- Mailboxes registry への alias chain resolution 機構の追加 (別 change で必要性が出たら対応、DP-M1 と比較して impact が小さいため見送り)
- `DEFAULT_BLOCKING_DISPATCHER_ID` の変更 (既に Pekko id `"pekko.actor.default-blocking-io-dispatcher"` なので不変)
- `core/typed/dispatchers.rs` の `DEFAULT_DISPATCHER_ID` / `INTERNAL_DISPATCHER_ID` 定数値の変更 (既に Pekko id)
- DC-P1 (`BootingSystemState` / `RunningSystemState`) / REG-P1 (`#[allow(dead_code)]` 散在) の整理 (別 change `dead-code-deprecation-audit` で対応)
