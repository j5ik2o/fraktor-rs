# 要件ドキュメント

## 導入
SystemStateGeneric が一部フィールドを `Option` として持ち、生成後に別 API で差し込む前提になっている状態を解消する。これにより「未初期化状態が存在する/初期化順序が逆転する/呼び出し側が Option 分岐を強いられる」問題を排除し、SystemStateGeneric を常に完全初期化済みの状態だけで表現できるようにする。

また、共有が必要な呼び出し側への影響は SystemStateSharedGeneric 側で吸収し、テスト用途の `new_empty()` は `cfg(test)` / `feature = "test-support"` に限定して維持する。

## 要件

### 要件1: SystemStateGeneric の未初期化状態排除
**目的:** ランタイムとして、SystemStateGeneric が未初期化状態を表現しないことで、初期化順序の逆転と Option 分岐を排除したい。

#### 受け入れ条件
1. When SystemStateGeneric が生成されるとき、SystemStateGeneric は scheduler_context と tick_driver_runtime を初期化済みとして保持しなければならない。
2. The SystemStateGeneric shall scheduler_context と tick_driver_runtime を取得する公開 API において `Option` を返してはならない。
3. When remoting が無効な構成で actor system が構築されるとき、SystemStateGeneric は remoting 設定の取得 API で `None` を返さなければならない。
4. When remoting が有効な構成で actor system が構築されるとき、SystemStateGeneric は remoting 設定の取得 API で `Some` を返し、canonical host/port と quarantine duration を構成と一致させなければならない。
5. While remote watch hook が未登録の間、SystemStateGeneric は watch/unwatch の転送を行わず、既存のフォールバック処理を継続しなければならない。
6. When remote watch hook が登録されているとき、SystemStateGeneric は watch/unwatch を hook に転送し、hook が消費した場合はフォールバック処理を実行してはならない。

### 要件2: 初期化順序の保証（構築フロー）
**目的:** 開発者として、SystemStateGeneric を生成してから依存要素を差し込む手順を不要にし、誤った初期化順序を不可能にしたい。

#### 受け入れ条件
1. When ActorSystem が構築されるとき、ActorSystemCore は SystemStateGeneric を完全初期化済みの状態で生成しなければならない。
2. The ActorSystemCore shall SystemStateGeneric 生成後に scheduler_context/tick_driver_runtime/remoting 設定を差し込むための公開 API を必須手順として要求してはならない。
3. When ActorSystem が構成（system 名、remoting、scheduler、tick driver）付きで構築されるとき、ActorSystemCore は起動前に当該構成を SystemStateGeneric へ反映しなければならない。
4. If ActorSystem の構築に必要な tick driver 構成が不足している場合、ActorSystemCore は起動を拒否し、診断可能なエラーを返さなければならない。

### 要件3: SystemStateSharedGeneric による共有アクセス維持
**目的:** 利用者として、Option 排除による API 変更の影響を最小化しつつ、安全に共有アクセスできる状態管理を維持したい。

#### 受け入れ条件
1. The SystemStateSharedGeneric shall 共有所有権の提供を維持し、呼び出し側へロックガードを返さずにアクセスを完結させなければならない。
2. When 呼び出し側が scheduler_context または tick_driver_runtime を参照するとき、SystemStateSharedGeneric は `Option` 分岐を要求せずに有効なハンドルを提供しなければならない。
3. When 既存の tick_driver_snapshot / shutdown_scheduler / remoting 設定参照 API が呼ばれるとき、SystemStateSharedGeneric は従来と同等の意味論で結果を返さなければならない。
4. Where 呼び出し側が remote watch hook を登録しない場合でも、SystemStateSharedGeneric は既存の watch/unwatch の振る舞いを維持しなければならない。

### 要件4: テスト支援としての new_empty() 維持
**目的:** テスト作者として、最小構成の actor system を生成する `new_empty()` を維持しつつ、本番ビルドに漏れない形で安全に提供したい。

#### 受け入れ条件
1. Where `cfg(test)` または `feature = "test-support"` を含む場合、ActorSystemCore は `new_empty()` を提供しなければならない。
2. Where `cfg(test)` でも `feature = "test-support"` でもない場合、ActorSystemCore は `new_empty()` を公開 API として提供してはならない。
3. When `new_empty()` が呼ばれたとき、ActorSystemCore は SystemStateGeneric を完全初期化済みの状態で返し、追加の初期化手順を要求してはならない。
4. When `new_empty()` で生成されたシステムが drop されるとき、ActorSystemCore は tick driver runtime の停止処理を安全に実行しなければならない。

### 要件5: 回帰防止と品質ゲート
**目的:** メンテナとして、Option 排除による影響が既存機能を破壊しないことを自動検証で保証したい。

#### 受け入れ条件
1. When `./scripts/ci-check.sh all` が実行されるとき、プロジェクトはエラー無しで完了しなければならない。
2. The ActorSystemCore shall 本変更において core ランタイム本体へ `#[cfg(feature = "std")]` による分岐を追加してはならない。
3. The ActorSystemCore shall lint を `allow` 等で回避せず、既存の lint/テストを無効化してはならない。
