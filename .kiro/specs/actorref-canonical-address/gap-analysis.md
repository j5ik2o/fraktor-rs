# ギャップ分析: actorref-canonical-address

## 1. 現状把握
- **ActorPath/Address**: `modules/actor/src/core/actor_prim/actor_path` が scheme/authority/UID 付き canonical URI を生成できる。`ActorPathParts::local` は authority なしのローカル用。  
- **SystemState**: `system_state.rs` が `RemotingConfig` を適用し、`canonical_parts()` で host/port を保持。`canonical_actor_path` で authority 付きパスを生成・`actor_path_registry` に登録。`canonical_authority_endpoint` も取得可能。  
- **ActorRefGeneric::path()**: `system_state.actor_path(&pid)` を返すため常に authority なし。canonical パスを公開する API が存在しない。  
- **SerializationExtension**: `serialized_actor_path` は `actor_ref.path()` を使い、`TransportInformation` があればそれを先頭に付与、無ければ `local://` のみ。`RemotingConfig` を用いた canonical 付与は未実装。ActorRef 型を検出すると文字列に変換して再シリアライズしている。  
- **送信経路の補完**: `remote_actor_ref_provider.rs` / `tokio_actor_ref_provider.rs` で `reply_to` が authority を持たない場合に限り `writer.canonical_authority_components()` で補完。任意の ActorRef フィールドを包括的に補完する仕組みはない。  
- **ActorRef 解決 API**: ユーザ向けの `ActorSystem::resolve_actor_ref` 相当の公開ファサードが存在しない。  
- **ドキュメント/サンプル**: `examples/tokio_tcp_quickstart` は provider を明示的に呼ぶ前提で、自動 canonical 化の UX を示していない。

## 2. 要件に対するギャップ
- 要件1: ActorRef が canonical URI を公開する手段がない（path はローカルのみ）。  
- 要件2: TransportInformation が無い場合の canonical address 付与が欠落（RemotingConfig を参照しない）。  
- 要件3: 任意の ActorRef フィールドの authority 補完・隔離判定が未実装（reply_to に限定）。  
- 要件4: ActorPath から ActorRef を解決する公開 API が無い。  
- 要件5: Quickstart/ドキュメントが手動 provider 前提のまま。

## 3. 実装アプローチ案
### Option A: 既存拡張
- `ActorRefGeneric` に `canonical_path()` を追加し、`SystemState::canonical_actor_path` を参照（`path()` は互換維持か canonical 優先へ切替）。  
- `SerializationExtension::serialized_actor_path` を拡張し、TransportInformation 不在でも `SystemState::canonical_authority_endpoint` を優先利用、無ければ `local://`。  
- 送信経路で任意の ActorRef フィールドを巡回して authority を補完するヘルパを追加（MessageEnvelope/AnyMessage を再書き込み）。  
- `ActorSystem` / `ExtendedActorSystem` に `resolve_actor_ref(ActorPath)` 公開 API を追加し、scheme ごとに provider を選択。  
- Quickstart を canonical 化前提のコードへ更新。  
**利点**: 変更範囲が既存コンポーネント内で完結。  
**懸念**: ActorRef 補完ロジックの挿入ポイント特定と副作用抑制。

### Option B: 新規コンポーネント
- `ActorRefResolver` サービスを新設し、(1) canonical 付与、(2) シリアライズ補助、(3) 任意フィールドの authority 補完を集中管理。送信前に必ず通すよう Dispatcher/Envelope へフックを追加。  
- `SerializationExtension` は resolver を呼ぶ形に単純化。  
- `ActorSystem::resolver()` を公開し、ユーザ API を一元化。  
**利点**: 責務分離が明確でテスト容易。  
**懸念**: 新レイヤ配線コストと既存パスへの差し込み作業。

### Option C: ハイブリッド
- フェーズ1: Option A の最小セット（canonical_path, serialization fallback, resolve API, Quickstart 更新）を先行。  
- フェーズ2: 送信経路補完ヘルパを共通化し、必要なら resolver サービスへ抽出。  
**利点**: 段階的導入でリスク分散。  
**懸念**: 中間段階の補完漏れに注意、回帰テスト強化が必須。

## 4. 努力度・リスク
- 努力度: **M (3–7日)** — ActorRef/Serialization/Remoting/Doc への横断変更とテスト追加が必要。  
- リスク: **中** — authority 補完漏れや canonical 化ミスがリモート配送失敗に直結。早期に統合テストを用意すべき。

## 5. Research Needed
- RemotingConfig に bind と advertise (public) を分離するフィールドが既にあるか確認。なければ拡張要。  
- AnyMessage/MessageEnvelope 内の ActorRef フィールドを安全に列挙・書き換えする仕組みの有無。  
- no_std 側への影響（canonical_path API 追加時の cfg とカスタム lint との整合）。

## 推奨
Option C の段階導入を推奨。まず canonical_path と serialization の canonical fallback、resolve API、Quickstart 更新を実装し、その後 ActorRef 補完ヘルパを共通化・必要なら resolver 抽出へ進める。
