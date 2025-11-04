## Why
現状の `ActorRef` と `AnyMessage` はローカル配送のみを想定しており、アクターシステムを跨いだ配信経路が定義されていない。そのため、将来的なクラスタリング機能や他プロセスへの制御メッセージ送信が阻害されている。早期にシリアライズとトランスポートの境界を策定し、後続の typed レイヤーやクラスタ統合での API 変更コストを抑える必要がある。

## What Changes
- `AnyMessage` を汎用的にエンコード/デコードできる `Serializer` トレイトを `modules/serializer-core`（`no_std + alloc` 前提）に導入し、標準実装候補（`bincode` / `postcard` / `prost` など）の評価指針を定義する。
- `modules/serializer-core` をベースにしつつ、`std` 依存の JSON 実装やファイル IO をまとめる `modules/serializer-std` を新設し、feature 経由で有効化できるようにする。
- リモート情報と配送制御を担う `modules/remote-core`（`no_std + alloc`）を新設し、`RemotePid` 記述子や `ActorRef` 復元ロジック、`Transport` 抽象のインターフェースを集約する。
- `std` 依存のトランスポート実装（TCP など）を置く `modules/remote-std` を追加し、`remote-core` の抽象を実装する。
- 抽象化された `Transport` トレイトとメッセージバッチ構造（ヘッダ・本文・再送ポリシー）を規定し、PoC 実装（インメモリまたはローカル TCP）を用意する。
- 段階的ロールアウト（PoC → 内部 API 公開 → 外部 API 安定化）とガイドライン更新のロードマップを策定する。

## Impact
- `ActorRef` と `ActorSystem` API にリモート配送前提のフックが追加されるため、既存利用者への移行手順を整理する必要がある。
- ネットワーク越しのメッセージ配送が可能になり、将来のクラスタや typed レイヤー開発に向けた土台となる。
- シリアライズ／トランスポート層の抽象が確立されることで、異なるシリアライザや通信方式を差し替え可能にできる。
- エラーハンドリングや再送制御の定義により、遅延・障害時の振る舞いが明確になる。

## Scope
### Goals
1. `modules/serializer-core` に `AnyMessage` 用シリアライズ API を設計し、`no_std` フレンドリーなデフォルト戦略を決める。
2. `modules/remote-core` に `RemotePid` 記述子および `ActorRef` 復元手順を仕様化する。
3. `modules/serializer-std` / `modules/remote-std` を用いた `std` 依存機能（JSON や TCP など）の外部化を整理する。
4. メッセージバッチ前提の `Transport` 抽象とプロトコルレイアウトを定める。
5. PoC 実装と段階的公開計画をまとめる。

### Non-Goals
- クラスタメンバーシップや gossip などの分散制御実装。
- typed レイヤーの構築（別提案で扱う）。
- 完全なセキュリティ層の導入（後続タスク）。

## Rollout Plan
1. PoC 設計と調査を Spec Kit 上でタスク分解する。
2. `modules/serializer-core` に `Serializer` トレイトとデフォルト実装を追加し、`AnyMessage` と統合する。
3. `modules/remote-core` を新設して `RemotePid` / resolver / `Transport` 抽象を実装する。
4. `modules/serializer-std` と `modules/remote-std` を追加し、JSON や TCP などの `std` 依存実装を feature 経由で提供する。
5. ドキュメントとガイド類を更新し、内部 API として公開後に feature をデフォルト化する。

## Risks & Mitigations
- **シリアライザ選定の難航**: 初期段階では抽象を優先し、デフォルト実装を差し替え可能に設計する。
- **再送制御の複雑化**: PoC 段階では簡易 ACK ベースに限定し、要件増加に応じて拡張。
- **性能劣化**: ベンチマークタスクを後続で設定し、バッチング戦略を調整する。

## Impacted APIs / Modules
- `modules/serializer-core`, `modules/serializer-std`
- `modules/actor/runtime` 配下の `ActorRef`, `ActorSystem` 関連 API
- メッセージエンベロープやバッチング基盤となるモジュール
- シリアライザ／トランスポート拡張ポイント

## References
- protoactor-go remote / PID 実装
- Apache Pekko Remoting Architecture
