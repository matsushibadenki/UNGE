# Dependency maintenance — 2026-10-12

## 日本語

開始時の作業ツリーはclean。Rust/Cargo 1.99.0、macOSで検証する。直接依存15種類の最新stable・MSRV・公開日時を公式crates.io APIから取得した。現行のすべての依存が最新版になったという意味ではない。

| 直接依存 | 更新前 | 今回の採用版 | 最新stable候補 |
|---|---|---|---|
| jsonschema (CI) | >=4.18,<5 | 同じ範囲（4.26.0で検証） | 4.26.0 |
| TypeScript (CI) | 5.9.3 | 7.0.2 | 7.0.2 |
| sha2 | 0.10.9 | 0.11.0 | 0.11.0 |
| pollster | 0.4.0 | 1.0.1 | 1.0.1 |
| serde_json | 1.0.151 | 1.0.154 | 1.0.154 |
| thiserror | 2.0.20 | 2.0.21 | 2.0.21 |
| uuid | 1.26.1 | 1.28.0 | 1.28.0 |
| Tauri | 2.11.6 | 2.11.6 | 2.12.3 |
| tauri-runtime-wry | 2.11.4 | 2.11.4 | 2.12.1 |
| tauri-build | 2.6.3 | 2.6.3 | 2.7.2 |
| wgpu | 27.0.1 | 27.0.1 | 30.0.1 |
| cosmic-text | 0.15.0 | 0.15.0 | 0.19.0 |
| schemars | 0.8.22 | 0.8.22 | 1.2.2 |

bytemuck 1.25.2、chrono 0.4.45、futures 0.3.34、serde 1.0.229は確認時の最新stableを維持。更新前後の推移依存は[lock差分](dependencies/2026-10-12-lock-changes.json)、採用版の公式情報は[解決版監査](dependencies/2026-10-12-resolved.json)、全直接依存の候補/MSRVは[直接依存監査](dependencies/2026-10-12-direct.json)を参照。取得日時は2026-10-12 JST。Cargo.lockが採用組み合わせの正本。

TypeScript 7.0.2はstrict IPC型チェックが成功したためCIの固定版を更新した。公式npm metadataのNode要件は>=16.20.0（CIはNode 22）。[公開元とintegrity](dependencies/2026-10-12-typescript.json)。

jsonschemaはCIの`>=4.18,<5`が最新stable 4.26.0を許可するため範囲は維持した。一時領域へ4.26.0を導入し、Schema契約6件とACX相互運用5件が成功した。[公式PyPIの版情報](dependencies/2026-10-12-jsonschema.json)。

### 互換性と次の移行

- sha2 0.11のDigest戻り値は従来のLowerHex実装を持たないため、各byteを2桁の小文字16進数で出力する。`sha256:` + 64桁のwire契約を維持する。空入力とabcの既知ベクトル、Rust/PythonのACX照合で検証する。sha2のMSRVは1.85、pollsterは1.69。Rust 1.99で検証し、古いRustの動作は保証しない。
- 一括の`cargo update`では、固定Tauri/Wryと新tauri-runtime 2.12.1のtraitが不整合になった。runtimeを2.11.3へ戻しても、tauri-macros/codegen 2.7.2が旧Tauriに存在しないUnexpectedMenuKindを生成して失敗した。runtime 2.11.3、build/macros/codegen 2.6.3、utils 2.9.3を復元した。[失敗した更新のコンパイル記録](dependencies/2026-10-12-rejected-update.log)。次回もCargo.lockを保護し、Tauri本体/Wry/runtime/build/macros/codegen/utilsを一組として更新する。lockだけの無条件更新は互換性を保証しない。
- 🟠 [Next] Tauri 2.12.3 / Wry 2.12.1へまとめて移行し、native入力、子WebView合成、reparent、dock/float、配置復元をmacOSで確認する。今回は旧組み合わせを維持した。
- 🟠 [Next] cosmic-text 0.19.0へ移行する。0.19でBuffer setterがlazyとなり、0.16でhinting API、0.17でfallbackが変わるため、整形タイミング・Glyph Atlas・3言語の欠字/クリップ/重なりを検証してからexact pinを変更する。今回は0.15.0を維持した。
- 🟠 [Next] wgpu 30.0.1へ段階移行する。27→28→29→30のAPI差とnative Surface/診断query/描画パイプラインを確認する。今回は27.0.1を維持した。Rust 1.99は`block 0.1.6`にstatic of uninhabited typeの将来互換性警告を出す。`cargo tree --workspace -i block@0.1.6`でwgpu-hal/metal経由と確認した。将来のRustでのエラー化に備え、wgpu移行時に確認する。現在のClippyは終了コード0。[警告記録](dependencies/2026-10-12-future-incompat.log)。
- 🟠 [Next] schemars 1.2.2の移行。現行はRootSchema/schemaモジュール、Draft7、definitions参照に依存する。型APIとgenerator設定を移植し、Schema/TypeScript生成差分と3,000例以上の判定一致を確認する。今回は0.8.22を維持した。
- ⭕️ [Pending] Windows/Linuxのnative Surface・入力・dock/floatと更新されたOS固有依存の実機検証。WASM実行と各crateの最低Rust版での検証も未実施。

### 検証

- `cargo fmt --all`、`cargo fmt --all -- --check`、`git diff --check`: 成功。
- `cargo test --workspace --locked` / `cargo test --workspace --all-features --locked`: それぞれ149件成功。通常実行のignored GPU4件は別に実行した。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`とall-features版: 終了コード0。上記blockのCargo将来互換性警告は残る。
- `cargo run --locked -p unge-headless`: 42を確認。ACX Providerビルドと`python3 scripts/test_acx.py -v`: 5件成功。
- `cargo test --locked -p unge-render -- --ignored`: 実GPU4件成功。renderの2件、themesの1件、profilerの1件。CJK、2段階の画素密度、クリップ、重なり、テーマ切替、ピクセル、queryを確認。
- `python3 scripts/generate_contracts.py --check`: 17契約一致、生成成果物の変更なし。`scripts/test_contracts.py -v`: 6件成功、3,000例以上のPort値判定一致を含む。jsonschema 4.26.0でも契約6件とACX5件が成功。
- TypeScript 7.0.2でstrict型検査成功。Node 26.10.0とCI相当のNode 22.23.3で確認。
- Panel / Group / Accessible graph / Execution UIの3言語回帰が成功。既存Playwright＋インストール済みChromeでProperties UIの3言語・両テーマ・狭い幅・保存・検証・競合・下書き・JSON・選択競合/回復が成功。
- macOS native Surface診断（debug build）: Apple M4 / Metal、10,000ノード・30,000エッジ、3言語・両テーマ・2段階のZoomの12条件×30フレーム＝360件成功。スキップ0、欠字・GPU Validationエラーなし。[生データ](dependencies/2026-10-12-surface.json)。動作確認として記録し、性能改善率や60FPS認定には使用しない。操作入力とdock/floatの手動再検証は今回未実施。Windows/Linux、WASM、最低Rust版は未検証。

### 一次資料

- [sha2 0.11 API](https://docs.rs/sha2/0.11.0/sha2/) — one-shot Digest API。
- [pollster 1.0.1 API/MSRV](https://docs.rs/pollster/1.0.1/pollster/) — block_onを維持。
- [Tauri 2.12.3 release](https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.12.3)。
- [wgpu releases](https://github.com/gfx-rs/wgpu/releases)。
- [cosmic-text changelog](https://github.com/pop-os/cosmic-text/blob/main/CHANGELOG.md)。
- [schemars 1.2.2 API](https://docs.rs/schemars/1.2.2/schemars/)。

## English

This maintenance batch updates TypeScript (CI), sha2, pollster, serde_json, thiserror, uuid and compatible transitive dependencies. SHA-256 receipts retain the same lowercase 64-digit encoding. It does not upgrade every dependency to the latest stable release. The pinned Tauri stack was restored after runtime trait and code-generation incompatibilities; migrate the entire stack together. wgpu 30, cosmic-text 0.19 and schemars 1.2 require separate API, rendering and schema migrations. Candidate versions, MSRVs and lock changes are recorded in the linked JSON audit files. Rust workspace tests (149 in each feature configuration), Clippy, four real-GPU tests, 17 generated contracts, ACX, strict TypeScript and three-language UI regressions pass. jsonschema 4.26.0 also passes all schema and ACX checks. The macOS native Surface check passes 360 frames across 12 language/theme/zoom conditions on Apple M4/Metal (debug build, zero skipped frames). This is a functional check, not a performance certification. A future-Rust warning from block 0.1.6 remains in the wgpu/Metal dependency chain. Native Windows/Linux, WASM and minimum-toolchain verification remain pending.

## 简体中文

本次维护更新TypeScript（CI）、sha2、pollster、serde_json、thiserror、uuid及兼容的间接依赖。SHA-256 Receipt仍采用相同的64位小写十六进制格式，并非所有依赖均已升级到最新stable版本。Tauri runtime trait及代码生成出现不兼容后，已恢复固定的Tauri组合；后续需整体迁移。wgpu 30、cosmic-text 0.19和schemars 1.2需要分别验证API、渲染和Schema迁移。候选版本、最低Rust版本及lock差异见关联JSON审计文件。两种feature配置各149项Rust测试、Clippy、4项实际GPU测试、17种生成契约、ACX、strict TypeScript及三语UI回归均通过。jsonschema 4.26.0也通过全部Schema和ACX检查。macOS原生Surface在Apple M4/Metal上通过12组语言／主题／缩放条件的360帧检查，跳过0帧；此debug构建用于功能验证，不用于性能认证。wgpu/Metal间接依赖block 0.1.6仍有未来Rust兼容性警告。Windows/Linux原生交互、WASM及最低工具链验证仍待完成。

## 2026-10-14: Tauri一式の移行

Tauri 2.12.3、runtime／runtime-wry 2.12.1、build／macros／codegen 2.7.2、utils 2.10.1、Wry 0.57.0、Tao 0.37.1へまとめて移行した。ホストのTauri／runtime-wry／buildはexact pin、ライブラリーのtauri依存は既存の互換範囲を維持する。前回失敗したtraitと生成APIの不整合は、新しい組み合わせでコンパイルが通り解消した。[公式release](https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.12.3)、[lock差分](dependencies/2026-10-14-tauri-lock.json)。最新版すべてへの更新ではない。

macOSでworkspace/all-features Rust152件、Clippy、17契約と3言語のPanel／Group／Accessible／Execution UI回帰が成功。新しいdebugホストのMetal Surfaceは12条件×30＝360成功サンプル、スキップ0。近景／全体、3言語、両テーマを含む。[Surfaceデータ](dependencies/2026-10-14-tauri-surface.json)。GPU実装自体は今回変更していない。Surfaceの時間はdebugでの動作確認で、速度比較やFPS認定に使わない。

実アプリのQAはBrowserではなく、macOSの一時 .app とCUAを使用した。tauri://localhostの画面・nativeグラフが表示され、切り離し→浮動パネルから追加（6→7ノード、revision 1）→Undo（6ノード、revision 2）→再ドッキングが成功。グラフ上のドラッグでrevision 3とインスペクター追従を確認。通常終了でfloating=trueを保存し、別プロセスで再起動して「設定パネルをドッキング」の浮動ウインドウを確認した。既存の個人用配置は使わず、`UNGE_PANEL_PREFERENCES=/tmp/unge-tauri-qa-layout.json` で隔離した。検証アプリは終了した。

`UNGE_PANEL_PREFERENCES` はサンプルホストの任意の保存先指定。未設定なら従来のapp_config_dir、別ウインドウモードでは従来どおり配置保存なし。実配置ファイルを試験で変更しないための設定で、Document／IPCの形式は変えない。指定先のファイル保存権限と運用はホストが管理する。

一時 .app 起動時にmacOSの `sandbox_extension_issue_file_to_process failed` がstderrへ出た。画面・IPC・reparent・描画操作は成功した。正式な署名付き配布バンドルの検証は別途必要。Window寸法の全組み合わせ・複数モニター・Windows/Linux・WASM・最低Rust版は未検証。wgpu由来のblock将来互換性警告も引き続き残る。

English: The Tauri stack now consistently uses 2.12.3 / runtime and runtime-wry 2.12.1 / build, macros and codegen 2.7.2 / utils 2.10.1, with Wry 0.57.0 and Tao 0.37.1. Previous runtime/codegen incompatibilities are resolved. 152 Rust tests, strict Clippy, 17 contracts and three-language UI regressions pass. Native debug Surface passes 360 samples. macOS CUA confirms floating/docking, shared edits, undo, native dragging and floating-mode restoration after restart. An optional UNGE_PANEL_PREFERENCES path isolates QA storage. Temporary-app macOS sandbox-extension warnings are recorded; signed distribution bundles, other operating systems, multi-monitor geometry and minimum toolchains remain unverified. This is not a latest-version or FPS claim.

简体中文：Tauri整组迁移至2.12.3，runtime／runtime-wry 2.12.1，build／macros／codegen 2.7.2，utils 2.10.1，Wry 0.57.0及Tao 0.37.1。之前runtime／codegen不兼容已解决。152项Rust测试、严格Clippy、17项契约及三语UI回归通过。原生debug Surface通过360样本。macOS CUA确认浮动／停靠、共享编辑、Undo、原生拖动和重启后浮动模式恢复。可选UNGE_PANEL_PREFERENCES路径隔离QA配置。已记录临时app的macOS sandbox-extension警告；签名分发包、其他OS、多屏位置及最低工具链未验证。不代表全部最新版或FPS认证。
