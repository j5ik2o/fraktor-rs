## GitHub Issue #577: Pekko整合レビュー(main..HEAD): Mailbox未接続を含む設計乖離の統合対応

## 背景
`origin/main..HEAD`（現ブランチ `takt/575/github-issue-575-actor-pekko-t`）の差分コミットをレビューしたところ、Pekko整合の観点で複数の未統合ポイントが残っていた。

- 対象コミット: `ad3f1fb6ea6cf4e5b6d0cd99299279489c470492`

## 問題点

### 1. MailboxType/MessageQueue が実行経路に未接続
Pekko寄せとして `MailboxType` / `MessageQueue` は追加されているが、ランタイム生成経路は依然として `MailboxPolicy` 直結。

- `modules/actor/src/core/actor/actor_cell.rs:117`
  - `Mailbox::new(props.mailbox_policy())` を直接使用
- `modules/actor/src/core/dispatch/mailbox/base.rs:29-35`
  - `Mailbox` 自体が `policy` + `QueueStateHandle` を保持する旧構造
- `modules/actor/src/core/dispatch/mailbox/mailboxes.rs:15`
  - レジストリは `HashMap<String, MailboxConfig>` のまま
- `modules/actor/src/core/typed/props.rs:106-115`
  - `MailboxSelector::Bounded` も `MailboxPolicy` + `MailboxConfig` 合成

結果として、`BoundedMailboxType/UnboundedMailboxType` は定義・単体テスト止まりで、実運用の生成点に入っていない。

### 2. Typed `DispatcherSelector::Blocking` がデフォルト構成で解決不能
`Blocking` が固定ID `"blocking-dispatcher"` を使う一方で、デフォルト登録は `"default"` のみ。

- `modules/actor/src/core/typed/props.rs:97`
  - `with_dispatcher_id("blocking-dispatcher")`
- `modules/actor/src/core/dispatch/dispatcher/dispatchers.rs:64-66`
  - `ensure_default()` は `default` しか登録しない
- `modules/actor/src/core/system/base.rs:581-584`
  - 未登録IDは `resolve_dispatcher` 失敗で spawn エラー

`Blocking` セレクタを使うと、明示登録なしで失敗するため API 期待とずれる。

### 3. Typed Receptionist / Group Router のライフサイクル整合不足
Pekko typed の receptionist/group ルータが持つ運用上の前提（購読解除・死活追従・システム標準受付）に対して不足がある。

- `modules/actor/src/core/typed/receptionist_command.rs:16-53`
  - `Unsubscribe` がない
- `modules/actor/src/core/typed/receptionist.rs:61-68`
  - `Subscribe` で subscriber を積むのみ（解除・重複抑止・終端追跡なし）
- `modules/actor/src/core/typed/group_router_builder.rs:47`
  - `build` が `TypedActorRef<ReceptionistCommand>` の外部注入必須（システム標準 receptionist を暗黙利用しない）

結果として stale subscriber/routee が残る可能性や、利用時の手作業設定コストが高い。

### 4. `Listing::typed_refs` の型整合チェック不足
`Listing` が `type_id` を持っているにもかかわらず、`typed_refs<M>()` 側で整合検証をしない。

- `modules/actor/src/core/typed/listing.rs:55-59`
  - `TypeId` 不一致でも `TypedActorRef<M>` へ変換してしまう

型安全APIに見えるが、実質的には呼び出し側責任へ丸投げになっている。

### 5. 追加機能に対する統合テスト不足
複数機能で「統合テスト計画コメントのみ」で、実動作の検証が未実装。

- `modules/actor/src/core/typed/receptionist/tests.rs:26-36`
- `modules/actor/src/core/typed/group_router_builder/tests.rs:9-20`

重要な連携点（登録・購読通知・経路更新・ルーティング）が未検証のまま。

## 期待する対応
- [ ] `MailboxType -> MessageQueue -> Mailbox` の生成経路を一本化し、`Mailboxes` レジストリも型ファクトリ中心へ移行
- [ ] `DispatcherSelector::Blocking` はデフォルトで解決可能にする（予約IDの標準登録 or selector解決方式見直し）
- [ ] Receptionist に購読解除/終端追従を追加し、Group Router の receptionist 依存を簡素化
- [ ] `Listing::typed_refs` を `Result` 化するか、`TypeId` 検証を必須化
- [ ] 上記に対する統合テストを追加

## 補足
このIssueは、Pekko整合の観点で `main..HEAD` 差分から見えた論点を1本に統合したトラッキング用。


### Labels
actor, compatibility