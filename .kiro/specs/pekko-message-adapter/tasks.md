# Implementation Plan

- [ ] 1. MessageAdapterRegistry と型消去コンポーネントを実装して型安全な登録基盤を作る
  - 登録・解除・逆順検索・全削除というコア操作をまとめた単一レジストリを TypedActorAdapter 内で管理し、未登録時にも deterministic に動作するよう整える
  - _Requirements: 要件1.2, 要件2.1_
- [ ] 1.1 レジストリの登録／置換ロジックを実装する
  - `TypeId` ごとにエントリを差し替えながら保持し、同一型の再登録で既存クロージャを確実に置換する
  - `AdaptWithRegisteredMessageAdapter` が届いたときに逆順検索で最初のマッチだけを実行し、未マッチの場合は `ActorCell::unhandled` へフォールバックする
  - _Requirements: 要件1.2, 要件2.1, 要件2.3_
- [ ] 1.2 AdapterPayload と AdapterFnErased を整備してクロージャ評価を型安全化する
  - 所有権付き `AdapterPayload` から `TypeId` を取得し、downcast に失敗した場合は `AdapterError::TypeMismatch` を返す
  - `AdapterFnErased` が Actor スレッド上でのみ `Fn(U) -> Result<M, AdapterFailure>` を起動し、結果に応じて `MessageAdaptionFailure` を生成する
  - _Requirements: 要件2.2, 要件3.1_

- [ ] 2. AdapterRefSender と ActorCell の FunctionRef ライフサイクルを統合する
  - 親アクター直下に 1 つの匿名 FunctionRef を生成して再利用し、`AdapterRefSender` が Registered メッセージを `AdapterEnvelope::Registered` に包んでメールボックスへ投入する
  - _Requirements: 要件1.1, 要件1.3_
- [ ] 2.1 AdapterRefSender の生成と停止検知を実装する
  - Dispatcher ハンドルと `WeakShared` の番兵で親アクターの停止を検知し、停止後は即座に `SendError::TargetStopped` と DeadLetter 記録を返す
  - `AdapterEnvelope` へ書き込む `type_id` を `AdapterPayload` と同期させ、不整合時は `AdapterError::EnvelopeCorrupted` を報告する
  - _Requirements: 要件1.3, 要件2.1, 要件3.1_
- [ ] 2.2 ActorCell 側で FunctionRef を登録・破棄できるようにし、停止フローへ組み込む
  - `register_adapter_ref` がハンドル ID を払い出して `adapter_handles` に保持し、`drop_adapter_refs` で Dispatcher ハンドルと DeadLetter をクリーンアップする
  - PostStop/PreRestart で MessageAdapterRegistry を破棄し、再起動後に再登録されるまで残骸を残さない
  - _Requirements: 要件1.4, 要件3.3_

- [ ] 3. TypedActorContext API を拡張して開発者向けの登録面を整える
  - `message_adapter`／`spawn_message_adapter`／`ask` がレジストリや `AdaptMessage` と自然に連携し、親アクターの実行コンテキスト（メッセージ処理中は単一スレッドで実行される環境）で結果を評価できるようにする
  - _Requirements: 要件1.1, 要件2.4_
- [ ] 3.1 message_adapter / spawn_message_adapter の委譲レイヤーを実装する
  - API から `MessageAdapterRegistry` への登録・置換を一手に引き受け、戻り値の Typed ActorRef がアクターの実行コンテキスト内でのみクロージャを実行することを保証する
  - 既存の `_messageAdapters` 互換リストを移行し、On-the-fly 再登録でも unbounded growth が起きないことを検証する
  - _Requirements: 要件1.1, 要件1.2_
- [ ] 3.2 AdaptMessage を用いた ask/pipe_to_self 経路を整備する
  - Future/CompletionStage 完了時に `AdaptMessage { value, adapter }` を自分自身へ送るヘルパを実装し、親アクターが自身のメッセージ処理コンテキスト内でレスポンス変換クロージャを評価できるようにする
  - 成功・失敗の両経路で `MessageAdaptionFailure` と SupervisorStrategy へ橋渡しする共通ハンドラを追加する
  - _Requirements: 要件2.2, 要件2.4, 要件3.1_

- [ ] 4. TypedActorAdapter と BehaviorRunner を拡張してルーティングと失敗伝搬を統合する
  - `AdapterEnvelope::Registered` を検出したらレジストリへ委譲し、成功時は `BehaviorRunner` へ `M` を渡し、未マッチ時は DeadLetter → EventStream → `ActorCell::unhandled` の順にフォールバックする
  - _Requirements: 要件2.1, 要件2.3_
- [ ] 4.1 BehaviorSignal と EventStream で `MessageAdaptionFailure` を伝搬する
  - `BehaviorSignal::AdapterFailed` を追加し、`system.event_stream()` へ `MessageAdaptionFailure` を publish してから supervisor へ例外を再スローする
  - PostStop/PreRestart でレジストリと FunctionRef をクリアし、監督戦略が deterministically 動くようにする
  - _Requirements: 要件3.1, 要件3.2, 要件3.3_

- [ ] 5. 回帰テストと性能検証で全要件をカバーする
  - レジストリ置換、AdapterRefSender の停止検知、AdaptMessage 経路、未マッチ時の DeadLetter を網羅するユニット／統合テストを追加する
  - 32 個のアダプタと 1M msg/s シナリオでメモリ確保と待ち行列の健全性を確認し、`MessageAdaptionFailure` と監督シグナルの観測パスを検証する
  - _Requirements: 要件1.1〜1.4, 要件2.1〜2.4, 要件3.1〜3.3_
