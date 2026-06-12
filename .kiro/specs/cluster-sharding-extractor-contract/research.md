# 調査・設計判断

## 要約

- **機能**: `cluster-sharding-extractor-contract`
- **ディスカバリー範囲**: 拡張（既存 grain 配送への extractor 契約追加、light discovery + 参照実装比較）
- **主要な発見**:
  - 「メッセージ → entity id」のハードコードは存在しない。現状は呼び出し元が `ClusterIdentity::new(kind, entity_id)` を明示構築して `GrainRef` に渡す設計であり、brief が想定した「既存の固定的な抽出ロジックの SPI 化」ではなく「extractor 経由の解決経路の追加 + 既存経路の維持」が正しい統合形
  - Pekko の `ShardingMessageExtractor[E, M]` は 3 メソッド（`entityId` / `shardId` / `unwrapMessage`）のみの薄い契約。標準実装は `HashCodeMessageExtractor` / `HashCodeNoEnvelopeMessageExtractor` / `Murmur2MessageExtractor` の3系統
  - Kafka 互換 Murmur2 の正確な仕様: `shardId(entityId, n) = (toPositive(murmur2(entityId.utf8_bytes)) % n).toString`、seed=`0x9747B28C`、m=`0x5BD1E995`（Pekko `internal/Murmur2.scala` = Kafka `DefaultPartitioner` 互換）

## 調査ログ

### 既存 grain 配送経路と抽出規則の所在

- **背景**: extractor を差し込むべき接続点の特定
- **参照した情報源**: `modules/cluster-core-kernel/src/grain/grain_rpc_router.rs`, `grain_ref.rs`, `extension/cluster_api.rs`, `activation/cluster_identity.rs`, `activation/placement_coordinator.rs`
- **発見**:
  - `GrainRpcRouter::dispatch(key: GrainKey, ...)` は解決済みの `GrainKey` を受け取るだけで、メッセージからの抽出は行わない
  - `ClusterIdentity::key()`（`"{kind}/{identity}"` 合成）が entity id → `GrainKey` の唯一の変換点。`ClusterApi::resolve_actor_ref` がこれを利用
  - placement は `RendezvousHasher::select(authorities, key)`（FNV 派生の独自 mixing）。shard id とは無関係の authority 選択であり、本 spec は触れない
- **含意**: kernel の既存経路は無変更でよい。extractor 接続点は `GrainRef` の上流に「(kind, extractor, message) → ClusterIdentity → GrainRef」の合成点を新設する形が最小

### Pekko 参照実装の公開面

- **背景**: SPI の最小面と標準実装の正確な仕様確認（過剰設計防止）
- **参照した情報源**: `references/pekko/cluster-sharding-typed/.../ShardingMessageExtractor.scala`, `Murmur2MessageExtractor.scala`, `internal/Murmur2.scala`
- **発見**:
  - `ShardingMessageExtractor[E, M]`: `entityId(message: E): String` / `shardId(entityId: String): String` / `unwrapMessage(message: E): M` の3操作。E=envelope 型、M=内部メッセージ型
  - `ShardingEnvelope[M](entityId: String, message: M)` は検証なしの単純なペア
  - `HashCodeMessageExtractor[M](numberOfShards)`: `envelope.entityId` 抽出 + `hashCode % numberOfShards`。`HashCodeNoEnvelopeMessageExtractor[M]` は entityId が abstract（利用者定義）
  - Pekko の shard id は String（Kafka partition の数値を文字列化）
  - entityId が null の場合は unhandled 扱い（Pekko）→ fraktor では `Option` で導出不能を表現（要件 2.5）
- **含意**: fraktor 側も 3 操作の trait + 3 標準実装 + envelope の最小面で要件 1〜3 を満たせる。公開型数は Pekko（4型 + internal Murmur2）と同水準

### SerializedMessage / GrainCodec との関係

- **背景**: 要件 5.4（serialization 不変）の確認
- **発見**: grain 側 `SerializedMessage { bytes, schema_version }` と actor-core 側 serialization の `SerializedMessage` は別型。extractor はどちらにも依存しない pure な計算として設計できる
- **含意**: serialization 契約とは完全に独立。envelope / extractor は `SerializedMessage` を参照しない

## アーキテクチャパターン評価

| 選択肢 | 説明 | 強み | リスク／制約 | メモ |
|--------|-------------|-----------|---------------------|-------|
| 合成点の新設（採用） | `ShardingRouter` が (kind, extractor) を保持し、メッセージから `ClusterIdentity` を導出して既存 `GrainRef` へ委譲 | kernel 既存経路が無変更（5.1）、Pekko `entityRefFor` 系と対称 | 公開型が1つ増える | 送信ロジックは `GrainRef` が正本のまま |
| `ClusterApi::resolve_actor_ref` への hook 注入 | 既存解決経路の内部に extractor を差し込む | 接続が深い | 既存契約の変更（5.1 違反リスク）、extractor 未使用経路にも影響 | 却下 |
| typed facade（cluster-core-typed）に接続点を置く | `GrainTypeKey<M>` + extractor の typed 統合 | 型安全 | 要件は kernel レベルで満たせる。typed 統合は本 spec の要件にない（YAGNI） | 後続で必要になれば typed wrapper を追加（境界外に明記） |

## 設計判断

### 判断: extractor trait は `ShardingMessageExtractor<E, M>` のジェネリック契約とし、`entity_id` は `Option<String>` を返す

- **背景**: 要件 2.1〜2.5（3操作 + 導出不能の識別）
- **検討した代替案**: (1) `AnyMessage` ベースの非ジェネリック trait — downcast が利用者に漏れ、型安全性を失う。(2) entity_id を `Result<String, E>` — エラー型の発明が必要だが「導出できない」以上の情報がない
- **採用したアプローチ**: Pekko と同型の `<E, M>` ジェネリクス。導出不能は `Option::None`（要件 2.5 の「識別できる結果」）。ジェネリクスは no_std で完結し typed facade に依存しない
- **トレードオフ**: trait object 化には型パラメータの固定が必要だが、接続点はジェネリック保持で対応

### 判断: shard id は `String` で表現する（Pekko parity）

- **背景**: 要件 2.2 / 3.x。後続の shard allocation spec が入力に使う
- **根拠**: Pekko `shardId: String`、Kafka partition の文字列化と互換。数値型にすると HashCodeNoEnvelope 等の利用者定義規則が数値以外を返せなくなる
- **トレードオフ**: 数値が必要な後続 spec はパースが必要（後続 spec の責務として委ねる）

### 判断: HashCode 標準実装のハッシュ関数は FNV-1a 32bit を固定仕様として文書化する

- **背景**: 要件 3.1, 3.5, 3.6（決定性・環境非依存）。Rust には JVM `String.hashCode` 相当の標準がなく、`std::hash::Hasher` 既定（SipHash）はシード乱択のため環境非依存性を満たさない
- **検討した代替案**: JVM `String.hashCode` 互換実装 — Pekko とのハッシュ互換は要件にない（Kafka 互換が要件なのは Murmur2 のみ）
- **採用したアプローチ**: FNV-1a 32bit（リポジトリ既存の RendezvousHasher が FNV 派生である前例に整合）を rustdoc で固定仕様として明記
- **トレードオフ**: Pekko の HashCode 実装と同一 entity id で異なる shard に割れるが、shard 割当の越境互換は要件外

### 判断: Murmur2 は Kafka リファレンスベクタで互換を証明する in-repo 実装とする

- **背景**: 要件 3.3, 3.4。brief 制約「Kafka のリファレンス出力との互換を参照ベクタテストで証明」
- **検討した代替案**: 外部 crate の採用 — no_std 対応・メンテ状況・~30行のアルゴリズムに対する依存追加コストを考慮し却下（build vs adopt）
- **採用したアプローチ**: Pekko `internal/Murmur2.scala`（= Kafka `DefaultPartitioner`）と同一の定数・手順で実装し、Kafka の既知ベクタ（複数 entity id → partition）を sibling テストに焼き込む
- **フォローアップ**: 参照ベクタは Kafka 本家 `Utils.murmur2` の出力に基づくこと（値の出典をテストコメントに記録）

### 判断: 接続点は `ShardingRouter<E, M, X>`（kind + extractor 保持の合成点）とし、既存 `GrainRef` へ委譲する

- **背景**: 要件 4.1〜4.4 と 5.1（既存契約不変）
- **採用したアプローチ**: `ShardingRouter` は (ClusterApi, kind, extractor) を保持し、(1) メッセージから `ClusterIdentity` を導出して `GrainRef` を返す解決操作、(2) tell / request / request_future の送信委譲（unwrap した内部メッセージを `AnyMessage` 化して既存 `GrainRef` の同名操作へ）を提供する。導出不能・識別不正は専用エラーで拒否（4.3）
- **根拠**: 既存 `GrainRef` / `ClusterApi` / placement に一切手を入れない（5.1, 5.2）。Pekko の「extractor は sharding region の入口で適用される」構図と対称
- **トレードオフ**: GrainRef の送信面と一部重複した API 形状になるが、責務（宛先導出 vs 送信実行）の分離として正当化。送信実行の正本は GrainRef のまま

### 判断: 専用エラー型 `ShardingDispatchError` を新設する

- **背景**: 要件 4.3（導出不能の拒否理由）と 5.1（既存 `GrainCallError` の不変）
- **採用したアプローチ**: `EntityIdUnderivable` / `InvalidIdentity(ClusterIdentityError)` / `Call(GrainCallError)` の3 variant。既存エラー型へ variant を追加しない
- **根拠**: エラー型は常に独立ファイル（type-organization ルール）。既存契約への variant 追加は利用側の網羅 match を壊す

## リスクと緩和策

- **HashCode のハッシュ仕様が暗黙に変わるリスク** — rustdoc に固定仕様（FNV-1a 32bit、offset/prime 定数）を明記し、既知ベクタの sibling テストで回帰を検出
- **Murmur2 参照ベクタの出典曖昧化** — テストコメントに Kafka 由来である旨と入力/期待値の根拠を記録
- **接続点 API の肥大化** — 送信面は GrainRef と同名 3 操作（tell_with_sender / request / request_future）+ 解決操作のみに限定。options / codec は解決後の `GrainRef` 側で指定する（重複させない）

## 参考資料

- `references/pekko/cluster-sharding-typed/.../ShardingMessageExtractor.scala` — SPI 3操作と標準実装群
- `references/pekko/cluster-sharding-typed/.../internal/Murmur2.scala` — Kafka 互換 Murmur2（seed 0x9747B28C / m 0x5BD1E995）
- `.kiro/specs/cluster-sharding-extractor-contract/brief.md` — discovery 決定事項
- `docs/gap-analysis/cluster-gap-analysis.md` — カテゴリ8（extractor 実装群 easy / envelope・SPI medium）
