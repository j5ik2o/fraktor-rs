# 要件ドキュメント

## 導入

本ドキュメントは、fraktor-cluster-rs における `PartitionIdentityLookup` 実装の要件を定義する。現在の実装は `NoopIdentityLookup` のみであり、protoactor-go の `disthash` パッケージに相当する分散ハッシュベースの Identity Lookup が必要である。

`PartitionIdentityLookup` は、クラスタ内の仮想アクター（Grain）の配置と解決を担う中核コンポーネントであり、`VirtualActorRegistry` を用いたアクティベーション管理、`RendezvousHasher` を用いたオーナーノード選定、`PidCache` を用いた高速キャッシュ検索を統合する。

## 要件

### 要件1: IdentityLookup トレイトの実装

**目的:** クラスタ開発者として、分散ハッシュベースの Identity Lookup を利用し、仮想アクターのアドレス解決を実現したい。

#### 受け入れ条件

1. `PartitionIdentityLookup` 構造体が作成されたとき、`IdentityLookup` トレイトを実装しなければならない
2. `setup_member` が呼び出されたとき、`PartitionIdentityLookup` は提供された `ActivatedKind` リストを内部に保存しなければならない
3. `setup_client` が呼び出されたとき、`PartitionIdentityLookup` は提供された `ActivatedKind` リストを内部に保存しなければならない
4. `PartitionIdentityLookup` は `Send + Sync` を実装しなければならない
5. `PartitionIdentityLookup` は `no_std` 環境で動作しなければならない

### 要件2: Grain PID の解決

**目的:** クラスタ開発者として、`GrainKey` から対応する PID を取得し、仮想アクターとの通信を実現したい。

#### 受け入れ条件

1. `get` メソッドが呼び出されたとき、`PartitionIdentityLookup` は `GrainKey` に対応する PID を返さなければならない
2. キャッシュにヒットした場合、`PartitionIdentityLookup` は `VirtualActorRegistry` を経由せずに PID を返さなければならない
3. キャッシュにヒットしない場合、`PartitionIdentityLookup` は `VirtualActorRegistry` を使用してアクティベーションを確保しなければならない
4. 有効なオーナーノードが存在しない場合、`PartitionIdentityLookup` は `None` を返さなければならない
5. `PartitionIdentityLookup` は `get` 呼び出し時に現在時刻を受け取り、TTL 判定に使用しなければならない

### 要件3: オーナーノード選定

**目的:** クラスタ開発者として、Rendezvous ハッシュアルゴリズムに基づく一貫したオーナーノード選定を利用し、負荷分散と再配置の最小化を実現したい。

#### 受け入れ条件

1. `PartitionIdentityLookup` は `RendezvousHasher` を使用してオーナーノードを選定しなければならない
2. 同一の `GrainKey` と同一のメンバーリストに対して、`PartitionIdentityLookup` は常に同一のオーナーノードを選定しなければならない
3. メンバーリストに変更がない限り、`PartitionIdentityLookup` は既存のアクティベーションを維持しなければならない
4. `PartitionIdentityLookup` は現在のクラスタメンバーリスト（authority リスト）を保持しなければならない

### 要件4: PID キャッシュ統合

**目的:** クラスタ開発者として、高速な PID ルックアップを利用し、ネットワーク遅延を最小化したい。

#### 受け入れ条件

1. `PartitionIdentityLookup` は内部に `PidCache` を保持しなければならない
2. アクティベーション成功時、`PartitionIdentityLookup` は PID をキャッシュに登録しなければならない
3. キャッシュエントリが TTL を超過した場合、`PartitionIdentityLookup` はキャッシュミスとして扱わなければならない
4. キャッシュ容量を超過した場合、`PartitionIdentityLookup` は古いエントリを削除しなければならない
5. `PartitionIdentityLookup` はキャッシュ容量と TTL を設定可能にしなければならない

### 要件5: VirtualActorRegistry 統合

**目的:** クラスタ開発者として、`VirtualActorRegistry` を通じてアクティベーション状態を一元管理したい。

#### 受け入れ条件

1. `PartitionIdentityLookup` は内部に `VirtualActorRegistry` を保持しなければならない
2. 新規アクティベーション時、`PartitionIdentityLookup` は `VirtualActorRegistry::ensure_activation` を呼び出さなければならない
3. `VirtualActorRegistry` がイベントを生成した場合、`PartitionIdentityLookup` はそれを取得可能にしなければならない
4. `PartitionIdentityLookup` は `VirtualActorRegistry` の設定（キャッシュ容量、PID TTL）を受け取らなければならない

### 要件6: トポロジ変更への対応

**目的:** クラスタ開発者として、ノードの参加・離脱時に自動的にキャッシュとアクティベーションが更新され、システムの一貫性を維持したい。

#### 受け入れ条件

1. メンバーが離脱したとき、`PartitionIdentityLookup` は該当 authority のキャッシュエントリを無効化しなければならない
2. メンバーが離脱したとき、`PartitionIdentityLookup` は該当 authority のアクティベーションを無効化しなければならない
3. トポロジ更新を受け取ったとき、`PartitionIdentityLookup` は内部の authority リストを更新しなければならない
4. 新しいメンバーが参加したとき、`PartitionIdentityLookup` は既存のアクティベーションに影響を与えてはならない（新規ルックアップのみ新配置）
5. `PartitionIdentityLookup` は `on_member_left` メソッドを提供しなければならない
6. `PartitionIdentityLookup` は `update_topology` メソッドを提供しなければならない

### 要件7: PID の削除

**目的:** クラスタ開発者として、不要になった PID をレジストリから削除し、リソースを解放したい。

#### 受け入れ条件

1. `remove_pid` メソッドが呼び出されたとき、`PartitionIdentityLookup` は指定された `GrainKey` のキャッシュエントリを削除しなければならない
2. `remove_pid` メソッドが呼び出されたとき、`PartitionIdentityLookup` は指定された `GrainKey` のアクティベーションを削除しなければならない
3. 存在しない `GrainKey` に対して `remove_pid` が呼び出された場合、`PartitionIdentityLookup` はエラーなく処理を完了しなければならない

### 要件8: アイドルパッシベーション

**目的:** クラスタ開発者として、長期間使用されていないアクティベーションを自動的にパッシベートし、メモリを解放したい。

#### 受け入れ条件

1. `passivate_idle` メソッドが呼び出されたとき、`PartitionIdentityLookup` は指定された TTL を超えたアクティベーションを削除しなければならない
2. パッシベーション時、`PartitionIdentityLookup` は対応するキャッシュエントリも削除しなければならない
3. `PartitionIdentityLookup` は `VirtualActorEvent::Passivated` イベントを生成しなければならない

### 要件9: ClusterCore 統合

**目的:** クラスタ開発者として、`ClusterCore` から `PartitionIdentityLookup` を利用し、クラスタ全体の Identity 解決を実現したい。

#### 受け入れ条件

1. `ClusterCore` は `PartitionIdentityLookup` を `IdentityLookup` として受け入れなければならない
2. `ClusterCore::setup_member_kinds` 呼び出し時、`PartitionIdentityLookup` の `setup_member` が呼び出されなければならない
3. `ClusterCore::setup_client_kinds` 呼び出し時、`PartitionIdentityLookup` の `setup_client` が呼び出されなければならない
4. `ClusterCore` がトポロジ更新を受け取ったとき、`PartitionIdentityLookup` に変更が伝播されなければならない

### 要件10: 設定の提供

**目的:** クラスタ開発者として、`PartitionIdentityLookup` の動作をカスタマイズし、ユースケースに応じた最適化を行いたい。

#### 受け入れ条件

1. `PartitionIdentityLookupConfig` 構造体が提供されなければならない
2. `PartitionIdentityLookupConfig` は PID キャッシュ容量を設定可能にしなければならない
3. `PartitionIdentityLookupConfig` は PID TTL（秒）を設定可能にしなければならない
4. `PartitionIdentityLookupConfig` はアイドル TTL（秒）を設定可能にしなければならない
5. `PartitionIdentityLookupConfig` はデフォルト値を提供しなければならない
6. `PartitionIdentityLookup` は `PartitionIdentityLookupConfig` から構築可能でなければならない

### 要件11: イベント通知

**目的:** クラスタ開発者として、Identity Lookup の内部状態変化を監視し、デバッグやメトリクス収集を行いたい。

#### 受け入れ条件

1. `PartitionIdentityLookup` は `VirtualActorEvent` を取得可能な `drain_events` メソッドを提供しなければならない
2. `PartitionIdentityLookup` は `PidCacheEvent` を取得可能な `drain_cache_events` メソッドを提供しなければならない
3. アクティベーション成功時、`VirtualActorEvent::Activated` または `VirtualActorEvent::Reactivated` が生成されなければならない
4. キャッシュヒット時、`VirtualActorEvent::Hit` が生成されなければならない
5. パッシベーション時、`VirtualActorEvent::Passivated` が生成されなければならない
