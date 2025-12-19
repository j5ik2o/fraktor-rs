# ギャップ分析: guardian-pid-handle

## 1. 現状把握
- **ガーディアン保持**: `system_state.rs` で `root/system/user_guardian: ToolboxMutex<Option<ArcShared<ActorCellGeneric<TB>>>>` を保持し、`set_*_guardian` で Arc を格納、`clear_guardian` で Option を空にしている。`root_guardian_pid()` など PID 取得も Option 経由。  
- **生成フロー**: `ActorSystemGeneric::bootstrap` が root→user→system の順に spawn し、生成直後にそれぞれ `set_*_guardian` で登録。`mark_root_started` で起動完了を示すだけで型レベル保証はない。  
- **停止フロー**: `ActorCell::handle_stop` で `clear_guardian` を呼び、root が消えた場合は `mark_terminated`。`SystemGuardianActor` は hooks 完了で自分を停止し、root が watch している。  
- **解決/観測**: ガーディアン参照は Arc を直接返す (`system_guardian()` など)。Terminated/Failure/Log などイベントの送出は PID ベースだが、ガーディアン存否判定は Option<Arc> に依存。  
- **命名・構造ルール**: 1ファイル1型、no `mod.rs`, core 側で `#[cfg(feature="std")]` 禁止。テストは `hoge/tests.rs`。共有所有権は `ArcShared` 利用。  

## 2. 要件に対するギャップ
- 要件1（PIDハンドル化）: ガーディアンを Arc で保持しており、PID と存活フラグの分離がない → **Missing**。  
- 要件1/2（cells 経由解決）: ガーディアン実体を cells にだけ置く前提がなく、Option<Arc> に依存 → **Missing**。  
- 要件2（停止整合）: termination 判定は Option 空判定に依存し、PID 存活フラグは未実装 → **Missing**。  
- 要件3（タイプステート）: Booting/Running の段階モデルなし、未初期化ガーディアンを型で禁止できない → **Missing**。  
- 要件4（外部API非影響）: ActorPath/イベント配送は現状 Arc 参照前提コードが点在（例: `root_guardian()` 呼び出し）、PID 化後の互換 API が未定義 → **Unknown/要整理**。  

## 3. 実装アプローチ案
### Option A: 既存構造拡張（最小変更）
- `SystemStateGeneric` に `Guardians { root: Pid, system: Pid, user: Pid, flags }` を追加し、既存 `root_guardian` などを段階的に非推奨化。  
- `set_*_guardian` は PID を記録し、実体は `cells.register_cell` のみが所有。`*_guardian_pid()` は非Option 返却（Booting 期間のみ未設定扱い）。  
- `clear_guardian`/termination 判定を PID＋存活フラグで実装し、既存の Option ベース呼び出し点を薄いラッパで互換維持。  
- **利点**: 変更範囲が SystemState/ActorCell/ActorSystem に限定。  
- **懸念**: 移行期に Arc を期待する呼び出しが残るとパニック化するため、API 互換ラッパが必要。  

### Option B: ガーディアン専用レジストリ + タイプステート
- `guardian_registry.rs` を新設し、PID と存活フラグだけを保持。`BootingSystemState` では `Guardians::Uninitialized`、`RunningSystemState` で `Guardians::Initialized` を型で保証。  
- `ActorSystemGeneric::bootstrap` を `BootingSystemState` を返すファクトリに分割し、3 つの PID 設定後に `into_running()` で型遷移。  
- `root_guardian_ref()` 等は Running のみで提供し、Booting では利用不可にする。  
- **利点**: 未初期化誤用を型で封じ、ガーディアン API を整理しやすい。  
- **懸念**: 型追加により呼び出しシグネチャが広く変わり、テスト・サンプル修正が多い。  

### Option C: ハイブリッド（段階導入）
- フェーズ1: Option A で PID/flag 化＋Arc 互換ラッパ（デプリケーション警告コメント）を導入し、挙動を維持。  
- フェーズ2: Option B のタイプステートを導入し、互換ラッパを段階的に削除。  
- **利点**: 段階的にリスクを抑え、テスト増強と並行できる。  
- **懸念**: 期間限定で二重 API を維持する負債が発生。  

## 4. 努力度・リスク
- 努力度: **M (3–7日)** — SystemState/ActorSystem/ActorCell/RootGuardian/テストの横断変更と型遷移の導入が必要。  
- リスク: **中** — termination 判定や watch/Terminated の回りで回帰が起きやすい。PID 解決漏れがあると死活監視が壊れるため、統合テストが必須。  

## 5. Research Needed
- `ActorSystemGeneric::user_guardian_ref/system_guardian_ref` を PID 化後どう互換提供するか（panic 維持か Result にするか）。  
- ガーディアン停止と `root_started`/`root_guardian_pid().is_none()` の判定ロジックを PID/flag 化後にどう整理するか。  
- no_std 側の `ArcShared`/ToolboxMutex 置き換えで追加メモリが許容範囲か（flags の型と配置を決める）。  
- テストの所在: 既存 guardian 関連の統合テストが不足しているため、どのシナリオを追加すべきか（watch/stop/termination 経路）。  

---
分析はギャップ抽出と選択肢提示に留めており、実装方針の最終決定は設計フェーズで行うこと。
