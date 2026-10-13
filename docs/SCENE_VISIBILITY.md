# Sceneの可視ID処理

SceneIndexの既存APIで自動的に利用します。SpatialIndexの検索結果はID順なので、Scene生成ではそのVecを保持し、BTreeSetへの再変換を省きます。プレビューから追加候補がある場合だけ並べ替え、重複を除きます。Node／Edgeの描画順、複数移動ノードで共有する接続線の一度だけの描画、画面外から移動するプレビューの表示を維持します。

Document、共有Engine、IPC、TypeScript、GPUの形式に変更はありません。曲線の分割、LOD、テーマ、文字、Groupの処理は既存経路を使います。SceneやGPUデータのキャッシュを追加しません。

```sh
cargo run --release --locked -p unge-render --example zoom_benchmark
```

10,000ノード／30,000接続で6段階の静的Zoomを測ります。各5回予熱＋30回計測。Scene生成だけを計時し、Quadバイト列と文字のFNV-1a指紋は計時外で算出します。指紋は変更前後の出力確認用で、暗号学的な証明ではありません。Group、選択、プレビューはこの性能fixtureに含みません。既存の回帰テストでそれらを確認します。[数値と生データ](PERFORMANCE.md)。

## English

Existing SceneIndex APIs retain sorted spatial-query vectors instead of rebuilding BTreeSets each frame. Preview candidates are sorted when appended and deduplicated. ID drawing order, shared incident-edge deduplication and previews moving into view remain intact. Document/IPC/TypeScript/GPU formats are unchanged; tessellation, LOD, theme, text and group processing use existing paths.

The release example measures only Scene generation for six static zoom levels on 10,000 nodes / 30,000 edges, five warm-ups and 30 samples each. Untimed FNV-1a fingerprints cover quad bytes and label representations for before/after comparison, not cryptographic proof. This fixture excludes groups, selection and previews; regression tests cover them. See [results](PERFORMANCE.md).

## 简体中文

已有SceneIndex接口直接保留SpatialIndex返回的有序Vec，省去每帧转换BTreeSet。只有追加预览候选时才排序，并去重。保持ID绘制顺序、多个移动节点共享连线的去重及移入视口的预览。Document／IPC／TypeScript／GPU格式不变；曲线细分、LOD、主题、文字和Group继续使用原路径。

release示例对10,000节点／30,000连线测量六档静态Zoom，仅计时Scene生成，每档预热5次、计测30次。在计时外用Quad字节及文字表示计算FNV-1a指纹，供修改前后比较，不是密码学证明。性能数据集不含Group、选择和预览，由回归测试覆盖。参见[结果](PERFORMANCE.md)。
