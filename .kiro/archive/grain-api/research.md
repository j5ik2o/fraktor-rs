# 調査ログ: grain-api

## Summary
- `ClusterApiGeneric` と `ClusterIdentity` により参照解決・ask/timeout は最小限実装済みであり、Grain API は既存 API を拡張する形が最小差分になる。
- `SerializedMessage`/`GrainRpcRouter` などの RPC 部品は存在するが、Grain 呼び出しの入口と未統合で、シリアライザ登録の責務境界が未決定。
- 既存サンプルは手動配線で Grain API 利用例に不足があり、`cluster_extension_*` 系の更新が最短導線となる。

## Discovery Scope
- WebSearch: 追加依存なしのため未実施
- Codebase: `modules/cluster/src/core`, `modules/cluster/src/std`, `modules/cluster/examples`
- References: `references/protoactor-go/cluster/grain.go`, `grain_context.go`, `grain.proto`

## Research Log

### 1. 既存 ClusterApi と参照解決
**Findings**
- `ClusterApiGeneric::get/request/request_future` が `ClusterExtension` 経由で `ClusterCore` に接続し、PID 解決と ask/timeout を提供している。
- `ClusterResolveError` は `LookupFailed` を返すため、未確定（Pending）を識別できない。

**Sources**
- `modules/cluster/src/core/cluster_api.rs`
- `modules/cluster/src/core/cluster_resolve_error.rs`

**Implications**
- Grain API は `ClusterApiGeneric` をラップしつつ、未確定/未起動/未登録の区別とリトライ政策を上位で扱う構成が自然。

### 2. Grain 識別子と仮想アクター管理
**Findings**
- `ClusterIdentity` と `GrainKey` が存在し、`kind/identity` から `key` へ変換できる。
- `VirtualActorRegistry` はアクティベーションの記録とイベント生成を担う。

**Sources**
- `modules/cluster/src/core/cluster_identity.rs`
- `modules/cluster/src/core/grain_key.rs`
- `modules/cluster/src/core/virtual_actor_registry.rs`

**Implications**
- Grain 参照型は `ClusterIdentity` を内包し、`VirtualActorEvent` と連携した観測イベントを定義できる。

### 3. RPC/シリアライズ基盤
**Findings**
- `SerializedMessage` と `SchemaNegotiator`、`GrainRpcRouter` が存在し、スキーマ互換性の検証イベント (`RpcEvent`) を発火できる。
- `ClusterApi` の ask 経路とは独立している。

**Sources**
- `modules/cluster/src/core/serialized_message.rs`
- `modules/cluster/src/core/schema_negotiator.rs`
- `modules/cluster/src/core/grain_rpc_router.rs`

**Implications**
- Grain API にメッセージ表現を統合する場合、`SerializedMessage` を選択肢として露出する設計が必要。

### 4. 参照実装の Grain API 形状
**Findings**
- protoactor-go では `GrainCallConfig` と `GrainContext` が用意され、呼び出し設定と実行時文脈を提供する。

**Sources**
- `references/protoactor-go/cluster/grain.go`
- `references/protoactor-go/cluster/grain_context.go`

**Implications**
- `GrainCallOptions` と `GrainContext` を Rust 側でも導入することで利用体験を揃えられる。

## Architecture Pattern Evaluation
- **Option A (ClusterApi 拡張のみ)**: 追加コストは最小だが `ClusterApi` が肥大化し責務境界が曖昧になる。
- **Option B (Grain API 新設)**: Grain API の責務は明確だが導入範囲が広がる。
- **Option C (ハイブリッド)**: `ClusterApi` を下位層に保ちつつ `GrainRef`/`GrainContext` を追加できる。

**Decision**: Option C を軸に設計する。`ClusterApi` は低レベルの解決/ask を担い、Grain API は上位層で利用体験を整える。

## Design Decisions
- Grain 参照型を追加し、identity を保持して `get/request` を提供する。
- 実行コンテキストは ActorContext を包む軽量ラッパで提供する。
- シリアライザは既存 `SerializationExtension` を優先し、`SerializedMessage` を扱う API を別経路で提供する。

## Risks and Mitigations
- **API 境界の曖昧化**: `ClusterApi` と `Grain API` を役割で分離し、命名で境界を明示する。
- **未確定状態の扱い**: `LookupPending` などの明示的なエラーを追加し、リトライの入口に使う。
- **観測イベントの増加**: EventStream の負荷増を想定し、重要イベントに限定して発火する。

## Open Questions
- Grain 呼び出しに `SerializedMessage` を必須化するか、`AnyMessageGeneric` との併用とするか
- `GrainCallOptions` のリトライ戦略を enum で固定するか、関数型を受けるか
- Grain イベントと既存 `RpcEvent` をどう統合するか
