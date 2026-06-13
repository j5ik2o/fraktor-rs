# 要件定義

## はじめに

grain / placement（sharding 相当）の運用面にある2つの欠けを埋める。第一に、ノードの grain runtime が「メッセージを受けられる状態か」を外部（ロードバランサ、orchestrator、readiness probe）へ伝える readiness 判定の契約が存在しない。第二に、join 時の設定互換チェックは pubsub / downing / split brain resolver / failure detector / singleton の互換キーだけを合成しており、grain / placement 設定の不一致を join 前に検出できない（Pekko の `ClusterShardingHealthCheck` / `JoinConfigCompatCheckSharding` に相当する欠け）。gap analysis カテゴリ 8 / カテゴリ 10 の easy 項目に対応する。

readiness 判定は現在の runtime 状態（自ノードの membership 状態、placement 調整の状態、kind の登録状態）だけから導出する純粋な判定として定義する。公開手段は core の契約（拡張機能の読み取りアクセサ）として提供し、std を含むホスト環境からそのまま呼び出せるようにする。core が契約を定義しホスト層がそれに従う依存方向を保ち、core を呼ぶだけの便宜層をホスト側に作らない。

join 互換キーについては、調査の結果、現行の grain / placement 設定には「ノード間で一致しないと配送・配置の正しさが壊れる値」が存在しない（identity lookup の実装選択は factory 注入で設定非所有、現行の調整可能値はすべてローカルチューニング値）ことが判明している。このため本機能では、比較対象外の設定を除外理由とともに互換キー目録へ整備することに限定し、required な比較の追加は設定の config 所有化を行う後続の包括設定契約スペックへ委ねる（failure detector の実装選択キーが同じ扱いの先行例）。

## 境界コンテキスト

- **対象範囲**: grain runtime の readiness 判定契約（純粋な導出）、ホスト環境から呼び出せる判定の公開手段（core の読み取りアクセサ）、grain / placement 領域の join 互換キーの目録整備（比較対象外の設定の除外理由の明示）
- **対象外**: 包括的な sharding 設定契約（後続スペックの責務）、grain / placement 設定の required な join 比較の追加（設定の config 所有化とともに後続の包括設定契約スペックが行う）、HTTP サーバ等の具体的な probe endpoint 実装（endpoint 配線は利用者側の責務）、metrics 公開、liveness（プロセス生存）判定、placement / activation の挙動変更
- **隣接システム／スペックへの期待**: 既存の join 互換チェック基盤（互換キー目録と join 時の合成評価、不一致時の join 拒否経路）が公開契約として利用できること。grain runtime の既存状態（kind 登録状態、placement 調整状態）と membership の自ノード状態が読み取りで参照できること。failure detector の互換キー（単一キー + 差分項目名の detail）が先行例として参照できること

## 要件

### 要件 1: grain runtime の readiness 判定契約

**目的:** クラスタ運用者として、ノードへトラフィックを流してよいかを外部システムから判断するために、grain runtime がメッセージを受けられる状態かを導出する判定契約が欲しい

#### 受け入れ基準

1. readiness 判定は常に、自ノードの membership 状態・placement 調整の状態・期待される kind の登録状態という観測可能な入力だけから結果を導出しなければならない
2. 自ノードの membership が稼働状態であり、placement 調整が解決可能な状態であり、かつ期待される kind がすべて登録済みである場合、readiness 判定は ready を返さなければならない
3. placement 調整が解決不能な状態の場合、readiness 判定は not ready を返し、原因を識別できる理由を提供しなければならない
4. 自ノードの membership が稼働状態でない場合、readiness 判定は not ready を返し、原因を識別できる理由を提供しなければならない
5. 期待される kind に未登録のものがある場合、readiness 判定は not ready を返し、原因を識別できる理由を提供しなければならない
6. 期待される kind の指定が空の場合、readiness 判定は kind の登録状態を ready の条件に含めてはならない
7. readiness 判定は常に、同一の入力に対して同一の判定結果を返さなければならない

### 要件 2: readiness 判定の外部公開

**目的:** クラスタ運用者として、readiness probe やロードバランサから判定を利用するために、ホスト環境（std を含む）から判定を呼び出せる公開手段が欲しい

#### 受け入れ基準

1. 利用者が公開手段を呼び出したとき、公開手段は呼び出し時点の runtime 状態を反映した判定入力の写しを返さなければならない
2. 公開手段が返した写しからの判定は常に、要件 1 の判定契約そのもので行われなければならず、ホスト層に独自の判定規則を置いてはならない
3. 公開手段は常に、判定結果の公開までを責務とし、HTTP サーバ等の具体的な probe endpoint 実装を含んではならない

### 要件 3: grain / placement 領域の join 互換キー整備

**目的:** クラスタ運用者として、join 時の設定互換チェックがどの範囲をカバーし、何が意図的に比較対象外なのかを判断できるようにするために、grain / placement 領域の互換キーが目録上で整備されてほしい

#### 受け入れ基準

1. 互換キー目録は常に、grain / placement 領域でノード間一致を要求しない現行設定（identity lookup の実装選択、キャッシュ容量・有効期限等のローカルチューニング値）を表すキーを、除外理由とともに識別できるようにしなければならない
2. grain / placement 領域の除外キーは常に、join 時の互換チェックの合成評価の対象に含まれてはならない
3. 将来 grain / placement 領域の join 互換キーの比較対象を拡張する場合、対象は常に、ノード間で一致しないと grain の配送・配置の正しさが壊れる設定だけに限定しなければならない

### 要件 4: 既存挙動の維持と範囲の限定

**目的:** クラスタ運用者として、既存機能の安定を保つために、本機能の追加が既存の配置・配送・join 判定の挙動を変えないことを保証したい

#### 受け入れ基準

1. 本機能は常に、placement / activation の既存挙動を変えずに維持しなければならない
2. 本機能は常に、既存の join 互換チェック（pubsub / downing / split brain resolver / failure detector / singleton）の評価結果を変えずに維持しなければならない
3. readiness 判定の契約は常に、ホスト環境の機能（I/O、時刻取得、ネットワーク）に依存せず導出を完結しなければならない
4. readiness 判定は常に、readiness（トラフィック受け入れ可否）だけを判定対象とし、liveness（プロセス生存）の判定を提供してはならない
