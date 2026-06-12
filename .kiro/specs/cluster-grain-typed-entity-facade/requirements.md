# 要件定義

## はじめに

cluster の typed 層に、メッセージ型でパラメータ化された型安全な Grain（virtual actor）参照の契約を定義する。現状の typed 層には membership 向けの Cluster facade しかなく、grain を使う typed actor のコードは型消去された untyped の識別と参照を直接扱うため、誤った型のメッセージ送信をコンパイル時に防げない。

対象は Pekko の `EntityTypeKey[M]` / `EntityRef[M]` に相当する typed grain 識別・typed grain 参照、型安全な fire-and-forget 送信と応答待ち呼び出し、typed ActorSystem からの取得経路（Pekko `ClusterShardingSetup` 相当の最小面）、および untyped API との明示的な相互変換である。actor-core の typed 層が確立した「薄い typed facade が untyped kernel を包み、ロジックは kernel 側に置く」パターンを踏襲し、kernel 側の挙動は変更しない。gap analysis カテゴリ 8 の easy 項目に対応する。

## 境界コンテキスト

- **対象範囲**: メッセージ型でパラメータ化された grain 識別と grain 参照、型安全な fire-and-forget 送信・応答待ち呼び出し・非同期応答呼び出し、typed ActorSystem からの取得経路（setup 統合の最小面）、untyped API との明示的な相互変換、呼び出しオプションと message codec の typed 層からの指定
- **対象外**: typed behavior factory（Pekko `Entity[M, E]` / `EntityContext` 相当）、grain の lifecycle（activation / passivation）と配置決定の変更、message envelope / extractor SPI（cluster-sharding-extractor-contract が所有）、message serialization の変更
- **隣接システム／スペックへの期待**: 既存の untyped grain API（識別・参照・codec）が公開契約として利用できること。actor-core typed 層の wrapper パターン（typed actor 参照・typed ActorSystem）が先行例として参照できること。cluster-sharding-extractor-contract は本契約の typed 識別を前提として envelope 契約を定義できること

## 要件

### 要件 1: 型付き Grain 識別契約

**目的:** typed actor 利用者として、grain の宛先を受信メッセージ型と紐づけて宣言するために、メッセージ型でパラメータ化された grain 識別が欲しい

#### 受け入れ基準

1. typed grain facade は常に、kind と entity id の組で構成される grain 識別をメッセージ型でパラメータ化して表現できなければならない
2. kind または entity id が識別として不成立（空文字等）の場合、typed grain facade は識別の構築を拒否し、原因を特定できる理由を返さなければならない
3. typed grain facade は常に、同一の kind と entity id を持つ識別を、パラメータ化されたメッセージ型の如何にかかわらず untyped 層では同一の grain 宛先として扱わなければならない

### 要件 2: 型付き Grain 参照と型安全な呼び出し

**目的:** typed actor 利用者として、誤った型のメッセージ送信をコンパイル時に防ぐために、メッセージ型でパラメータ化された grain 参照が欲しい

#### 受け入れ基準

1. typed grain facade は常に、メッセージ型でパラメータ化された grain 参照を提供しなければならない
2. 利用者が grain 参照へメッセージを送るとき、typed grain facade はパラメータ化されたメッセージ型のみを受け付け、それ以外の型の送信をコンパイル時に拒否しなければならない
3. fire-and-forget 送信（tell 相当）が起きたとき、typed grain facade は応答を待たずに送信し、送信が失敗した場合は失敗理由を呼び出し元へ返さなければならない
4. 応答待ち呼び出し（request 相当）が起きたとき、typed grain facade は応答または失敗理由を呼び出し元へ返さなければならない
5. 非同期応答呼び出し（request_future 相当）が起きたとき、typed grain facade は応答を将来値として呼び出し元へ返さなければならない
6. 宛先解決・呼び出し・メッセージ符号化のいずれかが失敗した場合、typed grain facade は失敗の種類を区別できる理由として呼び出し元へ返さなければならない
7. typed grain facade は常に、grain 参照からその識別（kind と entity id）を参照できなければならない

### 要件 3: typed システムからの取得経路

**目的:** typed actor 利用者として、untyped API に触れずに grain 参照を得るために、typed ActorSystem からの取得経路が欲しい

#### 受け入れ基準

1. 利用者が typed ActorSystem から grain 参照の取得を要求したとき、typed grain facade はメッセージ型でパラメータ化された識別から grain 参照を構築して返さなければならない
2. cluster 拡張が未導入の ActorSystem に対して取得が要求された場合、typed grain facade は取得を拒否し、原因を特定できる理由を返さなければならない
3. 利用者が呼び出しオプション（タイムアウト等）や message codec を指定する場合、typed grain facade は untyped 層と同等の指定手段を typed 層から提供しなければならない

### 要件 4: untyped API との相互変換

**目的:** 既存の untyped grain API 利用者として、段階的に typed API へ移行するために、typed と untyped の明示的な相互変換が欲しい

#### 受け入れ基準

1. 利用者が typed grain 参照から untyped 参照への変換を要求したとき、typed grain facade は同一の宛先を指す untyped 参照を返さなければならない
2. 利用者が untyped 参照から typed 参照への変換を明示的に要求したとき、typed grain facade はメッセージ型を指定した typed 参照を構築して返さなければならない
3. typed grain facade は常に、typed と untyped の相互変換を明示的な操作としてのみ提供し、暗黙の変換を提供してはならない
4. 往復変換（typed から untyped を経て typed）が起きたとき、typed grain facade は同一の宛先（kind と entity id）を保持しなければならない

### 要件 5: 既存挙動の維持と範囲の限定

**目的:** cluster 利用者として、既存機能の安定を保つために、本契約の追加が untyped kernel の挙動を変えないことと、隣接スペックの責務を先取りしないことを保証したい

#### 受け入れ基準

1. typed grain facade は常に、既存の untyped grain API（識別・参照・codec）の公開契約と挙動を変えずに維持しなければならない
2. typed grain facade は本契約の範囲で grain の lifecycle（activation / passivation）および配置決定の挙動を変更してはならない
3. typed grain facade は本契約の範囲で typed behavior factory（`Entity[M, E]` / `EntityContext` 相当）および message envelope / extractor の契約を提供してはならない
4. typed grain facade は常に、既存の typed Cluster facade の公開契約と挙動を変えずに維持しなければならない
