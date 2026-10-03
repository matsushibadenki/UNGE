# 実行キャッシュ

`unge-executor` のSchedulerは、正常終了し出力検証に成功したpureノードの結果をRust内に保存します。既存の `Scheduler::new(concurrency, count)` はそのまま使え、バイト上限は16 MiBです。

```rust
use unge_executor::{CacheLimits, Scheduler};

let mut scheduler = Scheduler::with_cache_limits(4, CacheLimits {
    max_entries: 512,
    max_bytes: 8 * 1024 * 1024,
});
let usage = scheduler.cache_usage(); // entries, bytes, limits
scheduler.set_cache_limits(CacheLimits {
    max_entries: 128,
    max_bytes: 4 * 1024 * 1024,
}); // 変更時に古い結果から即座に削除
scheduler.clear_cache(); // 結果と保持中のexecutor参照を解放
```

## 保存と検索の契約

- 完全なJSONキーはtype_id、version、executor識別子、入力、propertiesで構成します。標準HashMapがハッシュ検索とキーの等価比較を行うので、衝突で異なる入力を取り違えません。検索はエントリ数に対して平均O(1)ですが、シリアライズ・ハッシュ・比較にはキー長に応じた時間が必要です。
- FIFOです。ヒットは保存順を変更せず、同じ階層で同じキーを実行しても保存枠は1件です。同じ階層内の実行自体をまとめる機能はありません。
- 件数とバイト上限を両方満たすまで最古の結果から削除します。どちらかの上限が0なら保存を無効化して既存結果を解放します。
- バイト数はUTF-8のキーと出力JSONの合計です。キーの生成には上限付きwriterを使い、出力は追加のJSONバッファを作らず上限まで数えます。大きすぎる値は正常に実行したうえで保存を省き、既存キャッシュを追い出しません。
- 上限は保持するシリアライズ相当のペイロード容量です。Rustのアロケータ、HashMap、実行中の入力/結果/キー、Report、executor内部、ホストのCPU/GPUリソースのメモリ量は含みません。プロセス全体のメモリ上限ではありません。
- Resourceは不変の内容・バージョンIDを持たせます。リソースの実データと寿命管理はホストの責務です。
- executorをArcで識別し、保存中は参照を保持してアドレス再利用による誤ヒットを防ぎます。別executorの同名・同version定義とは共有しません。同じArcと同じ契約なら別Registryでも共有可能です。変更時はversionを増やしてください。
- `pure: true` は、type/version・入力・propertiesが同じなら結果も同じで、副作用がないというホストの保証です。node ID、時刻、可変リソース、外部状態に依存するexecutorはpureにしません。
- 失敗、出力検証エラー、キャンセルされた結果は保存しません。キャンセル済みrunは保存済み結果も返しません。キャッシュ保存を省く際のシリアライズエラーは実行成功を失敗へ変更しません。

Report、TypeScript、ACXのwire形式は変更ありません。ACX Providerの既存Schedulerにも16 MiBの既定上限が適用されます。Rustホストへの進捗とdeadline Futureは [EXECUTION_PROGRESS.md](EXECUTION_PROGRESS.md) を参照してください。使用例は `examples/headless`、境界・FIFO・隔離・失敗の検証は `crates/unge-executor/tests/cache.rs` にあります。

## English

`Scheduler::new(concurrency, count)` keeps its existing signature and now limits retained serialized cache payloads to 16 MiB. Use `with_cache_limits`, `cache_usage`, `set_cache_limits` and `clear_cache` as shown above. Lower limits evict oldest entries immediately; either zero limit disables caching.

Exact JSON keys include type, version, executor identity, inputs and properties. Rust's standard HashMap checks key equality after hashing, preventing incorrect hits from hash collisions. Lookup is expected O(1) in entry count; serialization and hashing still depend on key length. Hits do not refresh FIFO order. Duplicate keys occupy one slot, but concurrent computations are not coalesced.

The byte budget counts UTF-8 key plus output JSON. Key serialization stops at the budget; output size is counted without creating an output JSON buffer. Oversized entries are skipped without evicting existing entries or failing execution. This excludes allocator/container overhead, in-flight work, reports, executor internals and host CPU/GPU resource storage; it is not a process memory limit. Resources must carry immutable content/version IDs, with storage and lifetime managed by the host.

The cache retains executor Arcs to prevent identity address reuse. Different executors do not share entries even with the same type/version; the same Arc and contract can share across registries. Bump the version when behavior or schema changes. Pure executors must have no side effects and derive results solely from their declared type/version, inputs and properties, without depending on node IDs, time or mutable external state. Failed, invalid and cancelled outputs are not cached. A pre-cancelled run ignores warm entries. Report, TypeScript and ACX wire formats remain unchanged. ACX uses the new default byte budget. Rust progress observers and host-supplied run deadlines are documented in [execution progress](EXECUTION_PROGRESS.md#english).

## 简体中文

`Scheduler::new(concurrency, count)` 保持原有签名，新增16 MiB的序列化缓存容量上限。可用上述 `with_cache_limits`、`cache_usage`、`set_cache_limits`、`clear_cache` 配置、查询和清除缓存。降低上限会立即从最旧条目开始淘汰；任一上限为0时禁用缓存。

完整JSON键包含类型、版本、executor标识、输入和properties。标准HashMap在哈希后比较完整键，避免碰撞导致错误命中。查找对条目数的平均复杂度为O(1)，序列化和哈希仍取决于键长度。命中不改变FIFO顺序；重复键只占一个条目，但不会合并并发执行。

容量统计为UTF-8键与输出JSON的总字节数。键序列化达到上限即停止，输出计数不生成额外JSON缓冲区。超大条目不保存，不淘汰现有条目，也不导致执行失败。统计不含分配器、容器开销、执行中的数据、Report、executor内部或主机CPU/GPU资源，因此不是进程内存上限。Resource必须使用不可变的内容/版本ID，实际数据与生命周期由主机管理。

缓存保留executor的Arc，防止地址复用。不同executor即使类型、版本相同也不共享缓存；同一Arc且约定相同时可跨Registry共享。行为或模式变化时必须增加版本。pure节点不得产生副作用，结果只能依赖声明的类型/版本、输入与properties，不得依赖节点ID、时间或可变外部状态。失败、无效输出和取消结果不缓存，已取消的run不使用预热缓存。Report、TypeScript与ACX的通信格式保持一致；ACX采用新的默认容量上限。Rust进度通知与主机提供的执行期限见[执行进度](EXECUTION_PROGRESS.md#简体中文)。
