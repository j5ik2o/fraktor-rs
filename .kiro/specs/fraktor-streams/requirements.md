# 要件ドキュメント

## 導入
fraktor-streams は fraktor-rs にストリーム処理の基盤を追加し、no_std のコアと std の拡張を分離した形で提供する。Pekko Streams 相当の概念（Source/Flow/Sink と Materializer）を最小限の API として定義し、バックプレッシャ・エラー伝播・完了通知を一貫したルールで扱えることを目的とする。std 環境では fraktor-actor の ActorSystem 上で実行できることを前提にし、remote/cluster が有効な環境でも制約なく利用できることを前提にする。

## 要件

### 1. コアストリーム API
**目的:** ランタイム利用者として Source/Flow/Sink を組み合わせたストリーム処理を記述し、型安全にデータを流したい。

#### 受け入れ条件
- 1.1 ストリーム構成要素が定義されたとき、fraktor-streams は Source/Flow/Sink の組み合わせを型安全に表現しなければならない
- 1.2 無効な接続（入力/出力型が一致しない）ならば、fraktor-streams はコンパイル時に不成立としなければならない
- 1.3 ストリームが構成されている間、fraktor-streams は構成要素間の接続関係を保持し続けなければならない
- 1.4 Source/Flow/Sink は DSL の入口として、Pekko Streams に準拠した最小の基本コンビネータを提供しなければならない
  - 1.4.1 `via` と `to` により Source/Flow/Sink の合成ができなければならない
  - 1.4.2 `map` により基本的な変換ができなければならない
  - 1.4.3 `flatMapConcat` によりストリームの逐次連結ができなければならない
  - 1.4.4 Source は `single` により単一要素を生成できなければならない
  - 1.4.5 Sink は `ignore`/`fold`/`head`/`last`/`foreach` を提供しなければならない
  - 1.4.6 Source に Flow を合成した結果は Source であり、Sink に Flow を合成した結果は Sink でなければならない
- 1.5 fraktor-streams は Pekko の GraphStage を中核抽象として採用し、実行基盤は GraphStage を直接扱わなければならない
  - 1.5.1 GraphStage は shape と処理ロジックを定義できなければならない
  - 1.5.2 DSL/Graph は GraphStage を生成・合成できなければならない
  - 1.5.3 GraphStage のマテリアライズ値は MatCombine の規則に従って合成されなければならない
  - 1.5.4 GraphStage の公開 API は actor 型を露出してはならない
  - 1.5.5 組み込みステージ（map/flatMapConcat/ignore 等）は GraphStage として表現されなければならない

### 2. グラフ合成とマテリアライズ値
**目的:** ランタイム利用者として複数のストリームを合成し、マテリアライズ結果を一貫した規則で取得したい。

#### 受け入れ条件
- 2.1 複数のストリームが合成されたとき、fraktor-streams は合成後のグラフを単一の実行単位として扱わなければならない
- 2.2 ストリームがマテリアライズされたとき、fraktor-streams は合成規則に基づいてマテリアライズ値を返さなければならない
- 2.3 合成規則が定義されている間、fraktor-streams は同一の構成に対して同一のマテリアライズ結果を返し続けなければならない

### 3. Materializer のライフサイクル
**目的:** ランタイム利用者として Materializer を通じてストリーム実行を開始・停止し、実行中の状態を制御したい。

#### 受け入れ条件
- 3.1 Materializer が起動されたとき、fraktor-streams はストリーム実行を開始できるようにしなければならない
- 3.2 Materializer の停止が要求されたとき、fraktor-streams は実行中のストリームを停止できるようにしなければならない
- 3.3 Materializer が有効である間、fraktor-streams はストリームの実行状態を一貫したルールで管理し続けなければならない
- 3.4 Materializer は拡張可能でなければならず、ActorMaterializer 以外の実装（embedded/WASM など）を後から追加できなければならない

### 4. バックプレッシャと需要制御
**目的:** ランタイム利用者として上流と下流の速度差を吸収し、過負荷やメモリ過剰使用を防ぎたい。

#### 受け入れ条件
- 4.1 下流が需要を出したとき、fraktor-streams は上流へ需要情報を伝播しなければならない
- 4.2 下流が需要を出していないならば、fraktor-streams は上流からのデータ生成を抑止しなければならない
- 4.3 バックプレッシャが有効な間、fraktor-streams は過剰なバッファ消費を抑制し続けなければならない

### 5. 完了・キャンセル・エラー伝播
**目的:** ランタイム利用者としてストリームの完了や失敗を明確に把握し、復旧や停止の判断を行いたい。

#### 受け入れ条件
- 5.1 ストリームが正常完了したとき、fraktor-streams は完了を下流に通知しなければならない
- 5.2 失敗が発生したならば、fraktor-streams は失敗を下流に伝播しなければならない
- 5.3 キャンセルが要求されたとき、fraktor-streams は上流へのキャンセル伝播を開始しなければならない

### 6. core/std 境界と no_std 互換
**目的:** ランタイム開発者として no_std コアを維持し、std 依存を拡張層に閉じ込めたい。

#### 受け入れ条件
- 6.1 core 機能がビルドされるとき、fraktor-streams は no_std のみでコンパイルできなければならない
- 6.2 std 機能を含む場合、fraktor-streams は std 依存を std モジュールに閉じ込めなければならない
- 6.3 core の公開 API が提供されている間、fraktor-streams は std 依存なしで利用可能であり続けなければならない
- 6.4 core は fraktor-actor の core 実行基盤を再利用し、Materializer の中核ロジックで重複実装を避けなければならない
- 6.5 core の fraktor-actor 依存は必要最小限に留め、streams/core の独立性を維持しなければならない
- 6.6 fraktor-actor core は streams/core に依存してはならない
- 6.7 streams の公開 API は fraktor-actor の型を露出してはならない

### 7. std 拡張の Actor 実行統合
**目的:** ランタイム利用者として fraktor-actor の ActorSystem 上でストリームを実運用したい。

#### 受け入れ条件
- 7.1 std 拡張が有効なとき、fraktor-streams は fraktor-actor の ActorSystem と統合してストリームを駆動しなければならない
- 7.2 std 拡張が有効なとき、fraktor-streams は Materializer を通じて ActorSystem の実行基盤を利用できなければならない
- 7.3 std 拡張が無効なとき、fraktor-streams は std 依存を要求してはならない
- 7.4 ActorSystem が提供されないとき、fraktor-streams は Materializer の起動を失敗させなければならない
- 7.5 remote/cluster が有効な ActorSystem のとき、fraktor-streams は追加設定なしでストリームを駆動できなければならない

### 8. examples による最小利用例
**目的:** ランタイム利用者として最小構成の使用例を参照し、設計意図を確認したい。

#### 受け入れ条件
- 8.1 std 拡張が有効なとき、fraktor-streams は examples で最小のストリーム構成サンプルを提供しなければならない
- 8.2 サンプルがビルドされるとき、fraktor-streams は core への std 依存を持ち込まずにコンパイルできなければならない
- 8.3 サンプルが実行されたとき、fraktor-streams は Materializer と Source/Flow/Sink の最小合成が動作することを示さなければならない
- 8.4 std 拡張が有効なとき、fraktor-streams は ActorSystem を利用したサンプルを提供しなければならない
- 8.5 modules/streams/examples のサンプルは DSL を利用した構成で提供しなければならない
