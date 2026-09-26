# UNGEを取り込むAIへの指示

1. `README.md`、`docs/AI_INTEGRATION.md`、`docs/ARCHITECTURE.md`、`docs/ROADMAP.md`を読む。
2. 原設計は `docs/Universal-Node-Graph-Engine設計仕様書.md`。ロードマップの[Later]を実装済みと解釈しない。
3. GraphとDocumentにTauri、wgpu、React依存を追加しない。表示座標はDocumentのplacement、View状態はRustサービスに置く。
4. 永続的な編集はCommand経由にする。大規模変更はBatchにまとめる。フレーム単位のGPU画像をIPCに送らない。
5. 既存APIを使ってアプリを作る場合、まずホスト側からRegistryにDefinitionとNodeExecutorを追加する。用途固有ロジックをcoreに埋め込まない。
6. 外部プラグインをこのin-process Registryで無条件に実行しない。Registryは信頼済みRustコードの拡張点であり、WASMサンドボックスではない。
7. TauriウインドウにDocumentの複製を持たせない。編集はexpected_revisionを付けて共有Engineへ送る。競合時に自動上書きしない。
8. 日本語・英語・简体中文のユーザー向け文言を揃える。技術的なエラー文字列はログ用、UIはcodeを翻訳する。
9. 変更した契約に応じてRustテスト、TypeScript型、取り込み文書を更新する。`cargo fmt`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`を実行する。
10. GPUを変更した場合は実GPUのignoredテストを明示的に実行し、GPU利用不可なら未検証と報告する。
11. ロードマップには `[Done]` / `[Next]` / `[Later]` を使い、未実装機能や性能目標を完成扱いしない。
