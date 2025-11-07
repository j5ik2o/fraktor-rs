## 背景
- ActorSystemGeneric には `/user` 相当のユーザガーディアンしかなく、`/` 直下のルートガーディアンや `/system` 相当のシステムガーディアンが存在しない。
- そのため、システム内部アクター（クラスタ管理・CoordinatedShutdown 等）のライフサイクルをユーザアクターと独立させられず、停止順序や監督戦略の分離ができない。
- Pekko ではルート→（`/user`, `/system`）という階層を前提に終了シナリオやシステムメッセージを構成しており、Pekko 互換の下地を用意することで将来のクラスタ／CoordinatedShutdown 実装を容易にする必要がある。

## 変更概要
- ActorPath/ActorRef を `/`, `/user`, `/system` を予約パスとして扱うよう拡張し、ルートガーディアンを ActorSystem 起動時に必ず生成する。
- ルートガーディアン直下にユーザガーディアンとシステムガーディアンを生成し、公開APIでは `/user` 配下のみユーザに開放する。`/system` 配下はフレームワーク内部用のコンストラクタに限定する。
- 監督戦略を再定義し、ユーザ/システムガーディアンで復旧できない障害はルートが即座にフェイルファストで停止シーケンスへ移行する。停止シーケンスは `/user` → `/system` → ルートの順で伝搬させ、CoordinatedShutdown の基盤にする。
- Pekko の実装同様にルート・システム・ユーザを DeathWatch で連結し、SystemGuardian が `RegisterTerminationHook` 参加者へ通知 (`TerminationHook` → `TerminationHookDone`) を仲介したのちに自身とイベントストリームを停止する流れを確立する。

## 影響範囲
- ActorSystem 初期化コード、ActorRef/Path の表現、Guardian 周りのAPIが更新される。
- 既存のユーザ向けAPIは `/user` を暗黙利用するため後方互換性の懸念は小さいが、内部的には破壊的変更となる。
- ルート/システムガーディアンを前提とした新しい監督挙動・終了順序、TerminationHook API が導入されるため、関連テスト・ドキュメントを追加する必要がある。
