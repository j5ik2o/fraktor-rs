# 実装計画

- [x] 1. 基盤: 応答型付けの変換点を公開する
  - actor-core-typed の typed 応答（response / future）に公開コンストラクタ from_untyped を追加する（既存 pub(crate) 実体の公開化、取り出し契約は変更しない）
  - rustdoc に typed facade crate 向けの変換点である旨を明記し、#[must_use] を付与する
  - from_untyped で構築した typed 応答が既存の取り出し契約（try_take / TypedAskError）で動作する単体テストが green になる
  - _Requirements:_ 2.4, 2.5
  - _Boundary:_ TypedAskResponse/TypedAskFuture 公開コンストラクタ
  - _Depends:_ none

- [ ] 2. コア: typed 宣言点と typed 参照
- [x] 2.1 (P) grain 種別の typed 宣言点を実装する
  - kind とメッセージ型の対応を一箇所で宣言し、entity id を与えて typed 識別を導出できるようにする
  - 識別の検証は kernel の検証規則へ委譲し、不正な kind / entity id は既存のエラー契約（ClusterIdentityError）で拒否される
  - 宣言点の構築が Result を返すか infallible とするかは、kernel の kind 検証規則を単体で再利用できるかで実装時に決定する（いずれの場合も不正識別の拒否が契約上の要点）
  - 導出した typed 識別が既存 typed 識別の直接構築と同値であることを検証する sibling テストが green になる
  - _Requirements:_ 1.1, 1.2, 4.3
  - _Boundary:_ GrainTypeKey<M>
  - _Depends:_ none

- [ ] 2.2 (P) typed grain 参照と型安全な呼び出しを実装する
  - メッセージ型でパラメータ化された grain 参照を提供し、型付き tell / request / request_future、識別の参照、呼び出しオプション・codec のパススルーを実装する
  - request / request_future の応答型付けには、タスク 1 で公開した from_untyped（TypedAskResponse / TypedAskFuture）を使用する
  - untyped との明示的相互変換（from_kernel / as_kernel / into_kernel）を提供し、From / Into による暗黙変換は実装しない
  - 誤った型の送信がコンパイルエラーになることを rustdoc compile_fail doctest で検証する
  - 往復変換で宛先（kind と entity id）が保持され、M 違いの識別が同一 kernel 宛先になり、呼び出しオプション・codec が kernel へパススルーされることを検証する sibling テストが green になる
  - _Requirements:_ 1.3, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 3.3, 4.1, 4.2, 4.3, 4.4
  - _Boundary:_ GrainRef<M>（typed）
  - _Depends:_ 1

- [ ] 3. 統合: typed システムからの取得経路を配線する
  - typed Cluster facade に typed 識別から grain 参照を構築する取得点を追加し、モジュール配線（公開面）を整える
  - 実装制約: GrainRef<M>（typed）境界への追加実装のみとし、Cluster の既存メソッドシグネチャを変更しない（事後の非回帰確認は 4.2 が所有）
  - cluster 拡張未導入時は既存の取得失敗契約（ExtensionNotInstalled）がそのまま適用される
  - typed ActorSystem → Cluster 取得 → grain 参照構築の経路が sibling テストで green になる
  - _Requirements:_ 3.1, 3.2, 5.4
  - _Boundary:_ Cluster facade 拡張
  - _Depends:_ 2.1, 2.2

- [ ] 4. 検証
- [ ] 4.1 取得経路と呼び出し往復の統合テストを追加する
  - 拡張導入済みシステムで宣言点 → 取得 → request 往復が typed 応答を返すことを検証する
  - 拡張未導入システムで取得が ClusterApiError::ExtensionNotInstalled で拒否されることを検証する
  - tell の送達、request_future の応答、失敗伝搬（宛先解決失敗 → GrainCallError::ResolveFailed、応答型不一致 → TypedAskError::TypeMismatch）を検証する
  - 呼び出しオプション・codec のパススルー検証は 2.2 の sibling テストが所有するため、本タスクでは扱わない
  - cluster-core-typed の統合テスト（tests/grain.rs）が green になる
  - _Requirements:_ 2.3, 2.4, 2.5, 2.6, 3.1, 3.2
  - _Boundary:_ 統合（cluster-core-typed の crate 統合テスト）
  - _Depends:_ 3

- [ ] 4.2 非回帰と範囲限定を確認する
  - cluster-core-kernel に差分がないことを確認する
  - cluster-core-typed / actor-core-typed の既存テストが無変更で green になる（3 の実装制約の事後確認を含む）
  - lifecycle / 配置 / envelope / extractor / behavior factory を導入していないことを確認する
  - 対象 crate の targeted check（cargo test -p、./scripts/ci-check.sh clippy / dylint <対象>）が exit 0 で通過する
  - _Requirements:_ 5.1, 5.2, 5.3, 5.4
  - _Boundary:_ 全体検証
  - _Depends:_ 4.1
