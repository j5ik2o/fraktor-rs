# 要件ドキュメント

## 導入
本仕様は、`fraktor-streams-rs` が Pekko Streams の互換MUST範囲において、観測可能な振る舞い（`emits / backpressures / completes / fails`）を同等に提供するための要件を定義する。
本仕様では、リリース前フェーズであることを前提に後方互換性の維持を要件に含めない。

## 要件

### 要件1: 公開API互換性の確立
**目的:** ライブラリ利用者として Pekko Streams のMUST演算子を移植時に同等概念で利用できるようにし、仕様準拠を優先して移行したい。

#### 受け入れ条件
1. Pekko Streams 互換MUST演算子の利用要求が起きたとき、fraktor-streams-rs は Source/Flow/Sink から同等概念の公開操作を選択できるようにしなければならない。
2. 互換対象外の演算子または不正な引数が指定されたならば、fraktor-streams-rs は失敗理由を呼び出し側へ明示しなければならない。
3. 互換機能が有効な間、fraktor-streams-rs は公開操作の入力条件・終端条件・失敗条件を一貫して提示し続けなければならない。
4. fraktor-streams-rs は常に互換対象APIの対応範囲を識別可能な形で示さなければならない。

### 要件2: Substream演算子の意味論互換
**目的:** ストリーム設計者として `group_by` / `split_when` / `split_after` / `merge_substreams` / `concat_substreams` の意味論を揃え、分岐統合処理を移植したい。

#### 受け入れ条件
1. `group_by` 実行中に新しいキーが観測されたとき、fraktor-streams-rs は当該キー専用のサブストリームへ要素を振り分けなければならない。
2. `group_by` のキー数が設定上限を超えたならば、fraktor-streams-rs はストリームを失敗として扱わなければならない。
3. `split_when` の述語が真になったとき、fraktor-streams-rs は当該要素を新しいサブストリームの先頭にしなければならない。
4. `split_after` の述語が真になったとき、fraktor-streams-rs は当該要素を現在のサブストリームの末尾にし、次要素から新しいサブストリームへ流さなければならない。
5. サブストリーム統合を含む場合、fraktor-streams-rs は完了した全サブストリームの要素を欠落なく下流へ引き渡さなければならない。
6. サブストリームのいずれかが下流圧力を発生させている間、fraktor-streams-rs は上流へ下流圧力を伝播し続けなければならない。

### 要件3: flatMap系演算子の意味論互換
**目的:** ストリーム設計者として `flat_map_concat` と `flat_map_merge` の差異を維持し、順序性と並行性を正しく使い分けたい。

#### 受け入れ条件
1. `flat_map_concat` で入力要素が到着したとき、fraktor-streams-rs は対応する内側ストリームの完了後にのみ次の内側ストリームを開始しなければならない。
2. `flat_map_merge` の同時実行数が `breadth` 上限に達したならば、fraktor-streams-rs は新しい内側ストリーム生成要求を上流圧力として抑止しなければならない。
3. `flat_map_merge` の内側ストリームが要素を発行したとき、fraktor-streams-rs は同一内側ストリーム内の順序を保持して下流へ渡さなければならない。
4. 回復ポリシーを含まない場合、内側ストリームの失敗時に fraktor-streams-rs はストリーム全体を失敗として扱わなければならない。

### 要件4: 動的Hub演算子の意味論互換
**目的:** サービス運用者として `MergeHub` / `BroadcastHub` / `PartitionHub` を動的接続で使い、接続順や購読数に依存する不整合を防ぎたい。

#### 受け入れ条件
1. `MergeHub` の利用開始イベントが起きたとき、fraktor-streams-rs は受信側が有効化された後にのみ送信側接続を受け付けなければならない。
2. `BroadcastHub` で購読者が存在しないならば、fraktor-streams-rs は上流をドロップせず下流圧力として制御しなければならない。
3. `BroadcastHub` に新しい購読者が接続されたとき、fraktor-streams-rs は接続後に到着する要素を受信可能にしなければならない。
4. `PartitionHub` でルーティング判定が行われたとき、fraktor-streams-rs は各要素をただ1つの有効な購読者へ割り当てなければならない。
5. `PartitionHub` で有効な購読者が存在しないならば、fraktor-streams-rs は上流へ下流圧力または明示的失敗のいずれかを一貫した規約で返さなければならない。

### 要件5: KillSwitch制御の意味論互換
**目的:** 運用者として外部制御でストリームを停止・異常終了させ、停止契約を予測可能にしたい。

#### 受け入れ条件
1. `shutdown` 呼び出しイベントが起きたとき、fraktor-streams-rs は上流をキャンセルし、下流を完了させなければならない。
2. `abort` 呼び出しイベントが起きたとき、fraktor-streams-rs は上流をキャンセルし、下流を指定エラーで失敗させなければならない。
3. 最初の `shutdown` または `abort` が確定したならば、fraktor-streams-rs は後続の制御呼び出しを無視しなければならない。
4. `SharedKillSwitch` を含む場合、fraktor-streams-rs は materialization 前に共有制御点を生成でき、紐付いた複数ストリームを同時制御できなければならない。
5. KillSwitch が未発火の間、fraktor-streams-rs は通常の要素処理を継続し続けなければならない。

### 要件6: Restart/Backoff/Supervisionの意味論互換
**目的:** 運用者として障害時の再試行挙動を統一し、復旧戦略を安全に選択したい。

#### 受け入れ条件
1. restart対象ステージが失敗または完了したとき、fraktor-streams-rs は再起動予算が残っていればバックオフ経過後に再開しなければならない。
2. バックオフ待機の間、fraktor-streams-rs は新規要素を下流へ発行せず、上流へ下流圧力を返し続けなければならない。
3. 再起動予算を超過したならば、fraktor-streams-rs は当該ストリームを完了または失敗として終端しなければならない。
4. `supervision_resume` が設定された場合、ステージ失敗時に fraktor-streams-rs は失敗要素をスキップして処理を継続しなければならない。
5. `supervision_restart` が設定された場合、ステージ失敗時に fraktor-streams-rs はステージ状態を初期化して処理を継続しなければならない。
6. `split_when` または `split_after` に `supervision_restart` を含む場合、fraktor-streams-rs は `supervision_resume` と同等の振る舞いをしなければならない。

### 要件7: 非同期境界と下流圧力契約
**目的:** ストリーム設計者として融合実行と非同期境界の違いを利用し、並行処理時の順序と圧力制御を担保したい。

#### 受け入れ条件
1. 非同期境界が挿入されていないとき、fraktor-streams-rs は演算子群を融合実行として扱わなければならない。
2. 非同期境界が挿入されたとき、fraktor-streams-rs は境界の上下流を独立した実行区間として扱わなければならない。
3. 非同期境界を跨いで要素が伝搬する間、fraktor-streams-rs は各実行区間内の要素順序を保持し続けなければならない。
4. 非同期境界の受け渡しバッファが飽和したならば、fraktor-streams-rs は上流へ下流圧力を返さなければならない。

### 要件8: 互換性検証と no_std 維持
**目的:** 保守担当者として互換性の退行を継続的に検出し、`core` の no_std 制約を維持したい。

#### 受け入れ条件
1. 互換性検証実行イベントが起きたとき、fraktor-streams-rs は互換MUST演算子ごとの期待結果（発行・圧力・完了・失敗）を判定可能にしなければならない。
2. `core` プロファイルの検証が行われたとき、fraktor-streams-rs は `std` 依存なしでビルド可能でなければならない。
3. 互換性検証で不一致が検出されたならば、fraktor-streams-rs は不一致箇所を識別可能な結果として提示しなければならない。
4. fraktor-streams-rs は常に本仕様の各受け入れ条件を再現可能な形で検証できなければならない。

### 要件9: 破壊的変更許容ポリシー
**目的:** 保守担当者として互換MUST達成を優先し、不要な互換コードを排除して保守コストを抑えたい。

#### 受け入れ条件
1. 互換MUST達成のために公開APIまたは内部構造の変更が必要になったとき、fraktor-streams-rs は破壊的変更を許容しなければならない。
2. 破壊的変更が適用されたとき、fraktor-streams-rs は関連テストおよびCI検証を通過しなければならない。
3. 互換MUST達成に不要な旧挙動維持コードが存在するならば、fraktor-streams-rs は当該コードを残してはならない。
4. fraktor-streams-rs は常に後方互換性よりも仕様準拠とコード簡潔性を優先しなければならない。
