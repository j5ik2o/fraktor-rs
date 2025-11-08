# Implementation Plan

- [x] 1. MessageAdapterRegistryを構築して型変換の土台を整える
  - Typedアクターごとのレジストリ所有モデルと初期化/クリア手順をまとめ、空状態でも determinism を保つようにする
  - TypeIdベースのエントリ構造と逆順探索ポリシーを固め、後続のアダプタ追加が一定順序で解決されるようにする
  - AdapterOutcome と DeadLetter フローを整理し、例外や未マッチ時の制御を一貫化する
  - _Requirements: 要件1.2, 要件2.1, 要件2.3, 要件3.1_

- [x] 1.1 レジストリ登録／置換ロジックを実装する
  - TypeId をキーに既存エントリを置き換えて unbounded growth を防ぎ、登録順の優先度を保持する
  - 逆順探索で最初にマッチした変換のみを実行し、残りのエントリを安全にスキップする走査処理を整備する
  - 未マッチ時に ActorCell の unhandled・DeadLetter 経路へ即座にフォールバックできる通知フローをつくる
  - _Requirements: 要件1.2, 要件2.1, 要件2.3_

- [x] 1.2 AdapterPayload と AdapterFnErased を整備してクロージャ評価を型安全化する
  - 所有権付きペイロードから TypeId を抽出し、クロージャに渡す前に downcast を検証する仕組みを追加する
  - 親アクターと同一スレッドでクロージャを実行できる実行ガードを設け、失敗時は AdapterFailure を生成する
  - TypeMismatch や EnvelopeCorrupted を区別し、DeadLetter へ正しい理由を記録できるようにする
  - _Requirements: 要件2.2, 要件2.4, 要件3.1_

- [x] 2. AdapterRefSender と ActorCell の FunctionRef を統合してライフサイクルを制御する
  - 親アクター配下に共有の匿名 FunctionRef を生成・再利用する方針を固め、停止時のリソースリークを防ぐ
  - AdapterRefSender が停止済みの親を検知し DeadLetter に記録する観測ポイントを整える
  - 再起動/停止時に ActorCell がレジストリと FunctionRef をクリーンアップする手続きを定義する
  - _Requirements: 要件1.1, 要件1.3, 要件1.4, 要件3.3_

- [x] 2.1 AdapterRefSender の生成と停止検知を実装する
  - Dispatcher ハンドルと Weak 参照を束ねて親アクターのライフサイクルを監視し、停止後の送信を即座に失敗させる
  - AdapterEnvelope へ記録する type_id と Payload 由来の type_id を同期させ、齟齬時には EnvelopeCorrupted を返す
  - DeadLetter へ「adapter-ref-stopped」を出力して外部送信者が失敗理由を観測できるようにする
  - _Requirements: 要件1.1, 要件1.3, 要件2.1, 要件3.1_

- [x] 2.2 ActorCell 側で FunctionRef を登録・破棄するフローを実装する
  - AdapterRef ハンドルの登録/解除 API を整備し、PostStop/PreRestart で確実にハンドルを破棄する
  - ハンドル破棄時に残存メッセージを DeadLetter へ移送する経路を組み込み、監督下での determinism を維持する
  - レジストリの clear と連動して再起動後の再登録をクリーンに開始できるようにする
  - _Requirements: 要件1.4, 要件2.3, 要件3.3_

- [x] 3. TypedActorContext API を拡張し、開発者が MessageAdapter を登録できるようにする
  - message_adapter / spawn_message_adapter の API 契約を整備し、親アクターが返す AdapterRef を型安全に公開する
  - アダプタ登録時のライフサイクル管理や既存レジストリ互換の挙動をまとめる
  - ask/pipe_to_self のレスポンス変換フローと MessageAdapterRegistry を連携させる
  - _Requirements: 要件1.1, 要件1.2, 要件2.4_

- [x] 3.1 message_adapter / spawn_message_adapter の委譲レイヤーを実装する
  - TypedActorContext からレジストリへの登録を一手に引き受けるハンドルを導入し、クロージャ再登録時の置換を保証する
  - AdapterRef の返却時に ActorContext 内でのみクロージャを実行する制約を明示し、利用者 API の契約を固める
  - 最初のアダプタ登録時に FunctionRef を起動し、以後の登録では再利用されることを確認する
  - _Requirements: 要件1.1, 要件1.2, 要件1.3_

- [ ] 3.2 AdaptMessage を用いた ask / pipe_to_self 経路を整備する
  - Future/CompletionStage の完了値を AdaptMessage エンベロープに包み、親アクター自身に送り返すフローを構築する
  - 変換クロージャの成功と失敗を AdapterOutcome へ統一し、MessageAdaptionFailure を supervisor へ橋渡しする
  - 同期/非同期どちらの呼び出しでも親スレッド上で変換が完了することを検証する
  - _Requirements: 要件2.2, 要件2.4, 要件3.1_

- [x] 4. TypedActorAdapter と BehaviorRunner を拡張してルーティングと失敗伝搬を統合する
  - AdapterEnvelope を検出してレジストリへ委譲する受信フローを構築し、通常メッセージとの優先順位を整理する
  - AdapterOutcome の結果を BehaviorRunner へ伝搬し、成功時はユーザビヘイビアへ、失敗時はシグナル経由で supervisor へ渡す
  - 再起動・停止時にレジストリ/FunctionRef をクリアし、監督戦略と整合するようにする
  - _Requirements: 要件2.1, 要件2.3, 要件3.1, 要件3.2, 要件3.3_

- [x] 4.1 AdapterEnvelope の受信とルーティングを実装する
  - TypedActorAdapter がユーザメッセージと AdapterEnvelope を判別し、レジストリの adapt 処理へ委譲する
  - 成功時に M 型メッセージへ変換して BehaviorRunner へ渡し、未マッチ時は ActorCell.unhandled を呼び出す
  - EnvelopeCorrupted を検出した場合に DeadLetter へ告知してメッセージを破棄する
  - _Requirements: 要件2.1, 要件2.3_

- [x] 4.2 MessageAdaptionFailure シグナルと EventStream 連携を実装する
  - 変換クロージャ失敗時に BehaviorSignal::AdapterFailed を生成し、EventStream へも同等のイベントを配信する
  - 既定ハンドラが例外を再スローして supervisor に伝えるフローと、ユーザビヘイビアが信号を処理できるフックを整備する
  - PostStop/PreRestart でレジストリと AdapterRef をクリアし、再起動後にクリーンな状態で再登録できるようにする
  - _Requirements: 要件3.1, 要件3.2, 要件3.3_

- [ ] 5. 品質検証と性能チェックで全要件を裏付ける
  - レジストリ置換、AdapterRef 停止検知、AdaptMessage 経路、DeadLetter フォールバックといった主要シナリオを網羅する
  - 高負荷シナリオでの adapter 連投に耐えることを確認し、監督イベントが観測できるようメトリクスを確認する
  - _Requirements: 要件1.1〜1.4, 要件2.1〜2.4, 要件3.1〜3.3_

- [ ] 5.1 ユニットテストを追加して基礎挙動を固定する
  - MessageAdapterRegistry, AdapterPayload, AdapterRefSender の happy/edge ケースをそれぞれ個別テストで検証する
  - TypeMismatch や EnvelopeCorrupted の失敗パスを明示し、DeadLetter 記録内容をアサートする
  - ask/pipe_to_self の AdaptMessage ハンドリングがスレッド拘束を守ることをテストする
  - _Requirements: 要件1.1〜1.3, 要件2.1〜2.4, 要件3.1_

- [ ] 5.2 統合テストで TypedActorAdapter と BehaviorRunner を検証する
  - 複数 adapter を登録して優先順位・置換・未マッチ時の DeadLetter をシナリオ駆動で確認する
  - アクター停止/再起動シーケンスを通し、AdapterFailure シグナルとレジストリ再登録が期待通りに動くか確かめる
  - 高頻度で MessageAdaptionFailure を発生させ、SupervisorStrategy が再起動/停止を選択する挙動を観測する
  - _Requirements: 要件1.4, 要件2.1〜2.4, 要件3.1〜3.3_
