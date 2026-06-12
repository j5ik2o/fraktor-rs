# 要件定義

## はじめに

cluster の grain 配送に、メッセージから宛先（entity id / shard id）を導出する差し替え可能な抽出契約を定義する。現状は送信側が grain の識別（kind と entity id）を毎回明示的に構築する必要があり、「メッセージ自体から宛先を導く」規則を宣言して再利用・差し替えする手段がない。また Kafka 互換 Murmur2 partitioning のような標準的な導出規則を提供する置き場もない。

対象は Pekko の `ShardingEnvelope[M]` に相当する envelope 契約、`ShardingMessageExtractor[E, M]` に相当する extractor 契約（entity id / shard id / 内部メッセージの導出）、HashCode 系と Murmur2 の標準実装群、および既存 grain 配送経路への接続点である。gap analysis カテゴリ 8 の easy 項目（extractor 実装群）と、その前提となる medium 項目（envelope / extractor 契約）に対応する。

## 境界コンテキスト

- **対象範囲**: envelope 表現（entity id + 内部メッセージ）、extractor 契約（entity id 導出・shard id 導出・内部メッセージ取り出し）、標準実装群（HashCode / HashCodeNoEnvelope / Murmur2）、extractor を経由して宛先を導出する送信経路の接続点
- **対象外**: shard allocation / rebalance 戦略（shard id を入力に使う配置決定は後続スペックの責務）、`ClusterShardingSettings` 相当の包括的な設定契約、wire serialization の変更、placement（どのノードに置くか）の決定規則の変更、activation / passivation
- **隣接システム／スペックへの期待**: 既存の grain 識別契約（kind と entity id の組、その検証規則）と grain 配送経路が公開契約として利用できること。typed grain facade（cluster-grain-typed-entity-facade で導入済み）の typed 識別・typed 参照が先行例として参照できること。message serialization の契約（cluster-message-serialization-contract が所有）は読み取りのみで変更しないこと

## 要件

### 要件 1: メッセージ envelope 契約

**目的:** grain 利用者として、宛先の指定をメッセージと一緒に運ぶために、entity id と内部メッセージを組にした envelope 表現が欲しい

#### 受け入れ基準

1. envelope は常に、entity id と内部メッセージの組を保持しなければならない
2. 利用者が envelope から entity id または内部メッセージを参照したとき、envelope は構築時に与えられた値を返さなければならない
3. envelope は常に、内部メッセージの型を識別できる形で保持しなければならない

### 要件 2: extractor 契約（宛先導出の差し替え点）

**目的:** grain 利用者として、メッセージルーティング規則を宣言・再利用・差し替えするために、メッセージから宛先を導出する契約が欲しい

#### 受け入れ基準

1. extractor 契約は常に、入力メッセージから entity id を導出する操作を提供しなければならない
2. extractor 契約は常に、entity id から shard id を導出する操作を提供しなければならない
3. extractor 契約は常に、入力メッセージから内部メッセージを取り出す操作を提供しなければならない
4. 利用者が独自の導出規則を定義した場合、extractor 契約はそれを既存の標準実装と同じ形で利用できるよう受け入れなければならない
5. 入力メッセージから entity id が導出できない場合、extractor 契約は導出不能を識別できる結果として呼び出し元へ返さなければならない

### 要件 3: 標準 extractor 実装群

**目的:** grain 利用者として、一般的なルーティング規則をすぐに使うために、ハッシュベースの標準実装が欲しい

#### 受け入れ基準

1. HashCode 標準実装を含む場合、envelope から entity id を取り出し、指定された shard 数に基づいて shard id を導出しなければならない
2. HashCodeNoEnvelope 標準実装を含む場合、envelope を使わないメッセージに利用者定義の entity id 導出規則を適用し、HashCode 標準実装と同じ shard 規則で shard id を導出しなければならない
3. Murmur2 標準実装を含む場合、Kafka の標準 partitioning と互換の規則で shard id を導出しなければならない
4. Murmur2 標準実装に Kafka のリファレンス出力が既知の entity id を与えたとき、導出された shard id はそのリファレンス出力と一致しなければならない
5. 標準実装は常に、同一の entity id と同一の shard 数に対して同一の shard id を返さなければならない
6. 標準実装は常に、実行環境やノード構成に依存せず同一入力から同一の導出結果を返さなければならない

### 要件 4: 配送経路への接続点

**目的:** grain 利用者として、宛先の明示構築を繰り返さずにメッセージを送るために、extractor を経由して宛先を導出する送信経路が欲しい

#### 受け入れ基準

1. 利用者が extractor と grain 種別（kind）を指定して envelope を送ったとき、配送経路は extractor が導出した entity id を用いて宛先 grain を解決しなければならない
2. extractor 経由で解決された宛先は常に、同一の kind と entity id を明示構築して送信した場合と同一の grain を指さなければならない
3. extractor が entity id の導出不能を返した場合、配送経路は送信を拒否し、原因を特定できる理由を呼び出し元へ返さなければならない
4. 利用者が extractor を指定しない場合、配送経路は既存の送信手段（識別の明示構築）をそのまま提供し続けなければならない

### 要件 5: 既存挙動の維持と範囲の限定

**目的:** cluster 利用者として、既存機能の安定を保つために、本契約の追加が既存の配送・配置・直列化の挙動を変えないことと、後続スペックの責務を先取りしないことを保証したい

#### 受け入れ基準

1. extractor 契約は常に、既存の grain 配送（識別の明示構築による送信）の公開契約と挙動を変えずに維持しなければならない
2. extractor 契約は本契約の範囲で、placement（ノード選択）の決定規則を変更してはならない
3. extractor 契約は本契約の範囲で、shard allocation / rebalance の戦略および shard id に基づく配置決定を提供してはならない
4. extractor 契約は常に、message serialization の既存契約を変えずに維持しなければならない
