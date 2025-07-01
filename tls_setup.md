# TLS設定ガイド

このドキュメントでは、admin_backendでTLS（HTTPS/gRPCS）を有効にする方法を説明します。

## 前提条件

- Let's Encryptまたは有効なSSL証明書の取得完了
- admin_backendが正常に動作していること

## TLS有効化手順

### 1. 証明書ファイルの確認

Let's Encryptの場合、以下のファイルが必要です：

```bash
# 証明書ファイルの存在確認
sudo ls -la /etc/letsencrypt/live/YOUR_DOMAIN/
# 以下のファイルがあることを確認：
# - fullchain.pem (証明書チェーン)
# - privkey.pem (秘密鍵)
```

### 2. 証明書へのアクセス権限設定

admin_backendサービスが証明書ファイルにアクセスできるよう権限を設定します：

```bash
# 証明書グループの作成
sudo groupadd letsencrypt

# ユーザーをグループに追加
sudo usermod -a -G letsencrypt kotobukid

# 証明書ディレクトリの権限設定
sudo chgrp -R letsencrypt /etc/letsencrypt/live/
sudo chgrp -R letsencrypt /etc/letsencrypt/archive/
sudo chmod -R g+rx /etc/letsencrypt/live/
sudo chmod -R g+rx /etc/letsencrypt/archive/
```

### 3. systemdサービスファイルの更新

TLS環境変数を含むサービスファイルに更新：

```bash
# 現在のサービスを停止
sudo systemctl stop admin-backend

# TLS対応サービスファイルをコピー
sudo cp admin-backend-tls.service.example /etc/systemd/system/admin-backend.service

# ドメイン名を実際のドメインに変更
sudo nano /etc/systemd/system/admin-backend.service

# systemd設定をリロード
sudo systemctl daemon-reload
```

### 4. サービスの再起動

```bash
# サービスを開始
sudo systemctl start admin-backend

# ステータス確認
sudo systemctl status admin-backend

# ログ確認
sudo journalctl -u admin-backend -f
```

## 動作確認

### TLS有効化確認

ログに以下のメッセージが表示されることを確認：

```
TLS enabled - gRPC server listening on 0.0.0.0:50051 with TLS
```

### gRPCクライアントでのテスト

TLS有効化後は、grpcurlで`-plaintext`オプションを削除してテストします：

```bash
# TLS接続でのテスト
grpcurl -proto proto/admin.proto -H "api-key: YOUR_API_KEY" YOUR_DOMAIN:50051 admin.AdminSync/GetSyncStatus

# 具体例
grpcurl -proto proto/admin.proto -H "api-key: ADM_xxx" ik1-341-30725.vs.sakura.ne.jp:50051 admin.AdminSync/GetSyncStatus
```

## トラブルシューティング

### 権限エラー

症状：`Permission denied` エラー

解決策：
1. 証明書ファイルの権限を確認
2. letsencryptグループの設定を確認
3. ユーザーがグループに追加されているか確認

### 証明書エラー

症状：`Failed to read certificate file` エラー

解決策：
1. 証明書ファイルのパスが正しいか確認
2. ファイルが存在するか確認
3. ファイルの読み取り権限を確認

### フォールバック動作

TLS設定でエラーが発生した場合、自動的にプレーンテキストモードで起動します。
ログに以下のメッセージが表示されます：

```
Failed to setup TLS: [エラー内容]. Falling back to plaintext
gRPC server listening on 0.0.0.0:50051 (plaintext)
```

## 証明書の自動更新

Let's Encryptの証明書は90日で期限切れになるため、自動更新を設定します：

```bash
# 自動更新のテスト
sudo certbot renew --dry-run

# 更新後のサービス再起動設定
sudo mkdir -p /etc/letsencrypt/renewal-hooks/deploy/
sudo tee /etc/letsencrypt/renewal-hooks/deploy/restart-admin-backend.sh > /dev/null << 'EOF'
#!/bin/bash
systemctl reload admin-backend || true
EOF

sudo chmod +x /etc/letsencrypt/renewal-hooks/deploy/restart-admin-backend.sh
```

## セキュリティ考慮事項

- TLS証明書の秘密鍵は適切に保護してください
- 定期的に証明書の有効期限を確認してください
- 不要なポートは閉じてください
- ファイアウォール設定を確認してください