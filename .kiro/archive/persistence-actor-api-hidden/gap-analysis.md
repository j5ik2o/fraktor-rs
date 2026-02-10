# ギャップ分析: persistence-actor-api-hidden

## 1. 現状調査
### 1.1 既存資産とレイアウト
- **永続アクター基盤**: `modules/persistence/src/core/persistent_actor_base.rs` が `PersistentActorBase::new(persistence_id, journal_ref, snapshot_ref)` を提供し、構築時に ActorRef を要求している。
- **永続アクタートレイト**: `modules/persistence/src/core/persistent_actor.rs` が `persist/save_snapshot/delete_*` を提供し、送信先は `journal_actor_ref()/snapshot_actor_ref()` に依存する。
- **永続化拡張**: `modules/persistence/src/core/persistence_extension.rs` に `PersistenceExtensionGeneric` があり、Journal/Snapshot Actor を生成して ActorRef を返す。ただし ExtensionId が存在せず、自動登録や `ActorSystem` からの取得は未整備。
- **Examples/Tests**: `modules/persistence/examples/persistent_counter_no_std/main.rs` や `modules/persistence/tests/persistent_actor_example.rs` が ActorRef を手渡しで構成している。

### 1.2 パターンと制約
- **core/no_std 境界**: core 内で `std` を使えない (`.kiro/steering/tech.md`)。永続化コンテキストも no_std で成立させる必要がある。
- **Extension 機構**: `modules/actor/src/core/extension/*` と `ExtendedActorSystemGeneric` が extension 登録と取得を提供している。`SerializationExtensionId` が実例。
- **命名規約**: `.codex/skills/avoid-ambiguous-suffixes` に従って責務が明確な名称が必要。

### 1.3 インテグレーション面
- `ActorContextGeneric::self_ref()` は ActorRef 登録済みでないと panic する。`pre_start` で拡張取得・初期化する場合、ActorCell 登録済みタイミングでの呼び出しが前提になる。
- 永続化 API は `PersistentActor` トレイトのデフォルト実装に集約されており、ここを通じてコンテキストへ移譲する設計が可能。

## 2. 要件別ギャップマップ
| 要件領域 | 関連資産 | ギャップ/制約 | Research Needed |
| --- | --- | --- | --- |
| 1. ユーザAPIの簡素化 | `PersistentActorBase::new`, examples/tests | ActorRef 手渡しが前提で、ユーザAPI簡素化要件を満たしていない（Missing）。 | なし |
| 2. 永続化コンテキストの提供 | 該当型なし | 永続化コンテキスト型が存在せず、合成パターンの公開APIが未整備（Missing）。 | コンテキストの責務境界とライフサイクルをどう置くか |
| 3. 拡張の登録と利用 | `PersistenceExtensionGeneric`, `ExtendedActorSystemGeneric` | ExtensionId がなく、ActorSystem からの取得や自動初期化が未整備（Missing）。 | ExtensionId 追加と登録位置（起動前/後）のルール整理 |
| 4. 互換性と境界 | no_std core, `RuntimeToolbox` | core で std 依存を追加できない（Constraint）。 | no_std での初期化フロー設計の確認 |
| 5. 設計指針とサンプル | 既存 examples | 新仕様に合わせた examples が未整備（Missing）。 | 既存 examples の構成に合わせた更新方針 |

## 3. 実装アプローチ候補
### Option A: 既存基盤の拡張
- **内容**: `PersistentActorBase` にコンテキスト相当を内蔵し、`pre_start` で拡張取得→内部 ActorRef を設定。既存 API を最小変更。
- **利点**: 変更範囲が小さく、既存テスト/実装の移行が容易。
- **欠点**: `base` の責務が肥大化し、コンテキストと基盤の境界が曖昧になりやすい。

### Option B: 新規コンテキスト型を追加
- **内容**: `PersistenceContext`（名称は要検討）を新設し、Actor は `pre_start` でコンテキストを取得・保持。`PersistentActor` はコンテキスト経由で操作。
- **利点**: 合成パターンが明確で Rust らしい設計になる。責務境界が明瞭。
- **欠点**: 既存の `PersistentActorBase` との役割分担を整理する必要があり、改修範囲が増える。

### Option C: ハイブリッド
- **内容**: `PersistentActorBase` は最小責務を維持しつつ、コンテキストを薄いラッパとして導入。`PersistentActor` はコンテキスト経由の API を提供し、内部で base を利用。
- **利点**: 既存の base を維持しながら合成導入ができる。段階移行に向く。
- **欠点**: APIが二層になり、設計が複雑化しやすい。

## 4. 努力・リスク評価
- **Effort**: **M (3–7日)** — 既存 API の整理、ExtensionId 追加、examples/tests 更新が必要。
- **Risk**: **Medium** — 起動順序と extension 取得タイミングに注意が必要。no_std での初期化フローが主要リスク。

## 5. デザインフェーズへの持ち越し事項
- ExtensionId 追加と自動登録フロー（system guardian 相当）をどこに置くか。
- `PersistentActorBase` と永続化コンテキストの責務分担（CQS/合成設計の観点）。
- コンテキストの命名（avoid-ambiguous-suffixes 適用）。
- examples の更新方針（既存例に合わせて構成を維持する方針）。

---
この分析は `.kiro/settings/rules/gap-analysis.md` に従い、要求とコードベースの差分を整理したものです。次はこの結果を踏まえて `/prompts:kiro-spec-design persistence-actor-api-hidden` で設計フェーズへ進むことを推奨します。
