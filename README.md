# 🕰️ Chronosync - Lightweight Task Scheduler Daemon

Chronosyncは、Rustで構築された超軽量なCLIタスクスケジューラー（デーモン）です。
JSON設定ファイルに基づき、cronライクなスケジュールで外部コマンドを永続的に実行します。
設定ファイルの変更を即座に検知し、実行中のタスクを停止・再構築（リロード）するホットリロード機能を備えています。

## ✨ 特徴

* **🛡️ 堅牢な設計:** Rustの非同期ランタイム `tokio` を採用し、安定した長時間稼働を実現。
* **🔄 ホットリロード:** 設定ファイル（`config.json`）の変更をリアルタイムで監視。サービスを再起動することなく、保存と同時にタスク定義を更新します。
* **⏰ 詳細なスケジューリング:** 秒単位を含む [Cron形式](https://crates.io/crates/cron) で、精密な実行時刻の指定が可能。
* **🚀 高パフォーマンス:** シングルバイナリで動作し、リソース消費を最小限に抑えます。

## 📦 インストールとビルド

ソースコードからビルドしてインストールします。

```bash
# リポジトリのクローン
git clone https://github.com/coil398/Chronosync.git
cd Chronosync

# ローカルへのインストール (パスが通った場所にバイナリが配置されます)
cargo install --path .
```

## ⚙️ 設定ファイル (`config.json`)

Chronosyncの動作には設定ファイルが必須です。
アプリケーションは起動時に標準的な設定ディレクトリ（XDG Base Directoryなど）を検索します。

### 設定ファイルの配置場所

設定ディレクトリに `config.json` を作成してください。

| OS | パス |
| :--- | :--- |
| **Linux / macOS** | `~/.config/chronosync/config.json` |

> 💡 **ヒント:** 初回起動時に設定ファイルが見つからない場合、Chronosyncは検索したパスをエラーログに出力して終了します。そのパスを参考にファイルを配置してください。

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
cargo run
```

### Systemd ユーザーサービスとして登録 (Linux)

常駐プロセスとして動作させる場合、systemdのユーザーサービスとして登録するのが便利です。

1.  **ユニットファイルの作成**
    `~/.config/systemd/user/chronosync.service` を作成します（ディレクトリがない場合は作成してください）。

    ```ini
    [Unit]
    Description=Chronosync Task Scheduler
    After=network.target

    [Service]
    # cargo installでインストールしたバイナリのパスを指定
    # "which chronosync" コマンドで確認できます (例: /home/ユーザー名/.cargo/bin/chronosync)
    ExecStart=%h/.cargo/bin/chronosync
    
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
    systemctl --user start chronosync

    # 自動起動の有効化
    systemctl --user enable chronosync
    ```

3.  **ログの確認**

    ```bash
    journalctl --user -u chronosync -f
    ```

## 📂 プロジェクト構成

```
.
├── src/
│   ├── main.rs       # エントリーポイント、初期化、メインループ
│   ├── config.rs     # 設定ファイルの定義と読み込みロジック
│   ├── scheduler.rs  # タスクのスケジューリングと実行管理
│   └── watcher.rs    # 設定ファイルの変更監視
├── config.json       # 設定ファイルのサンプル
└── Cargo.toml        # 依存関係定義
```
