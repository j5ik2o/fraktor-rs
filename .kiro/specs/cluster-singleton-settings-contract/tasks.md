# 実装計画

- [x] 1. singleton モジュールを新設し、設定検証エラーの語彙を定義する
  - cluster-core-kernel に singleton 専用モジュールの wiring を追加し、ワークスペースの公開モジュールとして到達可能にする
  - 検証失敗の原因項目（空 singleton 名 / buffer size 範囲外 / ゼロ以下の handover リトライ間隔 / ゼロ以下の identification 間隔 / 空 lease 実装名 / ゼロ以下の lease リトライ間隔）を特定できるエラー enum と英語 Display を定義する
  - 完了条件: ワークスペースがコンパイルでき、各 variant の Display が原因項目を含むことをテストで検証できる
  - _Requirements:_ 4.2, 4.3, 4.4, 4.5
  - _Boundary:_ singleton モジュール wiring, ClusterSingletonSettingsError
  - _Depends:_ none

- [ ] 2. コア: 設定契約
- [x] 2.1 lease 設定スロットを実装する
  - lease 実装の識別名と lease リトライ間隔の 2 項目のみを保持する設定型を定義する（SBR の lease 語彙とは結合しない）
  - 空の実装名とゼロ以下のリトライ間隔を検証で拒否する
  - 意味のある既定値が存在しないため Default は提供せず、スロット未指定は保持側の Option で表現する前提を守る
  - 完了条件: 2 項目の保持・取得と検証拒否 2 種の単体テストが通る
  - _Requirements:_ 1.4, 4.5
  - _Boundary:_ LeaseUsageSettings
  - _Depends:_ none

- [x] 2.2 manager 設定契約とリトライ上限導出を実装する
  - singleton 名・role・removal margin・handover リトライ間隔・最小リトライ回数・lease スロットを保持し、Pekko 互換の既定値（"singleton" / role なし / margin 未指定 / 1 秒 / 15 回 / lease なし）を適用する
  - removal margin の未指定は明示値と型で区別できる形（Option）で保持する
  - 検証で空 singleton 名とゼロ以下の handover リトライ間隔を拒否し、lease 保持時は lease 検証へ委譲する
  - リトライ上限を「最小リトライ回数と margin ÷ リトライ間隔 + 3 の大きい方」として決定的に導出する全域関数を提供する（ゼロ間隔でも panic しない）
  - 互換性確認用に他設定との差異フィールド名列挙を提供する
  - 完了条件: 既定値・Option 区別・検証拒否・導出の決定性（margin なし→15、margin 26s/間隔 1s→29）・差異列挙の単体テストが通る
  - _Requirements:_ 1.1, 1.2, 1.3, 1.4, 4.3, 4.4, 7.1
  - _Boundary:_ ClusterSingletonManagerSettings
  - _Depends:_ 2.1

- [x] 2.3 (P) proxy 設定契約を実装する
  - singleton 名・role・data center・identification 間隔・buffer size を保持し、既定値（"singleton" / role なし / DC なし / 1 秒 / 1000）を適用する
  - data center 項目は membership の既存 DataCenter 型を再利用する
  - 検証で空 singleton 名・ゼロ以下の identification 間隔・10000 超の buffer size を拒否し、buffer size 0 は「バッファリングなし」として受理する
  - 互換性確認用に差異フィールド名列挙を提供する
  - 完了条件: 既定値・buffer size 0 受理・検証拒否 3 種・差異列挙の単体テストが通る
  - _Requirements:_ 2.1, 2.2, 2.3, 4.2, 4.3, 4.4
  - _Boundary:_ ClusterSingletonProxySettings
  - _Depends:_ none

- [x] 2.4 typed 統合設定と manager / proxy への導出を実装する
  - cluster-core-typed に、manager / proxy 共通項目（role・data center・identification 間隔・removal margin・handover リトライ間隔・最小リトライ回数・buffer size・lease スロット)を一括指定できる統合設定を追加する
  - singleton 名を与えて manager 設定・proxy 設定を導出し、対応項目の値を変化させずに引き継ぐ
  - 既定の統合設定からの導出が kernel の既定値と一致することを保証する
  - 検証は導出先の検証に委ね、typed 側で二重定義しない
  - typed クレートの wiring に mod 宣言と pub use を追加する（module-wiring 規約に従う）
  - 完了条件: 非既定値での導出無損失と既定値一致の単体テストが通り、typed の公開 API として到達可能になる
  - _Requirements:_ 3.1, 3.2, 3.3, 3.4
  - _Boundary:_ ClusterSingletonSettings
  - _Depends:_ 2.2, 2.3

- [ ] 3. 観測契約
- [x] 3.1 (P) stuck 局面の語彙を定義する
  - 停滞の局面（oldest への昇格待ち / handover 実行中）を表す 2 値 enum を singleton モジュールに追加する
  - 完了条件: 2 variant の等価比較テストが通り、singleton モジュールから公開される
  - _Requirements:_ 7.2
  - _Boundary:_ SingletonStuckPhase
  - _Depends:_ none

- [ ] 3.2 stuck イベント variant と購読フィルタを追加する
  - cluster イベント語彙に stuck 通知（singleton 名・停滞局面・観測時刻）を既存 variant のフィールド規約に合わせて追加する
  - イベント種別フィルタに対応種別を追加し、種別照合の網羅 match を更新する
  - rustdoc に観測専用契約（通知を契機とした membership 遷移・down 判断の禁止）を明記し、この variant を消費するハンドラは追加しない
  - 既存のイベント種別は削除・変更しない
  - 完了条件: ワークスペースがコンパイルでき、新種別の種別照合テストが通る
  - _Requirements:_ 7.2, 7.3, 7.4, 8.3
  - _Boundary:_ ClusterEvent / ClusterEventType 拡張
  - _Depends:_ 3.1

- [ ] 4. 統合: 検証境界と参加互換性
- [ ] 4.1 (P) 互換キーカタログに singleton キーを追加する
  - cluster.singleton を required キーとしてカタログに追加し、required キー一覧に組み込む
  - カタログには sibling テストファイルが未存在のため、新規作成してテスト紐づけを追加する
  - 完了条件: 新キーが required 一覧に含まれることをテストで検証できる
  - _Requirements:_ 5.1
  - _Boundary:_ ClusterCompatibilityKeyCatalog
  - _Depends:_ none

- [ ] 4.2 cluster 拡張設定へ singleton 設定を統合し互換チェックを配線する
  - manager / proxy 設定を既定値内包のフィールドとして cluster 拡張設定に追加し、setter / getter を既存の規約に合わせて提供する（既存 API のシグネチャは変更しない）
  - singleton 専用の検証メソッドを追加し、manager → proxy の順に検証を委譲する
  - 参加互換性チェック配列に singleton エントリを追加し、不一致時は manager. / proxy. プレフィックス付きの差異フィールド名を理由として生成し、一致時は不一致理由を生成しない
  - 完了条件: 設定の保持・既定値・検証委譲・不一致理由生成（差異あり / なし）のテストが通り、既存の互換チェックテストが無変更で通る
  - _Requirements:_ 5.1, 5.2, 5.3, 6.1, 6.2, 6.3, 8.3
  - _Boundary:_ ClusterExtensionConfig（統合タスク）
  - _Depends:_ 2.2, 2.3, 4.1

- [ ] 4.3 install 境界で singleton 検証を実行する
  - installer の既存検証の直後に singleton 検証を追加し、失敗を同じ構成エラーへ写像する（検証の Result は握りつぶさない）
  - すべての設定が成立する場合は install を継続する
  - 完了条件: 不正な singleton 設定で install が構成エラーになり、既定値設定で install が成立する統合テストが通る
  - _Requirements:_ 4.1, 4.2, 4.6, 6.1, 6.2
  - _Boundary:_ ClusterExtensionInstaller
  - _Depends:_ 4.2

- [ ] 4.4 (P) stuck 通知の購読識別を統合検証する
  - EventStream 経由でテスト発行した stuck 通知を、対応種別フィルタの購読者だけが受信し、他種別フィルタの購読者は受信しないことを cluster の購読 API 経由で検証する
  - 完了条件: 購読フィルタの統合テストが通る
  - _Requirements:_ 7.3
  - _Boundary:_ ClusterApi 購読テスト
  - _Depends:_ 3.2, 4.2

- [ ] 5. 非回帰と範囲限定を検証する
  - cluster 3 クレート（kernel / typed / adaptor-std）の既存テストが無変更で通ることを確認する
  - singleton モジュールの公開 API が純粋データ型のみで、runtime 動作（oldest 選出・handover 実行・バッファリング）を提供していないことを公開面で確認する
  - 対象範囲の lint（clippy / dylint）を実行し、新規違反がないことを確認する
  - 完了条件: 対象クレートのテストと lint がすべて成功する
  - _Requirements:_ 8.1, 8.2, 8.3
  - _Boundary:_ 検証（全コンポーネント横断）
  - _Depends:_ 4.3, 4.4

## Implementation Notes

- 2.3: kernel は `#![deny(clippy::missing_const_for_fn)]`。`Option<&T>` を返す getter（`as_ref()` 利用）も `pub const fn` にすること（2.3 のレビュー rejection の原因）
