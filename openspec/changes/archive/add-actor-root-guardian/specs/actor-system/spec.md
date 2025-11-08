## ADDED Requirements

### Requirement: ルートガーディアン階層
ActorSystem MUST `/` を持つルートガーディアンを内部的に生成し、その直下に `/user` と `/system` を予約パスとして配置し、`/temp` `/deadLetters` などの特別ノードを一元管理しなければならない。

#### Scenario: 起動時にルートを生成する
- **WHEN** ActorSystem を初期化する
- **THEN** `/` ルートガーディアンを専用メールボックスと永続的な監督戦略付きで生成する
- **AND** ルート直下に `/user` ガーディアンと `/system` ガーディアンを即座に生成する
- **AND** ルートガーディアンは `/temp` `/deadLetters` などの予約名と登録済みの追加トップレベルパスを保持し、ActorPath/ActorRef 解決時に利用できるようにする

#### Scenario: 一時アクターを管理する
- **WHEN** ランタイムが一時アクターを生成・破棄する（例: ask パターンやプローブ）
- **THEN** ルートガーディアンは `/temp` 配下に VirtualPathContainer 相当の管理ノードを保持し、`register_temp_actor`/`unregister_temp_actor` API で参照を追加・削除できるようにする
- **AND** `/temp` 下のパスは他のトップレベル命名規則から隔離され、衝突なく base64 などで自動採番される

#### Scenario: 拡張がトップレベルパスを登録する
- **WHEN** 拡張やプロバイダが `root_guardian.start()` を呼び出す前に `/metrics` などの追加トップレベルパスを登録する
- **THEN** ルートガーディアンは登録内容を reserved child として保持する
- **AND** 登録済みパスは ActorSelection/ActorRef 解決で `rootGuardian` から辿れる
- **AND** `root_guardian.start()` 実行後（ActorSystem 初期化完了後）に登録しようとした場合はエラーを返し、ログに警告を出す
- **AND** ActorSystem MUST 遅延して `root_guardian.start()` を呼び出し、追加トップレベル登録が完了してから初めて root を起動する

#### Scenario: トップレベル登録でエラーが発生する
- **WHEN** `register_extra_top_level` で予約名（`user`, `system`, `temp`, `deadLetters` など）や既存名と衝突する、あるいは ActorSystem 起動後に登録しようとする
- **THEN** API は `RegisterExtraTopLevelError` を返し、種類は `ReservedName`, `DuplicateName`, `AlreadyStarted` のいずれかになる
- **AND** `AlreadyStarted` の場合は警告ログを出力し、登録は行われない

### Requirement: システムガーディアンAPI境界
システムガーディアン配下のアクター生成はフレームワーク内部APIに限定し、公開APIは `/user` 配下のみを作成可能にするようフレームワーク MUST 強制しなければならない。

#### Scenario: ユーザAPIでアクターを生成する
- **WHEN** 公開API（例: `actor_of`）で新規アクターを生成する
- **THEN** 新しいアクターは常に `/user` の子として作成される
- **AND** `/system` 配下への生成や命名は拒否される

#### Scenario: フレームワーク内部でシステムアクターを生成する
- **WHEN** 内部専用API（例: `system_actor_of`）が呼び出される
- **THEN** 新しいアクターは `/system` の子として生成される
- **AND** API呼び出しはフレームワーク内部コードに限定され、ユーザコードからは利用できない

### Requirement: 監督戦略と終了シーケンス
ルート/システム/ユーザ各ガーディアンは明示的な監督戦略と停止順序を持ち、ActorSystem 終了時に `/user` → `/system` → ルートの順で停止イベントが伝搬するよう ActorSystem MUST 実装しなければならない。

#### Scenario: ガーディアンで復旧できない障害が発生する
- **WHEN** `/user` または `/system` で監督戦略が `Escalate` を返す障害が発生する
- **THEN** ルートガーディアンは障害をログに記録し、対象ガーディアンを再生成せず ActorSystem の停止フロー（CoordinatedShutdown 相当）を即時に開始する
- **AND** ルートガーディアンの監督戦略は常に `SupervisorStrategy::Stop` を返し、フェイルファストを保証する

#### Scenario: ActorSystem を終了する
- **WHEN** ActorSystem::terminate または CoordinatedShutdown を開始する
- **THEN** ルートガーディアン（または同等のシャットダウンエンジン）は `/user` ガーディアンに停止要求を送り、`StopChild` 相当のシステムメッセージでユーザアクターを順次停止させる
- **AND** `/system` ガーディアンは `/user` の `Terminated` 通知を受け取ったタイミングで TerminationHook 処理を開始し、すべてのフックが完了したら自身を停止する
- **AND** ルートガーディアンは `/system` の `Terminated` 通知を受けて ActorSystem を終了済みとしてマークし、残リソースを解放する

#### Scenario: ガーディアン間の監視リンクを張る
- **WHEN** ActorSystem を起動する
- **THEN** `/system` ガーディアンは DeathWatch を用いて `/user` ガーディアンを監視し、`Terminated` を受けたら TerminationHook 流れに遷移する
- **AND** ルートガーディアンは `/system` ガーディアンを監視し、`Terminated` を受けたら ActorSystem 停止／ログ出力／終了フラグをセットする
- **AND** 監視リンクは ActorSystem が完全停止するまで維持され、再生成は行わない

#### Scenario: 監督戦略のカスタマイズ可否を管理する
- **WHEN** ユーザが ActorSystem 構築時に独自の `SupervisorStrategyConfigurator` を設定する
- **THEN** その戦略は `/user` ガーディアンにのみ適用され、`/system` とルートはそれぞれ `SupervisorStrategy::default()` と `SupervisorStrategy::Stop` に固定される
- **AND** フレームワークは `/system` またはルートの監督戦略をユーザコードから差し替えようとした場合にエラーまたは警告を返し、適用を拒否する

#### Scenario: 無効な監督戦略設定を検出する
- **WHEN** ActorSystem 構築時に無効な監督戦略（例: タイムアウトが負値、サポート外のクラス名）が `/user` 用に指定される
- **THEN** ActorSystem MUST 構築を失敗させ、`InvalidSupervisorStrategy` などのエラーを返す
- **AND** `/system` とルートについては常に固定戦略にフォールバックし、ユーザ設定を受け付けない

#### Scenario: ガーディアン再起動時に子アクターを維持する
- **WHEN** `/user` または `/system` ガーディアンが監督戦略により再起動される
- **THEN** ガーディアンは `pre_restart` を空実装として子アクターを停止せず再起動する
- **AND** DeathWatch や TerminationHook の登録状態はそのまま維持される

#### Scenario: StopChild システムメッセージを処理する
- **WHEN** ランタイムが `/user` または `/system` ガーディアンに `StopChild(child_pid)` を送信する
- **THEN** ガーディアンは指定された子のみを停止し、他の子や自分自身には影響を与えない
- **AND** `StopChild` は `ActorSystem::stop` や CoordinatedShutdown から利用され、公開APIからは直接露出しない

### Requirement: TerminationHook 調停
SystemGuardian MUST 終了フック用のプロトコルを公開し、`/user` の Terminated 通知を契機にフック処理をトリガし、ガーディアン停止前に内部サブシステムがクリーンアップを完了できるようにしなければならない。

#### Scenario: TerminationHook を利用してクリーンアップする
- **WHEN** クラスタやリモートトランスポートなどのシステムアクターがシャットダウン前に後処理をしたい
- **THEN** それらは SystemGuardian に `RegisterTerminationHook` を送り、SystemGuardian は `sender()` を watch してフック集合に追加する
- **AND** `/user` ガーディアンの `Terminated` 通知を受けると SystemGuardian は各登録先へ `TerminationHook` を送り、`TerminationHookDone` を待つ
- **AND** 全てのフックが `TerminationHookDone` を返した時点で SystemGuardian はイベントログを停止し、自身を停止する
- **AND** タイムアウトや応答欠如が発生した場合はログに警告を出しつつ停止フローを継続できるよう設計されていなければならない

#### Scenario: TerminationHook のタイムアウトを処理する
- **WHEN** SystemGuardian が `TerminationHook` を送ったにもかかわらず、一定猶予時間内に `TerminationHookDone` も `Terminated` も受け取れないフックが存在する
- **THEN** SystemGuardian は当該フックを警告ログ付きで強制的に解除し、残りのフック待機を継続する
- **AND** 全フックが完了または強制解除された時点でイベントログ停止→自己停止に進む
