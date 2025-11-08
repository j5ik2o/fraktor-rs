# 要求ドキュメント

## Introduction
セルアクターのシリアライザは、Pekko と同じく集約型 `A { name: String, b: B }` のすべてのフィールドを自前で処理し、外部シリアライザ（例: serde）への暗黙委譲を排して一貫性とデバッグ容易性を確保する必要がある。ただし末端フィールドが外部シリアライザ依存を持たない純粋値型の場合は、Registry で明示的に `external_serializer_allowed` として許可された外部実装を最適化経路として利用する折衷方針を採用する。本仕様はネストフィールドの再帰処理、バインディング検証、観測性を網羅的に定義する。

## Requirements

### Requirement 1: ネストフィールドの再帰シリアライズ
**Objective:** アクター間で伝送される集約型の全フィールドを Serializer サブシステムが直接扱い、Pekko 互換のエンベロープを維持する。

#### Acceptance Criteria
1. When Serializer サブシステムがネストフィールドを含む集約型のシリアライズ要求を受信したとき, the Serializer サブシステム shall 再帰的にすべてのフィールドを走査し各フィールドの登録済みシリアライザでバイト列を生成する。
2. When the Serializer サブシステムが子フィールド `B` のシリアライザバインディングを解決したとき, the Serializer サブシステム shall その結果のバイト列を親フィールドと同一エンベロープ形式でラップして返信する。
3. If ネストフィールド型に対応するシリアライザがレジストリに存在しない場合, then the Serializer サブシステム shall フィールドパスと型名を含むバインディングエラーイベントを発行しシリアライズ処理を失敗させる。
4. While Pekko 互換モードが有効な間, the Serializer サブシステム shall 親子フィールドのエンコード順序とエンベロープヘッダを Pekko 参照テストベクタと一致させる。
5. Where ネストフィールドが外部シリアライザ依存を持たず Registry に `external_serializer_allowed` として登録されている場合, the Serializer サブシステム shall 許可済み末端フィールドに限り登録外部シリアライザ（serde など）を呼び出しつつ親エンベロープのヘッダと順序を維持する。
6. If `external_serializer_allowed` として登録されていないフィールドに対して外部シリアライザを呼び出そうとした場合, then the Serializer サブシステム shall 即座に処理を失敗させバインディングエラーを発行する。

### Requirement 2: バインディング登録と検証
**Objective:** Serializer Registry がネストフィールドの責務境界を明示し、起動時に欠落や衝突を検知できるようにする。

#### Acceptance Criteria
1. When 開発者が集約型 `A` のシリアライザを登録するとき, the Serializer Registry shall `A` が参照するすべてのフィールド型 `B_i` のバインディング宣言と、外部シリアライザを許可したい末端フィールドの `external_serializer_allowed` メタデータを必須とする。
2. When ActorSystem がブートストラップされたとき, the Serializer Registry shall すべてのネストフィールドバインディングが存在し衝突していないことを検証し, 検証失敗時は起動を停止する。
3. When フィールドが `external_serializer_allowed` として登録されたとき, the Serializer Registry shall その型が外部シリアライザ依存を持たない純粋値型であることと、親 Serializer がエンベロープ整合性を維持する設定になっていることを検証する。
4. If ネストフィールドバインディングの解決が循環参照や自己参照を検出した場合, then the Serializer Registry shall 即座にエラーを返し問題の型チェーンをログへ出力する。
5. While バインディング監査フラグが有効な間, the Serializer Registry shall 直近の検証結果を EventStream/Telemetry に発行し観測性を提供する。
6. Where 型が Pekko の `Serializable` マーカー特性と互換である場合, the Serializer Registry shall 参照実装と同じデフォルトシリアライザを割り当てる。

### Requirement 3: 観測性とフォールバック通知
**Objective:** ネストフィールドを自前で処理することによるパフォーマンス負荷と例外経路を可視化し、フォールバックが起きた場合のトレースを即座に得られるようにする。

#### Acceptance Criteria
1. When シリアライズ処理に所定の閾値を超える時間がかかったとき, the Telemetry Service shall ネストフィールドパスと処理時間を含むレイテンシイベントを記録する。
2. If Serializer サブシステムが `external_serializer_allowed` として登録されていないフィールドに対して外部シリアライザ（serde など）を呼び出そうとした場合, then the Telemetry Service shall フォールバックの理由と対象フィールドを DeadLetter/EventStream へ通知する。
3. Where フィールドが `external_serializer_allowed` として許可されている場合, the Telemetry Service shall 許可済み外部シリアライザ経路の呼び出し回数と処理時間をメトリクスで公開する。
4. While デバッグトレースモードが有効な間, the Serializer サブシステム shall 各ネストフィールドのサイズとバインディング名をトレースログへ出力する。
5. When バインディングエラーが発生したとき, the Telemetry Service shall 直近の ActorSystem メッセージとフィールドパスを添えて監視 API へ通知する。
6. The Telemetry Service shall ネストフィールドシリアライゼーションの成功/失敗カウンタと `external_serializer_allowed` 経路の成功/失敗内訳をメトリクスとして公開する。
