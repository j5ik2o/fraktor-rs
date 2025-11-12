# 要件ドキュメント

## プロジェクト説明（入力）
ActorPathをPekko互換仕様にする。
参考資料: references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorPath.scala

## Introduction
fraktor ランタイムは Pekko の ActorPath 仕様（正規化 URI、UID 付きリモートパス、`ActorSelection` の相対解決規約）と同一の振る舞いを提供し、ロギング・Remoting・DeathWatch を跨いで互換性を確保することを目的とする。

## Requirements

### Requirement 1: ActorPath 文字列表現の正規化
**目的:** ランタイムメンテナが Pekko と同じ ActorPath 文字列でログやリモート API を共有できるようにする。

#### Acceptance Criteria
1. When fraktor Runtime が ActorPath をログ・DeadLetter・テレメトリ用途で文字列化するとき、ActorPath Formatter は Pekko の正規化規則と同一の URI を出力し、authority が設定されている場合は `fraktor://<system>@<host>:<port>/<path>`、設定されていない場合は `fraktor://<system>/<path>` を生成しなければならない。
2. If 任意のパスセグメントに RFC2396 準拠の文字（英数字 `[A-Za-z0-9]`、記号 `-_.*$+:@&=,!~';`、または `%HH` 形式のパーセントエンコード）以外が含まれるなら、ActorPath Validator はそのセグメントのインデックスを含むエラーを返してシリアライズを拒否しなければならない。
3. While ランタイムが authority（host/port）を構成していない間、ActorPath Formatter は `fraktor://<system>/<path>` 形式を維持したまま `@<host>:<port>` を省き、Pekko 既定の `/system` または `/user` 直下のルートパスを用いなければならない。
4. Where リモートパスに `#123456` のような UID サフィックスが含まれている場合、ActorPath Formatter はシリアライズ／デシリアライズの間でその UID を一語一句保持しなければならない。
5. fraktor Runtime は ActorPath Formatter を通じて各セグメントの大文字小文字を元のアクター生成要求どおり保持しなければならない。
6. If ユーザー定義の ActorPath セグメントが `$` から始まるなら、ActorPath Validator はそれをシステム予約セグメントとして拒否し、訂正を要求しなければならない。

### Requirement 2: ActorPath 解析と検証
**目的:** インフラエンジニアが ActorSelection・監視・リモート配送を Pekko と同じ解釈規則で運用できるようにする。

#### Acceptance Criteria
1. When ランタイムが任意の API から ActorPath 文字列を受け取るとき、ActorPath Resolver はそれを root/system/user/child セグメントへ分解し、Pekko のツリー構造と一致する形でパターンマッチに供さなければならない。
2. When ActorSelection が `..` を含む相対パスを使用するとき、ActorPath Resolver は親階層を決定的に解決し、ルートを逸脱する場合は検証エラーを返さなければならない。
3. If 渡された ActorPath のスキームが `fraktor` または `fraktor.tcp` でなければ、ActorPath Resolver はパスを拒否して未対応スキームエラーを通知しなければならない。
4. While ランタイムが参照されたリモート authority のクラスタアドレスマッピングを持たない間、ActorPath Resolver はそのパスを未解決状態に保ち、メッセージ配送を行わず延期対象としてマークしなければならない。
5. When パーセントエンコード済み文字を含む ActorPath をデシリアライズするとき、ActorPath Resolver は Pekko の URL デコード規則に従って復号してからパスマッチを実施しなければならない。

### Requirement 3: ActorPath 解決と等価性
**目的:** プラットフォームオペレータが解決・等価性・隔離の挙動を Pekko に合わせてキャッシュや DeathWatch を安全に扱えるようにする。

#### Acceptance Criteria
1. When 2 つの ActorPath が同一の system 名・authority・正規化済みセグメントを共有するとき、ActorPath Comparator はそれらを等しいと見なし、同一ハッシュ値を生成しなければならない。
2. If 2 つの ActorPath が UID の有無や値だけ異なる場合でも、ActorPath Comparator はルートアドレスとパスセグメントが一致している限り等価として扱わなければならない。
3. While ActorPath に紐づく ActorRef が死亡済みで DeathWatch から未確認の間、ActorPath Registry はそのパスを予約し、ActorSystemConfig API で指定された隔離期間（未指定ならデフォルト 5 日）が満了するまで同じ UID の再利用を禁止しなければならない。
4. When `ChildActorRef` を通じて子パスを解決するとき、ActorPath Resolver は親セグメントを再検証せず親子セグメントを連結し、深さに比例した決定的な解決を提供しなければならない。
5. If リモート authority が quarantine 中であるなら、ActorPath Router はその authority 宛て配送を拒否し、各ブロックされた送信に対して Pekko 互換の `InvalidAssociation` シグナルを発行しなければならない。
6. When ランタイムがアクターの再生成可否を判定するとき、ActorPath Registry は ActorPath 等価判定だけに依存せず、ActorRef（パス + UID）レベルの一意性チェックで再利用可否を決定しなければならない。
7. When ActorSystemConfig API が初期化パラメータを受け取るとき、利用者は隔離期間（Duration）をファイルではなく API 経由で上書き設定できなければならず、指定がない場合はデフォルト 5 日を適用しなければならない。

### Requirement 4: リモート authority の状態遷移管理
**目的:** リモート authority の解決/接続/隔離状態を明示し、配送ポリシーを一貫させる。

#### Acceptance Criteria
1. When ランタイムが authority のクラスタアドレスマッピングを保持していない状態で ActorPath を受け取るとき、Remote Authority Manager は当該 authority を「未解決」状態に設定し、関連メッセージを延期キューに入れなければならない。
2. When クラスタアドレスマッピングとハンドシェイクが成功するとき、Remote Authority Manager は状態を「接続済み」へ遷移させ、延期キュー内のメッセージを到達順に配送しなければならない。
3. If リモートから `InvalidAssociation` や quarantine 要求が報告される、もしくは再接続試行が RemotingConfig API で設定された隔離期間を超えて失敗するなら、Remote Authority Manager は状態を「quarantine」へ遷移させ、新規配送を拒否して各送信者へ `InvalidAssociation` を通知しなければならない。
4. While authority が「quarantine」状態にある間、Remote Authority Manager は RemotingConfig API 経由で設定された隔離期間または管理者による解除イベントが発生するまで状態を維持し、期間満了時にのみ「未解決」へ戻して再解決サイクルを再開しなければならない。
