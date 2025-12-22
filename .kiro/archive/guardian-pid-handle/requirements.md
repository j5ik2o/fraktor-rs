# 要件ドキュメント

## 導入
ガーディアン参照の循環を断つため、SystemState が保持する root/system/user ガーディアンを PID ハンドル化し、起動前と稼働後をタイプステートで分離する。機能の任意性に由来する他の Option（scheduler/tick_driver/remoting_config/remote_watch_hook）は据え置き、ガーディアン保持部分だけを非 Option 化する。

## 要件

### 要件1: ガーディアンPIDハンドル管理
**目的:** ランタイムはガーディアンの存在を PID で一意にトラッキングし、強参照循環を排除しながら既存のガーディアン生成順序を保ちたい。

#### 受け入れ条件
1. When root/system/user ガーディアン PID が初期設定されるとき、ActorSystemCore は PID を cells レジストリに登録しなければならない。  
2. While いずれかのガーディアン PID が未設定の間、ActorSystemCore はガーディアン依存の spawn/resolve を拒否しなければならない。  
3. If ガーディアン PID に対応する ActorCell が cells に存在しない場合、ActorSystemCore はガーディアン未初期化エラーを返さなければならない。  
4. The ActorSystemCore shall root/system/user の PID を存活フラグとともに保持し、再起動や再構築時も PID を再利用しなければならない。  

### 要件2: ガーディアン解決と停止整合
**目的:** PID ハンドル化後も watch/Termination 通知とシステム終了条件を破綻なく維持したい。

#### 受け入れ条件
1. When ガーディアン PID の ActorCell が停止完了したとき、ActorSystemCore shall cells から当該エントリを除去し、ガーディアン存活フラグをクリアしなければならない。  
2. If root ガーディアンの存活フラグがクリアされた場合、ActorSystemCore shall termination future を完了しなければならない。  
3. When system/user ガーディアンの存活フラグがクリアされ、かつ root ガーディアン PID が未設定の場合、ActorSystemCore shall termination future を完了しなければならない。  
4. When watch 要求が停止済みガーディアン PID に届いたとき、ActorSystemCore shall Terminated を即時返信しなければならない。  

### 要件3: タイプステート遷移（起動前→稼働後）
**目的:** ガーディアン未設定状態での誤用を型で防ぎ、稼働後は非 Option アクセスを保証したい。

#### 受け入れ条件
1. When BootingSystemState が生成されるとき、ActorSystemCore shall ガーディアン PID が未設定であることを型レベルで表現しなければならない。  
2. When 3 つのガーディアン PID が登録されたとき、ActorSystemCore shall BootingSystemState から RunningSystemState へ移行しなければならない。  
3. While RunningSystemState である間、ActorSystemCore shall ガーディアン PID を非 Option で公開しなければならない。  
4. If RunningSystemState でガーディアン PID 解決に失敗した場合、ActorSystemCore shall 異常状態としてエラーを報告しなければならない。  

### 要件4: 既存 API・可観測性への非影響
**目的:** ガーディアン参照方式の変更が外部 API と観測面に影響を与えないことを保証したい。

#### 受け入れ条件
1. The ActorSystemCore shall ActorPath の生成・解決結果を従来と同一に保たなければならない。  
2. When ガーディアン PID ベースの Terminated/Failure/Log イベントが発生するとき、ActorSystemCore shall EventStream へ従来同等のイベント種別とペイロードで配信しなければならない。  
3. If 既存の拡張・RemoteAuthorityManager がガーディアン参照を要求する場合、ActorSystemCore shall PID を介した解決で同等の結果を返さなければならない。  

