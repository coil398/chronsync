# 🕰️ chronsync - Lightweight Task Scheduler Daemon

chronsyncは、**Chrono (時間) + cron + sync (同期)** から名付けられました。
chronsyncは、Rustで構築された超軽量なCLIタスクスケジューラー（デーモン）です。
JSON設定ファイルに基づき、cronライクなスケジュールで外部コマンドを永続的に実行します。
設定ファイルの変更を即座に検知し、実行中のタスクを停止・再構築（リロード）するホットリロード機能を備えています。

## ✨ 特徴

* **🛡️ 堅牢な設計:** Rustの非同期ランタイム `tokio` を採用し、安定した長時間稼働を実現。
* **🔄 ホットリロード:** 設定ファイル（`config.json`）の変更をリアルタイムで監視。サービスを再起動することなく、保存と同時にタスク定義を更新します。
* **⏰ 詳細なスケジューリング:** 秒単位を含む [Cron形式](https://crates.io/crates/cron) で、精密な実行時刻の指定が可能。
* **🚀 高パフォーマンス:** シングルバイナリで動作し、リソース消費を最小限に抑えます。
* **💻 クロスプラットフォーム:** Linux, macOS, Windows でのサービス登録をサポート。

## 📦 インストールとビルド

ソースコードからビルドしてインストールします。

```bash
# リポジトリのクローン
git clone https://github.com/coil398/chronsync.git
cd chronsync

# ローカルへのインストール (パスが通った場所にバイナリが配置されます)
car go install --path .
```

## ⚙️ 設定ファイル (`config.json`)

chronsyncの動作には設定ファイルが必須です。
アプリケーションは起動時に標準的な設定ディレクトリ（XDG Base Directoryなど）を検索します。

### 設定ファイルの配置場所

設定ディレクトリに `config.json` を作成してください。
便利な `init` コマンドでひな形を作成することもできます。

```bash
chronsync init
```

| OS | パス |
| :--- | :--- |
| **Linux / macOS** | `~/.config/chronsync/config.json` |

> 💡 **ヒント:** 初回起動時に設定ファイルが見つからない場合、chronsyncは検索したパスをエラーログに出力して終了します。そのパスを参考にファイルを配置してください。

### 設定フォーマット

JSON形式で `tasks` 配列を定義します。

```json
{
  "tasks": [
    {
      "name": "ping_test",
      "cron_schedule": "*/5 * * * * *", 
      "command": "/bin/echo",
      "args": ["Hello World"]
    },
    {
      "name": "shell_script_example",
      "cron_schedule": "0 0 * * * *", 
      "command": "/bin/sh",
      "args": [
        "-c",
        "/bin/echo \"Current date is $(date)\""
      ]
    }
  ]
}
```

* **name**: タスクの識別子（ログ出力に使用）。
* **cron_schedule**: cron形式のスケジュール文字列（秒 分 時 日 月 曜日 年）。
  * 例: `*/1 * * * * *` (毎秒), `0 30 9 * * *` (毎日9:30:00)
* **command**: 実行するコマンドのパス。
* **args**: コマンドへの引数の配列。
  * **注意:** パイプ `|` やリダイレクト `>`、環境変数展開 `$VAR` を使用したい場合は、`command` にシェル（`/bin/sh` や `/bin/bash`）を指定し、`args` で `"-c"` とコマンド文字列を渡してください。

## 🚀 実行方法

### 手動実行 (開発・テスト)

```bash
# コンパイルして実行
car go run -- run
# またはインストール済みなら
chronsync run
```

### 常駐サービスとして登録 (推奨)

`chronsync` はサービス管理機能を内蔵しており、コマンド一つで常駐サービス（デーモン）として登録・管理できます。
Linux (Systemd), macOS (Launchd), Windows (Service) に対応しています。

#### `--user` (グローバルオプション): ユーザーサービスとして操作

`chronsync` の `service` コマンドは、デフォルトではシステムレベルのサービス（通常 `sudo` が必要）として操作しようとします。
現在のユーザーの権限でサービスを管理したい場合は、`--user` グローバルオプションを付けてください。

```bash
# システムサービスとしてインストール (sudo が必要)
sudo chronsync service install

# ユーザーサービスとしてインストール (sudo 不要)
chronsync --user service install
```

#### 1. サービスのインストール (登録)

自動起動の設定も行われます。

```bash
# システムサービスとしてインストール (sudo が必要)
sudo chronsync service install

# ユーザーサービスとしてインストール (sudo 不要)
chronsync --user service install
```

#### 2. サービスの開始

インストール後、サービスを開始します。

```bash
# システムサービス
sudo chronsync service start

# ユーザーサービス
chronsync --user service start
```

#### その他のコマンド

*   **停止:** `chronsync --user service stop` (ユーザーサービスの場合) または `sudo chronsync service stop` (システムサービスの場合)
*   **アンインストール:** `chronsync --user service uninstall` (ユーザーサービスの場合) または `sudo chronsync service uninstall` (システムサービスの場合)

#### ログの確認 (Linux - Systemd)

サービスのログは、`journalctl` コマンドを使う代わりに、`chronsync service log` コマンドで簡単に確認できます。

```bash
# システムサービスのログを表示 (デフォルト20行、リアルタイム監視)
sudo chronsync service log -f

# ユーザーサービスのログを表示 (デフォルト20行、リアルタイム監視)
chronsync --user service log -f

# システムサービスの最新100行を表示
sudo chronsync service log -n 100

# ユーザーサービスの最新50行を表示
chronsync --user service log -n 50
```

---

### Systemd ユーザーサービスとして手動登録 (Linux)

手動で細かく設定したい場合は、以下の手順で Systemd に登録できます。

1.  **ユニットファイルの作成**
    `~/.config/systemd/user/chronsync.service` を作成します（ディレクトリがない場合は作成してください）。

    ```ini
    [Unit]
    Description=chronsync Task Scheduler
    After=network.target

    [Service]
    # cargo installでインストールしたバイナリのパスを指定
    # "which chronsync" コマンドで確認できます (例: /home/ユーザー名/.cargo/bin/chronsync)
    ExecStart=%h/.cargo/bin/chronsync run
    
    # 常に再起動
    Restart=always
    RestartSec=5s
    
    # ログ出力設定
    StandardOutput=journal
    StandardError=journal

    [Install]
    WantedBy=default.target
    ```

2.  **サービスの有効化と起動**

    ```bash
    # 設定の再読み込み
    systemctl --user daemon-reload

    # サービスの起動
    systemctl --user start chronsync

    # 自動起動の有効化
    systemctl --user enable chronsync
    ```

3.  **ログの確認**

    ```bash
    journalctl --user -u chronsync -f
    ```

## 📂 プロジェクト構成

```
.
├── src/
│   ├── main.rs       # エントリーポイント
│   ├── commands.rs   # 各コマンドのハンドラ
│   ├── cli.rs        # CLI引数の定義
│   ├── config.rs     # 設定ファイルの定義と読み込みロジック
│   ├── scheduler.rs  # タスクのスケジューリングと実行管理
│   ├── watcher.rs    # 設定ファイルの変更監視
│   └── utils.rs      # ユーティリティ
├── config.json       # 設定ファイルのサンプル
└── Cargo.toml        # 依存関係定義
```