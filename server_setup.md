# Admin Backend サーバー構築手順

このドキュメントでは、VPS上でadmin_backendサーバーを構築し、動作確認を行うまでの手順を説明します。

## 前提条件

- Ubuntu 22.04 LTSのVPS
- ルート権限またはsudo権限
- 基本的なLinuxコマンドの知識

## 1. システム情報の確認

```bash
# システム情報確認
uname -a
free -h  # メモリ使用量確認
```

## 2. 必要なパッケージのインストール

### 2.1 システムの更新

```bash
sudo apt update && sudo apt upgrade -y
```

### 2.2 Rustのインストール

```bash
# Rustupを使用してRustをインストール
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# デフォルトインストールを選択（1を入力）
# インストール後、環境変数を読み込み
source ~/.cargo/env

# インストール確認
rustc --version
```

### 2.3 開発ツールとライブラリのインストール

```bash
# 必要なパッケージを一括インストール
sudo apt install -y build-essential pkg-config libssl-dev sqlite3 certbot libsqlite3-dev protobuf-compiler golang-go
```

## 3. メモリ不足対策（スワップファイル作成）

**重要**: 1GBのRAMしかないVPSでは、Rustのビルド時にメモリ不足が発生します。

```bash
# 2GBのスワップファイルを作成
sudo fallocate -l 2G /swapfile

# 権限設定
sudo chmod 600 /swapfile

# スワップファイルとして初期化
sudo mkswap /swapfile

# スワップを有効化
sudo swapon /swapfile

# 永続化設定（再起動後も有効）
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab

# スワップが有効になったことを確認
free -h
```

## 4. ファイアウォール設定

```bash
# 現在のファイアウォール状態確認
sudo ufw status

# admin_backend用のポート50051を開放
sudo ufw allow 50051/tcp

# 設定確認
sudo ufw status
```

## 5. admin_backendのデプロイ

### 5.1 GitHubからクローン

```bash
# ホームディレクトリに移動
cd ~

# リポジトリをクローン
git clone https://github.com/kotobukid/admin_backend.git

# プロジェクトディレクトリに移動
cd admin_backend
```

### 5.2 データベースディレクトリの作成

```bash
# データ保存用ディレクトリを作成
mkdir -p data
```

### 5.3 環境変数の設定

```bash
# SQLiteのシステムライブラリを使用するように設定
export LIBSQLITE3_SYS_USE_PKG_CONFIG=1
export DATABASE_URL=sqlite://data/admin.db
```

### 5.4 データベースの初期化

```bash
# 空のデータベースファイルを作成
touch data/admin.db

# 初期スキーマを適用
sqlite3 data/admin.db < migrations/001_initial_schema.sql
```

### 5.5 ビルド実行

```bash
# デバッグビルドを実行（メモリ使用量を抑制）
cargo build

# ビルド完了まで数分かかります
# スワップが有効になっているため、メモリ不足エラーは発生しません
```

## 6. 動作確認

### 6.1 APIキーの生成

```bash
# APIキー生成CLIを実行
./target/debug/admin-cli generate --client server-test --permissions read_write

# 生成されたAPIキーをメモしておく
# 例: ADM_cdee4ff73fc64a8a9356a3d72be17d99
```

### 6.2 サーバーの起動

```bash
# ログ出力を有効にしてサーバーを起動
RUST_LOG=info ./target/debug/admin_backend
```

サーバーが正常に起動すると、以下のようなログが表示されます：

```
2025-07-01T05:58:29.241340Z  INFO admin_backend: Starting admin_backend server...
2025-07-01T05:58:29.243237Z  INFO admin_backend::database: Running database migrations...
2025-07-01T05:58:29.244248Z  INFO admin_backend::server: gRPC server listening on 0.0.0.0:50051
```

### 6.3 gRPCテストツールのインストール

新しいターミナル/tmuxペインで以下を実行：

```bash
# プロジェクトディレクトリに移動
cd ~/admin_backend

# Goのパスを設定
export PATH="$PATH:~/go/bin"

# grpcurlをインストール
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest

# APIキーを環境変数に設定（生成されたキーに置き換え）
API_KEY="ADM_cdee4ff73fc64a8a9356a3d72be17d99"
```

### 6.4 gRPC接続テスト

#### ヘルスチェック

```bash
grpcurl -plaintext -proto proto/admin.proto -H "api-key: $API_KEY" localhost:50051 admin.AdminSync/GetSyncStatus
```

成功すると以下のような応答が返ります：

```json
{
  "serverTime": "2025-07-01T06:03:14.000109983Z"
}
```

#### データ挿入テスト

```bash
echo '{"pronunciation": "サーバーテスト", "fixed_bits1": 12345, "fixed_bits2": 67890, "fixed_burst_bits": 999, "note": "サーバーからのテストデータ"}' | grpcurl -plaintext -proto proto/admin.proto -H "api-key: $API_KEY" -d @ localhost:50051 admin.AdminSync/PushFeatureOverrides
```

成功すると以下のような応答が返ります：

```json
{
  "itemsReceived": 1,
  "itemsCreated": 1
}
```

#### データ取得テスト

```bash
echo '{}' | grpcurl -plaintext -proto proto/admin.proto -H "api-key: $API_KEY" -d @ localhost:50051 admin.AdminSync/PullFeatureOverrides
```

成功すると挿入したデータが返ります：

```json
{
  "pronunciation": "サーバーテスト",
  "fixedBits1": "12345",
  "fixedBits2": "67890",
  "fixedBurstBits": "999",
  "createdAt": "2025-07-01T06:03:29.000145323Z",
  "updatedAt": "2025-07-01T06:03:29.000148695Z",
  "note": "サーバーからのテストデータ"
}
```

## 7. トラブルシューティング

### メモリ不足エラー

症状：ビルド中に `(signal: 9, SIGKILL: kill)` エラーが発生

解決策：
1. スワップファイルが正しく設定されているか確認
2. `free -h` でスワップが有効になっているか確認
3. 必要に応じてスワップサイズを増加

### ポート接続エラー

症状：grpcurlで `connection refused` エラー

解決策：
1. サーバーが起動しているか確認
2. ファイアウォールでポート50051が開放されているか確認
3. `ss -tlnp | grep 50051` でポートが待機状態か確認

### API認証エラー

症状：`Invalid API key` エラー

解決策：
1. APIキーが正しく設定されているか確認
2. APIキーの形式が正しいか確認（ADM_から始まる）
3. データベースにAPIキーが保存されているか確認

## 8. 次のステップ

動作確認が完了したら、以下の作業を進めてください：

1. **systemdサービス設定**: 自動起動とプロセス管理
2. **TLS証明書設定**: certbotを使用したHTTPS化
3. **リリースビルド**: 本番用の最適化されたバイナリ作成
4. **監視設定**: ログ管理とヘルスチェック
5. **バックアップ設定**: データベースの定期バックアップ

## 9. 参考情報

- **使用ポート**: 50051 (gRPC)
- **データベース**: SQLite (`data/admin.db`)
- **ログレベル**: `RUST_LOG=info`で設定
- **設定ファイル**: 環境変数経由で設定

## セキュリティ注意事項

- APIキーは安全に管理し、絶対に公開しないでください
- 定期的にAPIキーをローテーションしてください
- 不要なポートは必ず閉じてください
- サーバーのセキュリティアップデートを定期的に適用してください