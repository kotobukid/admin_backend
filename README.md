# Admin Backend - 運用メモ

wx_dbの複数拠点開発用同期サーバー。一人開発（拠点1,2）での利用を想定。

## 初期セットアップ

### 1. VPSの準備
```bash
# Ubuntu 22.04 LTS推奨
# 必要なパッケージ
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev sqlite3
```

### 2. Rustのインストール
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 3. ドメイン設定
- VPSのIPアドレスにドメインを向ける（例: admin.example.com）
- DNSのAレコードを設定

### 4. certbot設定（TLS証明書）
```bash
# certbotインストール
sudo apt install certbot

# 証明書取得（初回のみ）
sudo certbot certonly --standalone -d admin.example.com

# 証明書の場所を確認
sudo ls -la /etc/letsencrypt/live/admin.example.com/
# fullchain.pem と privkey.pem があることを確認
```

### 5. 自動更新設定
```bash
# 更新テスト
sudo certbot renew --dry-run

# 自動更新用のhookスクリプト作成
sudo mkdir -p /etc/letsencrypt/renewal-hooks/deploy/
sudo nano /etc/letsencrypt/renewal-hooks/deploy/restart-admin-backend.sh
```

スクリプト内容:
```bash
#!/bin/bash
systemctl reload admin-backend || true
```

```bash
sudo chmod +x /etc/letsencrypt/renewal-hooks/deploy/restart-admin-backend.sh
```

## プロジェクトのビルド

```bash
# リポジトリをクローン
git clone <repository> /opt/admin_backend
cd /opt/admin_backend

# ビルド
cargo build --release

# データディレクトリ作成
mkdir -p data
```

## systemdサービス設定

```bash
sudo nano /etc/systemd/system/admin-backend.service
```

内容:
```ini
[Unit]
Description=Admin Backend gRPC Service
After=network.target

[Service]
Type=simple
User=admin
WorkingDirectory=/opt/admin_backend
Environment="RUST_LOG=info"
ExecStart=/opt/admin_backend/target/release/admin_backend
Restart=on-failure
RestartSec=5

# セキュリティ設定
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

```bash
# サービス有効化と起動
sudo systemctl daemon-reload
sudo systemctl enable admin-backend
sudo systemctl start admin-backend

# ログ確認
sudo journalctl -u admin-backend -f
```

## データベース初期化

```bash
cd /opt/admin_backend

# マイグレーション実行
sqlx migrate run --database-url sqlite://data/admin.db

# 権限設定
sudo chown -R admin:admin data/
chmod 700 data/
chmod 600 data/admin.db
```

## APIキー生成

```bash
# 初回のAPIキー生成（実装予定のCLIツール）
./target/release/admin_backend generate-key --name "dev-machine-1" --permission read_write

# 生成されたキーを安全に保管
# 例: ADM_1234567890abcdef...
```

## クライアント設定（wx_db側）

`.env`ファイルに追加:
```env
ADMIN_BACKEND_URL=admin.example.com:50051
ADMIN_BACKEND_API_KEY=ADM_1234567890abcdef...
```

## バックアップ

```bash
# 手動バックアップ
sudo -u admin cp /opt/admin_backend/data/admin.db /opt/admin_backend/data/backup/admin.db.$(date +%Y%m%d-%H%M%S)

# cronで自動バックアップ
sudo crontab -e -u admin
```

cron設定:
```
0 3 * * * cp /opt/admin_backend/data/admin.db /opt/admin_backend/data/backup/admin.db.$(date +\%Y\%m\%d)
```

## トラブルシューティング

### 接続できない
```bash
# ポート確認
sudo ss -tlnp | grep 50051

# ファイアウォール確認
sudo ufw status
sudo ufw allow 50051/tcp  # 必要に応じて

# 証明書の有効期限確認
sudo certbot certificates
```

### 証明書エラー
```bash
# 証明書の再取得
sudo systemctl stop admin-backend
sudo certbot certonly --standalone -d admin.example.com
sudo systemctl start admin-backend
```

### データベースエラー
```bash
# SQLiteの整合性チェック
sqlite3 /opt/admin_backend/data/admin.db "PRAGMA integrity_check;"

# バックアップから復元
sudo systemctl stop admin-backend
sudo -u admin cp /opt/admin_backend/data/backup/admin.db.YYYYMMDD /opt/admin_backend/data/admin.db
sudo systemctl start admin-backend
```

### gRPCデバッグ
```bash
# grpcurlでテスト（要インストール）
grpcurl -H "api-key: YOUR_API_KEY" admin.example.com:50051 list

# ヘルスチェック
grpcurl -H "api-key: YOUR_API_KEY" admin.example.com:50051 admin.AdminSync/GetSyncStatus
```

## 運用上の注意

1. **証明書更新**
   - Let's Encryptは90日で期限切れ
   - 自動更新が動作しているか定期的に確認

2. **バックアップ**
   - 最低でも週1回はバックアップ
   - 重要な変更前は必ず手動バックアップ

3. **ログ監視**
   - `/var/log/syslog`でエラーチェック
   - ディスク容量に注意

4. **セキュリティ**
   - APIキーは絶対に公開しない
   - 定期的にキーをローテーション
   - 不要なポートは閉じる

## 今後の拡張予定

- [ ] Web管理画面
- [ ] 差分同期の最適化
- [ ] コンフリクト解決UI
- [ ] 監視ダッシュボード