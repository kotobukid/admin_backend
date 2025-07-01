# Admin Backend

別プロジェクトの複数拠点開発用同期サーバー。カードゲームのデータベース開発において、複数の開発環境間でのデータ同期を管理します。

## 現在の実装状況

### ✅ 実装済み機能
- gRPCサーバー（ポート50051）
- SQLiteデータベース接続・マイグレーション
- APIキー認証システム
- カード機能オーバーライドの同期（Push/Pull）
- 機能確認の記録
- TLS/SSL対応（Let's Encrypt証明書サポート）

### 🚧 未実装機能
- ルールパターン同期
- 確認済み機能の取得・取消し
- Web管理画面

## クイックスタート

### 1. 開発環境での起動
```bash
# リポジトリをクローン
git clone <repository>
cd admin_backend

# ビルドと起動
RUST_LOG=info cargo run
```

### 2. データベースの確認
```bash
# テーブル一覧
sqlite3 data/admin.db ".tables"

# スキーマ確認
sqlite3 data/admin.db ".schema"
```

### 3. gRPCテスト（APIキーなしで接続テスト）
```bash
# サービス一覧の取得（認証エラーになるが接続は確認できる）
grpcurl -plaintext localhost:50051 list
```

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

## APIキー管理

APIキーの管理には専用CLIツールを使用します：

### APIキー生成
```bash
# CLIツールのビルド
cargo build --release --bin admin-cli

# APIキーの生成（read_write権限）
./target/release/admin-cli generate --client "dev-machine-1" --permissions read_write

# 読み取り専用APIキーの生成
./target/release/admin-cli generate --client "monitoring-service" --permissions read

# APIキー一覧表示
./target/release/admin-cli list

# 特定クライアントの情報表示
./target/release/admin-cli info --client "dev-machine-1"

# APIキーの取り消し（確認プロンプトあり）
./target/release/admin-cli revoke --client "old-client"
```

**重要**: 生成されたAPIキーは一度しか表示されません。安全に保管してください。

## SQLiteマイグレーション管理

プロジェクトではSQLxを使用してマイグレーションを管理しています：

### マイグレーション基本コマンド
```bash
# マイグレーション状況確認
DATABASE_URL=sqlite://data/admin.db sqlx migrate info

# 新しいマイグレーション作成
DATABASE_URL=sqlite://data/admin.db sqlx migrate add <description>

# マイグレーション実行（手動）
DATABASE_URL=sqlite://data/admin.db sqlx migrate run

# 最新マイグレーションを取り消し
DATABASE_URL=sqlite://data/admin.db sqlx migrate revert

# オフラインクエリキャッシュ生成（CI/CDで有用）
DATABASE_URL=sqlite://data/admin.db cargo sqlx prepare
```

### 自動マイグレーション
サーバー起動時に自動的にマイグレーションが実行されるため、通常は手動実行は不要です。

## gRPCテスト・動作確認

### grpcurlのインストール
```bash
# Goが必要
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest

# PATHに追加（~/.bashrcに記載）
export PATH="$PATH:~/go/bin"
```

### 基本的なテスト
```bash
# サーバー起動
RUST_LOG=info cargo run

# 別ターミナルで以下を実行
# APIキー生成
./target/debug/admin-cli generate --client test-client --permissions read_write

# 生成されたAPIキーを使用してテスト
API_KEY="ADM_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# ヘルスチェック
grpcurl -plaintext -proto proto/admin.proto -H "api-key: $API_KEY" localhost:50051 admin.AdminSync/GetSyncStatus

# データ挿入テスト
echo '{"pronunciation": "テストカード", "fixed_bits1": 12345, "fixed_bits2": 67890, "fixed_burst_bits": 999, "note": "テストデータ"}' | \
grpcurl -plaintext -proto proto/admin.proto -H "api-key: $API_KEY" -d @ localhost:50051 admin.AdminSync/PushFeatureOverrides

# データ取得テスト
echo '{}' | \
grpcurl -plaintext -proto proto/admin.proto -H "api-key: $API_KEY" -d @ localhost:50051 admin.AdminSync/PullFeatureOverrides
```

### 機能確認テスト
```bash
# 機能確認の記録
echo '{"pronunciation": "テストカード", "feature_bits1": 12345, "feature_bits2": 67890, "burst_bits": 999, "rule_version": "v1.0"}' | \
grpcurl -plaintext -proto proto/admin.proto -H "api-key: $API_KEY" -d @ localhost:50051 admin.AdminSync/ConfirmFeatures
```

### 注意事項
- APIキーはメタデータの`api-key`フィールドで指定
- ストリーミングメソッドには`-d @`でJSONデータを渡す
- 権限エラーの場合は`read_write`権限のAPIキーを使用

## TLS/SSL設定

### 基本設定

TLS（HTTPS/gRPCS）を有効にするには、環境変数で証明書のパスを指定します：

```bash
# 環境変数の設定
export TLS_CERT_PATH=/etc/letsencrypt/live/YOUR_DOMAIN/fullchain.pem
export TLS_KEY_PATH=/etc/letsencrypt/live/YOUR_DOMAIN/privkey.pem

# サーバー起動
RUST_LOG=info cargo run
```

### Let's Encrypt証明書の取得

```bash
# certbotで証明書取得
sudo certbot certonly --standalone -d your-domain.com --register-unsafely-without-email --agree-tos
```

### TLS接続テスト

```bash
# TLS有効時のテスト（-plaintextオプションを削除）
grpcurl -proto proto/admin.proto -H "api-key: YOUR_API_KEY" your-domain.com:50051 admin.AdminSync/GetSyncStatus
```

詳細な設定方法は [tls_setup.md](tls_setup.md) を参照してください。

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
# 開発環境（プレーンテキスト）
grpcurl -plaintext -H "api-key: YOUR_API_KEY" localhost:50051 list

# 本番環境（TLS）
grpcurl -H "api-key: YOUR_API_KEY" admin.example.com:50051 list

# ヘルスチェック
grpcurl -plaintext -H "api-key: YOUR_API_KEY" localhost:50051 admin.AdminSync/GetSyncStatus
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

## 開発メモ

### 依存関係
- Rust 1.70以上
- SQLite 3.35以上（JSON関数サポートのため）
- protobuf-compiler（protocコマンド）

### ビルド最適化
```bash
# リリースビルド
cargo build --release

# サイズ最適化
cargo build --release --profile=release
```

### テスト用データの投入
```bash
# カード機能オーバーライドのテストデータ
sqlite3 data/admin.db <<EOF
INSERT INTO card_feature_override (pronunciation, fixed_bits1, fixed_bits2, fixed_burst_bits, created_at, updated_at)
VALUES 
  ('テストカード', 1, 2, 3, datetime('now'), datetime('now')),
  ('サンプルカード', 4, 5, 6, datetime('now'), datetime('now'));
EOF
```

## 今後の拡張予定

- [x] APIキー生成CLIツール
- [ ] TLS証明書の自動設定
- [ ] GetConfirmedFeatures/UnconfirmFeature実装
- [ ] PushRulePatterns/PullRulePatterns実装
- [ ] Web管理画面
- [ ] 差分同期の最適化
- [ ] コンフリクト解決UI
- [ ] 監視ダッシュボード
- [ ] Docker対応
- [ ] GitHub Actions CI/CD