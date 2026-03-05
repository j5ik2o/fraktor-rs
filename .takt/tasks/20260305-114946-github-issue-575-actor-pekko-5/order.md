## GitHub Issue #575: actor: Pekko整合タスク進行トラッカー（#568-#574）

## 目的

actor モジュールの Pekko 整合タスク（#568〜#574）を、依存関係を崩さずに進めるための進行管理。

親Epic: #562

## 実行順（推奨）

### Phase 1: 基盤（Mailbox/Props）

- [ ] #568 feat: Pekkoスタイルのプラガブルメールボックス設計への全面移行
- [ ] #570 actor: Props Selector API 導入（Dispatcher/Mailbox） (G-013)

完了条件:
- `MailboxType`/`MessageQueue` と Props 側の選択 API が衝突せず接続できる

### Phase 2: Supervision DSL 拡張

- [ ] #571 actor: Supervise.on_failure の失敗分類別 DSL 対応 (G-014)
- [ ] #572 actor: SupervisorStrategy のログ制御 API 追加 (G-015)

完了条件:
- 失敗分類 + ログ制御を戦略 API で表現できる

### Phase 3: Service Discovery 境界

- [ ] #573 actor: TypedActorSystem の service discovery 境界を拡張 (G-016)
- [ ] #569 actor: Group Router + Receptionist 連携の追加 (G-012)

依存:
- #569 は #573 の最小 API 仕様確定後に着手

完了条件:
- typed system から discovery を扱え、group router が routee 変化を追従できる

### Phase 4: Ask API 補完

- [ ] #574 actor: typed askWithStatus 相当 API の追加 (G-017)

完了条件:
- 通常 ask と status ask の使い分けが API/テストで明確

## 並行実装の目安

- #571 と #572 は並行可能
- #574 は Phase 2〜3 と並行可能（依存薄）
- #568 と #570 は同時並行より、#568 → #570 の順が安全

## 更新ルール

- 各 Issue 着手時: このトラッカーに「着手コメント」を追記
- 各 Issue 完了時: チェックボックスを更新し、関連 PR を記載
- フェーズ完了時: フェーズ見出しに完了日を追記

### Labels
actor, compatibility