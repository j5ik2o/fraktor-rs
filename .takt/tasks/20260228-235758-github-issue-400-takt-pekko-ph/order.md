## GitHub Issue #400: takt: Pekkoギャップ埋め Phase 1-2 (trivial/easy/medium)

## 背景
Pekkoギャップ分析（2026-02-28）で分類した `trivial/easy/medium` を一括で解消する。

参照:
- `docs/gap-analysis/actor-gap-analysis.md`
- `docs/gap-analysis/streams-gap-analysis.md`
- `docs/gap-analysis/remote-gap-analysis.md`
- `docs/gap-analysis/cluster-gap-analysis.md`
- `docs/gap-analysis/persistence-gap-analysis.md`

## スコープ（trivial/easy/medium のみ）
- [ ] actor: `ReceiveTimeout/PoisonPill/Kill` の最小互換追加（easy）
- [ ] actor: ルーター戦略拡張（broadcast/random/hash/smallest-mailbox）（medium）
- [ ] actor: FSM DSL の最小導入（medium）
- [ ] streams: `RestartFlow/RestartSource/RestartSink` の薄いDSL追加（easy）
- [ ] streams: GraphDSL fan-in/fan-out 機能拡張（medium）
- [ ] streams: Attributes 基盤（`withAttributes`/`addAttributes`）追加（medium）
- [ ] remote: quarantine セマンティクス強化（medium）
- [ ] remote: ack バッファ戦略（送受信ウィンドウ）追加（medium）
- [ ] cluster: `join/leave/subscribe/unsubscribe` 公開API追加（medium）
- [ ] cluster: cluster router（pool/group）最小導入（medium）
- [ ] persistence: `persist_async` 互換エイリアス追加（trivial）
- [ ] persistence: `PersistentFSM` 相当レイヤ追加（medium）
- [ ] persistence: `PersistencePluginProxy` 相当抽象層追加（medium）

## 完了条件
- [ ] 各項目に対応する実装とテストが追加されている
- [ ] `./scripts/ci-check.sh all` がパスする
- [ ] 追加APIが既存命名規約・CQS・no_std/std分離に準拠している


### Labels
takt