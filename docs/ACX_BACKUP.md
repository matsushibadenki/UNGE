# Offline backup generations / オフラインバックアップ / 离线备份

## 日本語

`scripts/backup.py`は、停止済みPersistentHostのseed・状態ログ・Receiptログと、ホストが明示した資産を一つの世代へコピーする標準ライブラリだけのUnix向けツールです。3つの必須ファイルと資産それぞれの役割、元のファイル名、バイト数、SHA-256をversion 1のmanifestへ記録します。DocumentやIPCの契約は変更しません。

```sh
# 先にProviderと資産を書き換えるすべてのworkerを正常終了する
python3 scripts/backup.py create private/backup-001 \
  --document private/seed.json --state private/editor.jsonl \
  --replay private/replay.jsonl --resource private/source-image.png
python3 scripts/backup.py verify private/backup-001/generation
python3 scripts/backup.py restore private/backup-001/generation private/restored-001

# 復元先は元と異なる新規ディレクトリ。ファイル名はmanifestのroleで対応する
cargo run --locked -p unge-acx-provider -- \
  --document private/restored-001/generation/000000.bin \
  --state-log private/restored-001/generation/000001.bin \
  --journal private/restored-001/generation/000002.bin
```

- `--resource`は複数指定可能。ディレクトリを自動探索せず、資産参照を自動解決しません。ホストは必要な全資産と、それぞれのResource ID／パス対応を記したホスト用ファイルも明示してください。資産が不要なら省略できます。
- 全入力を開き、同じinodeの重複・通常ファイル以外・末端symlinkを拒否し、Unixの排他flockを維持します。RustのFile::try_lockとの競合をmacOSで実プロセス検証しています。親ディレクトリは信頼済みのものを使用し、バックアップ中に置換しないでください。
- 1MiB単位でコピーしてハッシュを計算し、入力のinode・長さ・変更時刻を再確認します。ファイルとディレクトリをfsyncし、隠れた作業ディレクトリを`generation`へrenameして公開します。新規ディレクトリは0700、ファイルは0600です。元ファイルを変更しません。
- 出力先が存在すれば拒否します。コピー／検証中の失敗では作業コピーを除去します。プロセス中断では隠れた未完了ディレクトリが残る場合があります。`generation`がない世代は完成扱いせず、別の新規出力先を使ってください。公開後のディレクトリ同期失敗はコマンド失敗でも世代が残ることがあり、検証と運用者の判断が必要です。
- restoreは実際に書くバイトを検証し、成功するまで`generation`を公開しません。元の名前は情報用で、出力パスには利用しません。欠損・余分なファイル・未知version・不正role／path・サイズ／hash不一致を拒否します。世代は不変の保管物として保持し、**起動にはrestoreで作ったコピー**を使います。

### 保証の境界

このmanifestはコピーの集合と完全性を記録します。署名や暗号化はなく、manifestごとの悪意ある書き換えや、整合した世代全体の巻き戻しを検出する証明ではありません。停止していない非協調的writer、ネットワークfilesystem、電源断への保証は提供しません。ロックと時刻の確認はホスト停止の代替ではありません。

seedのDocument ID、ログのhash chain、状態とReceiptの相互prefix、Registry互換性は**復元先を開くPersistentHost／Providerが検証**します。このツールは既に不整合な2ログを修復せず、意味上のペア整合性を認定しません。起動失敗時に元世代を編集して通さないでください。元の履歴予算、容量設定、互換Registryを引き継ぎます。復元先の初回起動確認後に通常運用へ切り替えます。[編集保存](ACX_EDITOR_PERSISTENCE.md)・[ログの相互検証](ACX_LOG_PAIRING.md)を参照。

🟢 [Done] macOSでコピー・改変／欠損検出・書込み失敗・重複／symlink／ロック拒否、実Rust writerとの競合と復元後のDocument／revision／Receipt再送を検証。

⭕️ [Pending] Windows対応、Linux実機、実電源断、ネットワークfilesystem、巨大資産集合の運用性能。Tauriへの保存接続、2ログ単一トランザクション、旧ログ移行、圧縮は未実装。

## English

`scripts/backup.py` creates offline immutable generations for a stopped PersistentHost using Python's standard library on Unix. Supply the seed, editor log, replay log and every required resource with repeated `--resource`. Include a host-owned Resource ID/path mapping file if needed; assets are not discovered automatically. The commands above create, verify and restore into a new destination. Roles map document/state/replay to `000000.bin`/`000001.bin`/`000002.bin`. Run the provider on the restored copy, never on the archived generation. Preserve history budgets, capacity settings and a compatible Registry.

Every input is a regular file, final symlinks and duplicate inodes are rejected, and exclusive flock locks remain held through publication. Stop all writers first and use trusted parent directories. One-MiB streaming copies record lengths and SHA-256 in a versioned manifest; metadata is rechecked. Files/directories are synced before publishing a `generation` directory by rename. New directories use 0700 and files 0600. Existing destinations are never overwritten. Failed copies are removed; interrupted staging folders may remain. A missing `generation` is incomplete. A sync failure after publication may leave a generation despite command failure; verify and review it before use.

Restore hashes the bytes it writes before publication. Unknown versions, malformed roles/paths, missing/extra files and integrity mismatches are rejected. Original names are informational only. This is an inventory/integrity manifest, not authentication, rollback protection or a state/receipt transaction. PersistentHost/Provider startup still validates Document ID, hash chains, mutual prefixes and Registry compatibility. The tool neither repairs nor certifies semantic log pairing. Locks/metadata checks do not replace stopping non-cooperative writers. macOS tests cover failures, real Rust writer exclusion and restored Document/revision/original receipt replay. Windows implementation, Linux runtime, power loss, network filesystems and large-resource performance remain pending.

## 简体中文

`scripts/backup.py`使用Python标准库，为已停止的PersistentHost生成Unix离线不可变备份。指定seed、编辑日志、Receipt日志及重复的`--resource`资产；必要时包括宿主的Resource ID／路径映射文件，不自动发现资产。上面的命令可创建、校验并恢复至新目录。document／state／replay分别对应`000000.bin`／`000001.bin`／`000002.bin`。在恢复副本上启动Provider，不直接修改归档世代。保留原历史预算、容量配置及兼容Registry。

所有输入保持排他flock，拒绝重复inode、末端符号链接和非普通文件。先停止全部writer，并使用可信父目录。按1MiB流式复制，manifest记录version、角色、原文件名、长度和SHA-256；再次核对源元数据。同步文件／目录后rename发布`generation`。新目录0700、文件0600，拒绝覆盖已有输出。复制失败时清理临时副本；进程中断可能留下隐藏目录，无`generation`的世代不完整。发布后同步失败可能仍留下世代，需校验并由运维判断。

恢复时核对实际写入字节后才发布；拒绝未知版本、非法角色／路径、缺失／多余文件和hash不符。原名仅供参考。manifest记录清单与完整性，不提供签名、加密、整组回退检测或状态／Receipt单一事务。启动PersistentHost／Provider仍须验证Document ID、日志hash链、相互前缀及Registry兼容性。工具不修复或认证语义配对。锁及元数据检查不能替代停止不协作writer。macOS已验证失败处理、真实Rust writer排他，以及恢复后的Document／revision／原Receipt重放。Windows实现、Linux实机、断电、网络filesystem及大资产性能仍待验证。
