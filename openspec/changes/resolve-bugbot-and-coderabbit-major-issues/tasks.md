## 1. Actor Runtime Safety
対応 issue: #560, #579, #581, #582, #600, #610, #611, #617, #618, #619, #762, #788, #792, #793

- [x] 1.1 `Mailbox::new_with_queue` と mailbox 構築経路を見直し、policy と queue の不整合を型または構築手順で防ぐ (#611, #618)
- [x] 1.2 registry / selector / dispatcher 解決経路を修正し、blocking dispatcher と mailbox 設定の解決結果を一貫させる (#579)
- [x] 1.3 bounded mailbox と metrics 更新の同期戦略を修正し、TOCTOU と double-lock を解消する (#581, #610, #617, #619)
- [x] 1.4 supervision 再起動時の intercepted behavior 再初期化と `supervisor_strategy` 読み取り経路を修正する (#600, #762)
- [x] 1.5 stash buffer の `contains` / `exists` / `foreach` を lock 外 callback 評価に変更する (#793)
- [x] 1.6 group router の hashing 戦略を実装保証に合わせて是正し、関連 examples / tests を更新する (#788, #792)
- [x] 1.7 actor spawn / receptionist 登録失敗時の rollback と `stop_child` 副作用テストを補強する (#582, #560)

## 2. Streams Backpressure Integrity
対応 issue: #621, #627, #628, #630, #634, #635, #637, #638, #639, #640, #753

- [x] 2.1 `Source::create` の取り込み経路を修正し、遅い producer でも `WouldBlock` で恒久失敗しないようにする (#753)
- [x] 2.2 `SourceQueue` / `BoundedSourceQueue` の pending offer、overflow、wake 通知の状態遷移を整理する (#627, #637)
- [x] 2.3 `QueueOfferFuture::poll` の self-wake 挙動と wake 観測テストを修正する (#638, #639)
- [x] 2.4 graph interpreter の async/timer 出力保持戦略を修正し、apply failure 後の値消失を防ぐ (#621)
- [x] 2.5 `actor_ref` / `actor_ref_with_backpressure` / cancel-complete 経路を実装契約どおりに修正する (#630, #635, #640)
- [x] 2.6 actor sink / timer graph stage / source queue の回帰テストを追加して major / bug 指摘を再発防止する (#628, #634)

## 3. Workflow Integrity
対応 issue: #507, #508, #578, #795

- [x] 3.1 `.takt/pieces/pekko-porting.yaml` の routing rules 構造を修正し、schema どおりに解釈されるようにする (#578)
- [x] 3.2 `.takt/facets/output-contracts/design-comparison.md` の nested code fence を修正する (#508)
- [x] 3.3 `.takt/facets/instructions/pekko-gap-analyze.md` を wiring するか削除し、未参照 instruction を解消する (#507)
- [x] 3.4 `scripts/ci-check.sh` の `run_examples` を `run_cargo` 経由に統一し、AI ガードを全経路に適用する (#795)

## 4. Integrated Verification
対応 issue: #507, #508, #560, #578, #579, #581, #582, #600, #610, #611, #617, #618, #619, #621, #627, #628, #630, #634, #635, #637, #638, #639, #640, #753, #762, #788, #792, #793, #795

- [x] 4.1 actor / streams / workflow の変更に対応する対象テストと lint を段階的に実行する (#507, #508, #560, #578, #579, #581, #582, #600, #610, #611, #617, #618, #619, #621, #627, #628, #630, #634, #635, #637, #638, #639, #640, #753, #762, #788, #792, #793, #795)
- [x] 4.2 閉じられる `[BugBot]` / `[CodeRabbit:major]` issue をコード確認つきで再棚卸しする (#507, #508, #560, #578, #579, #581, #582, #600, #610, #611, #617, #618, #619, #621, #627, #628, #630, #634, #635, #637, #638, #639, #640, #753, #762, #788, #792, #793, #795)
- [x] 4.3 解消済み issue に確認根拠コメントを付けてクローズする (#507, #508, #560, #578, #579, #581, #582, #600, #610, #611, #617, #618, #619, #621, #627, #628, #630, #634, #635, #637, #638, #639, #640, #753, #762, #788, #792, #793, #795)
- [x] 4.4 重複 issue を重複先への参照付きでクローズする (#507, #508, #560, #578, #579, #581, #582, #600, #610, #611, #617, #618, #619, #621, #627, #628, #630, #634, #635, #637, #638, #639, #640, #753, #762, #788, #792, #793, #795)
- [x] 4.5 未解決 issue を残件として整理し、継続対応対象を明確化する (#507, #508, #560, #578, #579, #581, #582, #600, #610, #611, #617, #618, #619, #621, #627, #628, #630, #634, #635, #637, #638, #639, #640, #753, #762, #788, #792, #793, #795)
