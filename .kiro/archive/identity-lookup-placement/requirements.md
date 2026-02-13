# 要件ドキュメント

## 導入
本仕様は IdentityLookup/Placement を中心に、PartitionManager/PlacementActor による分散アクティベーションと、
Lock/Storage/Activation の契約を明確化する。目的は配置決定の一貫性、重複アクティベーションの防止、
および運用可能なライフサイクルと観測性を確保することである。

## 要件

### 要件1: Partition/Placement の解決と一貫性
**目的:** クラスタ利用開発者として IdentityLookup/Placement による配置決定を安定して利用し、ルーティングの一貫性を得たい。

#### 受け入れ条件
1. ルックアップ要求が発生したとき、IdentityLookup/Placement 基盤は対象 ID のパーティションと配置先を決定しなければならない。
2. トポロジ変更が発生したとき、IdentityLookup/Placement 基盤は以降の配置決定に更新結果を反映しなければならない。
3. 配置情報が未確定である間、IdentityLookup/Placement 基盤は配置決定を拒否し続けなければならない。
4. 分散アクティベーション機能を含む場合、IdentityLookup/Placement 基盤は同一入力に対して決定的な配置を返さなければならない。
5. IdentityLookup/Placement 基盤は常に現在のパーティション/配置スナップショットを参照可能な形で提供しなければならない。

### 要件2: 分散アクティベーションの単一性
**目的:** クラスタ運用者として同一 ID のアクティベーションが一意に保たれることで、重複実行を防ぎたい。

#### 受け入れ条件
1. アクティベーション要求が発生したとき、IdentityLookup/Placement 基盤は担当ノードを決定しアクティベーションを開始しなければならない。
2. 対象 ID のアクティベーションが既に存在する場合、IdentityLookup/Placement 基盤は新規生成せず既存の参照を返さなければならない。
3. ノードがアクティベーション対象として不適格である間、IdentityLookup/Placement 基盤はアクティベーションを拒否し続けなければならない。
4. アクティベーションが失敗した場合、IdentityLookup/Placement 基盤は失敗理由を通知しなければならない。
5. IdentityLookup/Placement 基盤は常に同一 ID に対して同時に 1 つのアクティベーションのみを許可しなければならない。

### 要件3: Lock/Storage/Activation の契約
**目的:** 基盤開発者として Lock/Storage/Activation の責務が明確化され、実装差し替えが可能でありたい。

#### 受け入れ条件
1. アクティベーションを開始するとき、IdentityLookup/Placement 基盤は排他的なロック取得を行わなければならない。
2. ロック取得に失敗した場合、IdentityLookup/Placement 基盤はアクティベーションを開始してはならない。
3. ロック保持中である間、IdentityLookup/Placement 基盤はストレージに所有者情報を一貫して保持し続けなければならない。
4. ストレージ機能を含む場合、IdentityLookup/Placement 基盤はアクティベーションの所有者と状態を永続化しなければならない。
5. アクティベーションが完了または失敗したとき、IdentityLookup/Placement 基盤はロックを解放しなければならない。

### 要件4: ライフサイクルと観測性
**目的:** クラスタ運用者として Placement 系コンポーネントの起動/停止と状態観測を統一的に扱いたい。

#### 受け入れ条件
1. クラスタがメンバーモードで起動したとき、IdentityLookup/Placement 基盤は Partition/Placement コンポーネントを稼働状態へ遷移させなければならない。
2. クラスタのシャットダウンが要求されたとき、IdentityLookup/Placement 基盤は Partition/Placement コンポーネントを停止し関連リソースを解放しなければならない。
3. 基盤が未起動の間、IdentityLookup/Placement 基盤はルックアップ/アクティベーション要求を拒否し続けなければならない。
4. 状態遷移や失敗が発生したとき、IdentityLookup/Placement 基盤は観測用イベントを通知しなければならない。
5. IdentityLookup/Placement 基盤は常に状態とイベントのタイムスタンプを付与しなければならない。
