# Universal Node Graph Engine
## High-Performance GPU Node Graph Framework
Version 0.1

---

## 1. 概要

Universal Node Graph Engine（仮称: **UNGE**）は、ComfyUI、Blender Geometry Nodes、Unreal Blueprint、TouchDesignerなどに見られるノードベースUIを、特定用途に依存しない汎用コンポーネントとして提供する。

主要目標は、

- 大規模グラフ
- GPU高速描画
- 型付きPort
- 非同期処理
- CPU/GPU/AI処理
- Undo/Redo
- Copy/Paste
- Subgraph
- Auto Layout
- Plugin
- Serialization
- Collaboration
- Headless execution

を共通基盤として提供することである。

UIと実行エンジンは完全に分離する。

```text
Application
     │
     ▼
┌────────────────────────────┐
│       Node Graph API       │
├────────────────────────────┤
│ Graph Model                │
│ Execution Engine           │
│ Layout Engine              │
│ Interaction Engine         │
│ Plugin System              │
├────────────────────────────┤
│ Renderer                   │
│ ├ WebGPU / wgpu            │
│ └ Fallback Renderer        │
├────────────────────────────┤
│ Platform Adapter           │
│ Web / Tauri / Native       │
└────────────────────────────┘
```

---

# 2. 基本思想

最重要な設計原則は、

**Graph ≠ UI**

とすることである。

グラフそのものは純粋なデータ構造として保持し、表示方法から完全に独立させる。

```text
Graph Model
    │
    ├── Renderer
    ├── Executor
    ├── Serializer
    ├── AI
    ├── Collaboration
    └── Analyzer
```

これにより同じグラフを、

- GUI
- CLI
- Server
- AI Agent
- Batch Processing

から利用できる。

---

# 3. 推奨技術構成

Desktop:

```text
Tauri 2
   │
   ├ Rust Core
   │
   └ WebView UI
          │
          └ WebGPU
```

Core:

```text
Rust
```

Renderer:

```text
wgpu
```

Web:

```text
WebGPU
```

UI:

```text
React
```

ただし大量ノードの描画はReact DOMに依存させない。

Reactは、

```text
Toolbar
Inspector
Menu
Property Panel
Dialogs
```

など通常UIのみ担当する。

Graph CanvasはGPU描画する。

---

# 4. Workspace

最上位概念をWorkspaceとする。

```rust
Workspace {
    graphs
    assets
    plugins
    history
    settings
}
```

1 Workspaceは複数Graphを持つ。

```text
Workspace

 ├ Graph A
 ├ Graph B
 ├ Graph C
 └ Assets
```

---

# 5. Graph

Graphは以下から構成する。

```rust
Graph {
    id
    metadata

    nodes
    edges
    groups

    viewport
}
```

Graph自体にはUI状態を極力含めない。

論理データと表示データは分離する。

---

# 6. Node

基本Nodeモデル。

```rust
Node {
    id: NodeId

    type_id: NodeTypeId

    position: Vec2
    size: Vec2

    inputs: Vec<Port>
    outputs: Vec<Port>

    properties: PropertyMap

    metadata: Metadata
}
```

Node IDにはUUIDまたはUUIDv7を使用する。

---

# 7. Node Type

Node InstanceとNode Definitionを分離する。

```text
Node Definition

ImageLoader
Multiply
LLM
Prompt
Branch
Merge
```

↓

```text
Node Instance

ImageLoader #9382
ImageLoader #1938
```

NodeType:

```rust
NodeDefinition {
    type_id
    name
    category

    input_schema
    output_schema

    property_schema

    executor
}
```

---

# 8. Port

PortはNode間のデータ接続点。

```rust
Port {
    id
    name

    direction

    data_type

    cardinality
}
```

Direction:

```text
Input
Output
```

Cardinality:

```text
Single
Multiple
```

---

# 9. Type System

Portは型を持つ。

基本型:

```text
Bool
Int
Float
String
Bytes
Image
Audio
Video
Tensor
JSON
Object
Event
Any
```

さらにアプリケーション独自型を登録可能。

例:

```text
KOMYAKU:

Story
Chapter
Character
Scene
Timeline
```

AI:

```text
Prompt
Embedding
Latent
Tensor
Model
```

型安全な接続を基本とする。

```text
Image → Image
Float → Float

Image → Float
        ×
```

---

# 10. 型変換

明示的Converterを登録できる。

```text
Int
 │
 ▼
Float
```

Converter Registry:

```rust
TypeConverter {
    from
    to
    cost
    converter
}
```

自動変換可能な場合はUI上で変換Nodeを省略表示してもよい。

---

# 11. Edge

```rust
Edge {
    id

    from_node
    from_port

    to_node
    to_port

    metadata
}
```

EdgeはNode IDではなく、

```text
Node + Port
```

を接続する。

---

# 12. Edge Rendering

接続線は複数方式を選択可能とする。

```text
Bezier
Straight
Orthogonal
Spline
```

デフォルト:

```text
Cubic Bezier
```

制御点:

```text
P0 = Output
P3 = Input

dx = max(50, abs(P3.x - P0.x) * 0.5)

P1 = P0 + (dx,0)
P2 = P3 - (dx,0)
```

---

# 13. Graph Storage

内部表現ではVecだけではなくHashMapを基本とする。

```rust
nodes: HashMap<NodeId, Node>
edges: HashMap<EdgeId, Edge>
```

さらに高速探索用Indexを持つ。

```text
Node → Input Edges
Node → Output Edges

Port → Edge
```

---

# 14. Spatial Index

大量Node対応のためSpatial Indexを導入する。

候補:

```text
R-tree
BVH
Quadtree
```

用途:

```text
Viewport Culling
Selection
Hit Test
LOD
```

---

# 15. GPU Renderer

RendererはImmediate Modeに近い方式を採用する。

```text
Graph
  ↓
Render Scene
  ↓
Visible Objects
  ↓
GPU Buffers
  ↓
wgpu
```

GPUで描画するもの:

```text
Node body
Node border
Port
Edge
Selection
Grid
Group
Minimap
```

文字はGlyph Atlas方式。

---

# 16. GPU Instancing

Nodeを1個ずつDraw Callしない。

```text
Node Instances
      ↓
Instance Buffer
      ↓
GPU
```

Edgeも可能な限りBatch処理する。

目標:

```text
10,000 Nodes
30,000 Edges
60 FPS
```

通常操作でCPU負荷を極力増やさない。

---

# 17. Viewport Culling

Viewport外Nodeは描画しない。

```text
World

 ┌─────────────────────────┐
 │                         │
 │     ┌──────────┐        │
 │     │ Viewport │        │
 │     └──────────┘        │
 │                         │
 └─────────────────────────┘
```

Spatial IndexからVisible Nodeのみ取得する。

---

# 18. LOD

ズーム倍率によって描画量を変更する。

遠距離:

```text
┌──────┐
│ Node │
└──────┘
```

中距離:

```text
┌───────────────┐
│ Image Loader  │
●               ●
└───────────────┘
```

近距離:

```text
┌─────────────────────┐
│ Image Loader        │
│                     │
│ File                │
│ image.png           │
│                     │
● Path          Image ●
└─────────────────────┘
```

---

# 19. Interaction Engine

Rendererと入力処理も分離する。

```text
Pointer Event
      ↓
Interaction Engine
      ↓
Hit Test
      ↓
Command
      ↓
Graph Model
```

対応:

```text
Pan
Zoom
Node Drag
Multi Select
Box Select
Edge Connect
Edge Disconnect
Copy
Paste
Delete
Duplicate
Group
```

---

# 20. Command System

Graph変更を直接行わない。

すべてCommand経由とする。

```rust
trait Command {

    fn execute();

    fn undo();

}
```

例:

```text
AddNode
DeleteNode
MoveNode
Connect
Disconnect
SetProperty
GroupNodes
```

これによりUndo/Redoを統一する。

---

# 21. Transaction

大量変更はTransactionとしてまとめる。

```text
Transaction

 ├ Add Node
 ├ Add Node
 ├ Connect
 ├ Connect
 └ Move
```

UndoするとTransaction全体を戻す。

---

# 22. Execution Engine

UIとは別にExecution Engineを持つ。

```text
Graph
 ↓
Compiler
 ↓
Execution Plan
 ↓
Scheduler
 ↓
Executor
```

---

# 23. Graph Compiler

実行前にGraphを解析する。

```text
Validation
Type Check
Cycle Detection
Dependency Analysis
Optimization
```

結果:

```rust
ExecutionPlan
```

---

# 24. DAG Execution

基本Execution GraphはDAGとして扱う。

```text
A
├─ B
└─ C
   │
   D
```

依存関係を解析し、

```text
A

↓

B + C

↓

D
```

のように並列実行する。

---

# 25. Cyclic Graph

AI、シミュレーション、SNN、ゲームロジックなどではCycleが必要になる。

そのためGraph Modeを設ける。

```text
DAG
Reactive
Streaming
Simulation
```

SimulationではCycleを許可する。

---

# 26. Execution Model

Node executor:

```rust
trait NodeExecutor {

    async fn execute(
        &self,
        context: ExecutionContext,
        inputs: Inputs
    ) -> Result<Outputs>;

}
```

---

# 27. Execution Backend

Executorは複数Backendに対応する。

```text
CPU
GPU
Remote
Python
AI API
WASM
External Process
```

Nodeごとに実行先を変更可能。

---

# 28. Asynchronous Nodes

長時間処理に対応する。

例:

```text
LLM API
Image Generation
Video Encoding
HTTP
Database
```

状態:

```text
Idle
Queued
Running
Completed
Failed
Cancelled
```

---

# 29. Streaming

Streaming Outputも扱う。

```text
LLM

token
token
token
token
```

Port Type:

```text
Stream<T>
```

---

# 30. Cache

Node outputをキャッシュする。

```text
Hash(
 node_type
 inputs
 properties
)
```

同じ入力なら再実行しない。

ComfyUI型ワークフローでは特に重要となる。

---

# 31. Dirty Propagation

変更されたNodeから下流だけDirtyにする。

```text
A → B → C → D

B変更

A = cached
B = dirty
C = dirty
D = dirty
```

---

# 32. Subgraph

Graph自体をNode化できる。

```text
Main Graph

A
 │
 ▼
┌──────────────┐
│ Subgraph     │
└──────────────┘
 │
 ▼
B
```

Subgraph内部:

```text
X → Y → Z
```

---

# 33. Group

視覚的なGroupを提供する。

```text
┌─────────────────────────┐
│ Image Processing        │
│                         │
│ A → B → C               │
│                         │
└─────────────────────────┘
```

GroupはExecutionには影響しない。

---

# 34. Frame

Blender方式のFrame Nodeも実装可能とする。

Frame内部Nodeをまとめて移動できる。

---

# 35. Comment

Graph上に、

```text
Text
Markdown
Image
Link
```

を配置可能。

---

# 36. Auto Layout

Layout Engineは独立モジュールとする。

対応:

```text
Left → Right
Top → Bottom
Radial
Hierarchical
Force Directed
```

推奨アルゴリズム:

```text
Sugiyama
```

---

# 37. Edge Routing

大規模GraphではEdge Routingが重要。

```text
Obstacle Avoidance
Edge Bundling
Orthogonal Routing
```

をオプション提供する。

---

# 38. Minimap

Graph全体を縮小表示。

```text
┌──────────────────┐
│   ▪ ▪            │
│      ▪▪▪         │
│          ▪       │
│        [VIEW]    │
└──────────────────┘
```

---

# 39. Search

Command Palette:

```text
⌘K

Image
AI
Math
Story
Audio
```

検索結果からNodeを生成する。

---

# 40. Smart Connection

Edgeを空白へDropした場合、

```text
Image Output
      │
      ▼

     Drop
```

接続可能Nodeだけ表示する。

```text
Image Resize
Image Save
Image AI
Image Filter
```

---

# 41. Serialization

標準形式はJSONとする。

```json
{
  "version": "1",
  "nodes": [],
  "edges": [],
  "groups": []
}
```

ただし巨大Graph用にBinary形式も提供する。

候補:

```text
MessagePack
CBOR
FlatBuffers
```

---

# 42. Schema Version

必ずSchema Versionを持たせる。

```text
graph_version
engine_version
plugin_versions
```

Migration:

```text
v1
 ↓
v2
 ↓
v3
```

をサポートする。

---

# 43. Plugin System

Node TypeをPluginから追加可能にする。

```text
Plugin
 ├ manifest
 ├ nodes
 ├ executor
 └ assets
```

Manifest:

```json
{
  "id": "image-tools",
  "version": "1.0.0",
  "nodes": [
    "resize",
    "crop"
  ]
}
```

---

# 44. Plugin Sandbox

PluginをCoreと同一権限で動かさない。

基本:

```text
WASM Sandbox
```

特権処理のみ明示的Permissionを要求する。

```text
filesystem.read
filesystem.write
network
gpu
process
```

---

# 45. AI操作

重要機能として、AIがGraphを直接操作できるようにする。

GUI操作ではなくGraph APIを利用する。

```text
AI
 │
 ▼
Graph API
 │
 ├ create_node
 ├ connect
 ├ disconnect
 ├ move
 ├ set_property
 ├ create_group
 └ execute
```

これにより、

「この画像をリサイズしてSDXLで変換して保存するGraphを作って」

のような指示からGraphを生成できる。

---

# 46. Semantic Node Metadata

AIがNodeを理解できるよう、Node Definitionには意味情報を持たせる。

```json
{
  "name": "Resize Image",

  "description":
  "Resize an image while preserving aspect ratio.",

  "inputs": {
    "image": "Image"
  },

  "outputs": {
    "image": "Image"
  }
}
```

---

# 47. Graph Validation

Validator:

```text
Missing Input
Invalid Type
Broken Edge
Missing Plugin
Cycle Error
Invalid Property
Unavailable Backend
```

を検出する。

---

# 48. Error Visualization

エラーNode:

```text
┌────────────────────┐
│ Image Loader       │
│                    │
│ ⚠ File not found   │
└────────────────────┘
```

Graph実行を止める必要がないエラーは部分的に継続する。

---

# 49. Collaboration

将来的にリアルタイム共同編集へ対応する。

```text
Client A
Client B
Client C

    │
    ▼

Graph Sync
```

CRDTを使用可能な設計にする。

候補:

```text
Yjs
Automerge
独自Rust CRDT
```

---

# 50. Graph Diff

Gitとの親和性を重視する。

単純なJSON line diffだけではなく、

```text
Node Added
Node Removed
Node Changed
Edge Added
Edge Removed
Property Changed
```

というSemantic Diffを提供する。

---

# 51. Git Integration

Graph保存単位:

```text
.graph.json
```

Git Diff Viewer:

```text
Before                    After

A → B                     A → B