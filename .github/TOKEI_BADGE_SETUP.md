# tokeiバッジの自動更新セットアップ手順

## 📋 前提条件
- GitHubアカウント
- このリポジトリへの書き込み権限

## 🔧 セットアップ手順

### 1. GitHub Gistの作成

1. https://gist.github.com/ にアクセス
2. 「Create new gist」をクリック
3. 以下の内容で作成：
   - ファイル名: `tokei_badge.json`
   - 内容:
     ```json
     {
       "schemaVersion": 1,
       "label": "lines of code",
       "message": "0",
       "color": "blue"
     }
     ```
   - **Public** gistとして作成（重要！）
4. 作成後のURLから **Gist ID** をコピー
   - 例: `https://gist.github.com/{username}/{gist_id}` → `{gist_id}` の部分

### 2. Personal Access Token (PAT) の作成

1. GitHub設定を開く: https://github.com/settings/tokens
2. 「Generate new token」→「Generate new token (classic)」を選択
3. 設定：
   - **Note**: `fraktor-rs tokei badge`
   - **Expiration**: `No expiration`（または適切な期限）
   - **Select scopes**: `gist` にチェック
4. 「Generate token」をクリック
5. 表示されたトークンを**安全な場所にコピー**（再表示されません！）

### 3. リポジトリSecretsの設定

1. このリポジトリの設定を開く: https://github.com/j5ik2o/fraktor-rs/settings/secrets/actions
2. 「New repository secret」をクリック
3. 以下の2つのシークレットを追加：

   **シークレット1:**
   - Name: `GIST_TOKEN`
   - Value: 手順2で作成したPersonal Access Token

   **シークレット2:**
   - Name: `TOKEI_GIST_ID`
   - Value: 手順1で取得したGist ID

### 4. README.mdのバッジURLを更新

現在のREADME.md 10行目：
```markdown
[![tokei](https://tokei.rs/b1/github/XAMPPRocky/tokei)](https://github.com/XAMPPRocky/tokei)
```

以下に置き換え：
```markdown
[![Lines of Code](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/{YOUR_USERNAME}/{GIST_ID}/raw/tokei_badge.json)](https://github.com/j5ik2o/fraktor-rs)
```

**注意**: `{YOUR_USERNAME}` と `{GIST_ID}` を実際の値に置き換えてください。

### 5. 動作確認

1. 変更をコミット・プッシュ：
   ```bash
   git add .github/workflows/tokei.yml README.md
   git commit -m "feat: add tokei badge auto-update workflow"
   git push
   ```

2. GitHub Actionsで自動実行される（またはmainブランチにpush）
3. 数分後、Gistが更新されREADMEのバッジに反映される

### 6. 手動実行（オプション）

Actions タブから `tokei` ワークフローを選択し、「Run workflow」で手動実行できます。

## 🔄 更新頻度

- **自動**: 毎日0時（UTC）に実行
- **自動**: mainブランチへのpush時
- **手動**: GitHub Actionsから任意のタイミングで実行可能

## 🐛 トラブルシューティング

### バッジが表示されない
- Gistが **Public** になっているか確認
- GistのURLが正しいか確認
- GitHub Actionsのログでエラーがないか確認

### Gist更新が失敗する
- `GIST_TOKEN` の権限が `gist` を含んでいるか確認
- トークンの有効期限が切れていないか確認
- `TOKEI_GIST_ID` が正しいか確認

### バッジの数値が古い
- GitHub Actionsのログで最終実行時刻を確認
- 手動で「Run workflow」を実行してみる
