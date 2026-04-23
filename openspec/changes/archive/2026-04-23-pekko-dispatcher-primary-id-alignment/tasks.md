## Phase 1: 参照確認と影響範囲調査

- [x] 1.1 Pekko `Dispatchers.scala:160-164` (`DefaultDispatcherId`) と `Mailboxes.scala:58` (`DefaultMailboxId`) を Read で確認し、primary id の値を特定
- [x] 1.2 `rtk grep 'assert_eq!.*"default"' modules/actor-core modules/actor-adaptor-std --include "*.rs"` で値依存 assertion を列挙。flip 後の期待値に更新が必要な箇所を特定
- [x] 1.3 `rtk grep '"default"' modules/actor-core modules/actor-adaptor-std --include "*.rs"` で string literal 全列挙。Mailboxes 経路で使われている箇所 (`props.mailbox_id("default")` / `mailboxes.resolve("default")` 等) が無いことを確認
- [x] 1.4 `rtk grep 'REGISTERED_DEFAULT_DISPATCHER_ID' modules/ --include "*.rs"` で typed 層の参照を列挙

## Phase 2: Dispatcher primary id flip (DP-M1)

- [x] 2.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatchers.rs` の `DEFAULT_DISPATCHER_ID` の値を `"pekko.actor.default-dispatcher"` に変更 (symbol 名は不変)、rustdoc に Pekko `Dispatchers.scala:160-164` 対応を明記し、「legacy `"default"` は本 change で退役」旨を記述
- [x] 2.2 `register_pekko_default_aliases` を `register_internal_dispatcher_alias` にリネーム:
  - `pekko.actor.default-dispatcher` の alias 登録を削除 (primary entry 自身)
  - `pekko.actor.internal-dispatcher` → `DEFAULT_DISPATCHER_ID` の alias のみ登録
  - **legacy `"default"` alias は追加しない** (完全退役方針)
- [x] 2.3 `register_alias_if_absent` helper を拡張: target を `&'static str` 引数として受け取る形に変更 (従来の DEFAULT_DISPATCHER_ID ハードコードを廃止)
- [x] 2.4 `ensure_default` / `ensure_default_inline` / `replace_default_inline` が primary entry を新 id 下に登録することを確認 (`DEFAULT_DISPATCHER_ID.to_owned()` なので symbol 参照で自動追従)
- [x] 2.5 rustdoc (module-level / 各メソッド) を flip 後の primary id に合わせて更新

## Phase 3: Dispatcher tests の更新

- [x] 3.1 `dispatchers/tests.rs` の既存テストで `"default"` を resolve する箇所を `DEFAULT_DISPATCHER_ID` symbol に置換 (値依存テストは flip 後の値に更新)
- [x] 3.2 `dispatchers/tests.rs::pekko_default_dispatcher_id_resolves_via_alias_registered_by_ensure_default` を `pekko_default_dispatcher_id_resolves_as_primary_entry_after_ensure_default` にリネーム、期待値を「alias ではなく entry 直接 lookup」に変更
- [x] 3.3 `dispatchers/tests.rs::pekko_internal_dispatcher_id_resolves_via_alias_registered_by_ensure_default` の挙動は不変 (internal は引き続き alias)、コメント更新のみ
- [x] 3.4 `dispatchers/tests.rs::register_or_update_is_lenient_and_wipes_existing_alias` を **削除** (legacy `"default"` alias が存在しなくなったため前提が崩れる。同等の wipe 挙動は別 id で検証する必要があれば別テストに切り出し)
- [x] 3.5 新規テスト: `legacy_default_id_is_retired_and_returns_unknown` — ensure_default_inline 後に `resolve("default")` / `canonical_id("default")` が `Err(Unknown("default"))` を返すことを検証 (完全退役の回帰防止)
- [x] 3.6 新規テスト: `ensure_default_inline_registers_only_internal_dispatcher_alias` — aliases に internal-dispatcher の 1 件のみ存在することを検証
- [x] 3.7 既存 alias chain テスト (MAX_ALIAS_DEPTH 関連) は id に依存しないので変更不要
- [x] 3.8 `ensure_default_wipes_preexisting_alias_for_default_id` / `replace_default_inline_wipes_preexisting_alias_for_default_id` を **書き換え**: 対象 id を `DEFAULT_DISPATCHER_ID` (= `"pekko.actor.default-dispatcher"`) に更新、または他の意図的テスト id に変更

## Phase 3.5: legacy `"default"` callers の一括移行 (fraktor-rs 内 56 箇所)

- [x] 3.5.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_sender/tests.rs` の `"default"` string literal を `DEFAULT_DISPATCHER_ID` symbol 参照に置換 (import 追加必要、46 箇所が対象)
- [x] 3.5.2 `modules/actor-core/src/core/typed/dispatchers.rs` 内の rustdoc / コメントに残る `"default"` 言及を新 id に更新
- [x] 3.5.3 `modules/actor-core/src/core/typed/dispatchers/tests.rs` 内のコメントに残る `"default"` 言及を更新
- [x] 3.5.4 その他 `rtk grep '"default"' modules/actor-core modules/actor-adaptor-std --include "*.rs"` の残差を個別にチェック:
  - dispatcher / mailbox 経路でない場合 (例: `ProbeExtension::new("default")`) は対象外、そのまま残す
  - dispatcher / mailbox 経路の場合 (resolve/register/config) は symbol/Pekko id に置換
- [x] 3.5.5 置換後に `rtk cargo build -p fraktor-actor-core-rs` で compile 確認

## Phase 4: Mailbox primary id flip (MB-P1)

- [x] 4.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/mailboxes.rs` の `DEFAULT_MAILBOX_ID` の値を `"pekko.actor.default-mailbox"` に変更
- [x] 4.2 rustdoc (module-level / `ensure_default` メソッド) を更新し、primary id が Pekko 整合になったことを明記
- [x] 4.3 `mailboxes/tests.rs::ensure_default_*` テストの期待値を `"default"` → `"pekko.actor.default-mailbox"` に更新 (DEFAULT_MAILBOX_ID symbol 経由なら自動追従)

## Phase 5: typed 層追従 (DP-TC1)

- [x] 5.1 `modules/actor-core/src/core/typed/dispatchers.rs`:
  - `const REGISTERED_DEFAULT_DISPATCHER_ID: &str = "default"` を削除
  - `use crate::core::kernel::dispatch::dispatcher::DEFAULT_DISPATCHER_ID` で kernel const を import
  - `lookup` 内の `REGISTERED_DEFAULT_DISPATCHER_ID` 参照を `DEFAULT_DISPATCHER_ID` に置換
- [x] 5.2 typed `Dispatchers::DEFAULT_DISPATCHER_ID` / `INTERNAL_DISPATCHER_ID` 定数値は既に Pekko id なので変更不要 (確認のみ)
- [x] 5.3 `typed/dispatchers/tests.rs`:
  - `lookup_from_config_selector_resolves_internal_dispatcher_id_via_kernel_alias`: 挙動不変 (internal は引き続き alias)
  - `lookup_from_config_preserves_user_override_of_pekko_alias`: 旧動作は「user override が alias を wipe して custom entry を insert」だったが、flip 後は `"pekko.actor.default-dispatcher"` が primary entry なので「user override が既存 primary entry を上書き」になる。テスト名と assertion を調整
  - `default_dispatcher_id_matches_kernel_constant`: 不変 (typed const は既に Pekko id)

## Phase 6: CI 検証

- [x] 6.1 `rtk cargo test -p fraktor-actor-core-rs --lib` で既存 regression と新規テスト全 pass 確認
- [x] 6.2 `rtk cargo test -p fraktor-actor-core-rs --tests` でインテグレーションテスト全 pass 確認
- [x] 6.3 `rtk cargo test --workspace` で全ワークスペース pass 確認 (56 箇所の `"default"` callsite が alias 経由で動くこと)
- [x] 6.4 `./scripts/ci-check.sh ai all` 実行し exit 0 を確認
- [x] 6.5 clippy / rustdoc / dylint で新規警告ゼロを確認

## Phase 7: gap-analysis 更新

- [x] 7.1 `docs/gap-analysis/actor-gap-analysis.md` L20 の「分析日」履歴末尾に第19版を追加:
  - `第19版: 2026-04-23 — DP-M1 (Dispatcher primary id alignment) + MB-P1 (Mailbox primary id alignment) 完了反映`
- [x] 7.2 サマリーテーブルに第19版 entry を追加:
  - `内部セマンティクスギャップ数 (第19版、DP-M1 + MB-P1 完了反映後)` — `3+（high 0 / medium 3 / low 約 11）`
  - 残存 medium: `AC-M4b (deferred), FS-M1, FS-M2`
- [x] 7.3 Phase A3 セクションの「完了済み」リストに DP-M1 と MB-P1 を追加
- [x] 7.4 Phase A3 セクションの「残存 medium 4 件」を「残存 medium 3 件: AC-M4b (deferred), FS-M1, FS-M2」に更新
- [x] 7.5 第18版時点で追加した DP-M1 新規セクションを done 表記に更新。MB-P1 も同時に done 化
- [x] 7.6 まとめセクションの「第18版で顕在化した継続 divergence (DP-M1)」bullet を done 化して第19版 note を追加

## Phase 8: PR 発行とレビュー対応

- [x] 8.1 branch `impl/pekko-dispatcher-primary-id-alignment` で PR 発行、base は main
- [x] 8.2 PR 本文:
  - Pekko `Dispatchers.scala:160-164` / `Mailboxes.scala:58` との対応表
  - **挙動変更** (additive + backward-compat):
    - `DEFAULT_DISPATCHER_ID` 定数の**値**が `"default"` → `"pekko.actor.default-dispatcher"` に flip (**symbol 名不変**)
    - legacy `"default"` は alias 経由で透過的に解決 (後方互換)
    - `DEFAULT_MAILBOX_ID` (private) も対称に flip、alias 機構なし (破壊的だが callsite 0)
    - typed `REGISTERED_DEFAULT_DISPATCHER_ID` 削除、kernel const を直接参照
  - gap-analysis DP-M1 / MB-P1 done 化、第19版 medium 4 → 3
- [x] 8.3 レビュー対応: CodeRabbit / Cursor Bugbot の指摘が来た場合は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve
- [x] 8.4 マージ後、別 PR で change をアーカイブ + main spec を `openspec/specs/pekko-dispatcher-primary-id-alignment/spec.md` に sync
