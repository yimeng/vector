# Vector 本地编译优化指南（资源受限环境）

本文档针对在 **磁盘紧张、内存有限** 的环境下编译 Vector（包含 `greptimedb_wide_metrics` 组件）release 版本的场景，提供一套完整的编译优化方案。

---

## 一、最小 Feature 集合

Vector 默认启用大量 features，在本地编译时建议关闭默认功能，仅启用必需的组件，以显著降低编译时间、内存占用和磁盘占用。

**推荐的 feature 列表：**

```text
api,unix,vrl/stdlib,sinks-greptimedb_wide_metrics,sinks-greptimedb_metrics,sinks-greptimedb_logs,sources-opentelemetry,transforms-remap,sinks-blackhole,sinks-console
```

**说明：**
- `--no-default-features`：关闭默认特性，避免编译大量不需要的 sinks/sources。
- 保留 `api`、`unix` 等核心特性。
- 按需加入 `greptimedb` 相关的 sink 特性（wide_metrics / metrics / logs）。
- `sources-opentelemetry` + `transforms-remap` + `sinks-blackhole` / `sinks-console` 覆盖基本的数据流测试需求。

---

## 二、环境变量配置

在编译前建议设置以下环境变量，写入 `~/.bashrc` 或当前 shell session：

```bash
# 自定义编译产物目录，避免撑爆系统盘
export CARGO_TARGET_DIR=/data/vector-target

# 禁用增量编译，减少内存和磁盘开销
export CARGO_INCREMENTAL=0

# 优化链接速度，减少编译耗时
export RUSTFLAGS="-C link-arg=-fuse-ld=lld -C lto=thin"
```

**说明：**
- `CARGO_TARGET_DIR` 将 target 目录迁移到 `/data` 下的大容量磁盘。
- `CARGO_INCREMENTAL=0` 在 release 编译时尤为重要，可避免增量编译带来的额外内存峰值。
- `RUSTFLAGS` 中：
  - `-fuse-ld=lld` 使用 LLD 链接器，速度显著快于默认 ld。
  - `lto=thin` 在启用 LTO 的前提下，比 full LTO 更省内存。

> **注意**：如果系统未安装 `lld`，需先安装：`sudo apt install lld`（Debian/Ubuntu）或 `sudo yum install lld`（CentOS/RHEL）。

---

## 三、并发控制（-j 参数）

Rust 编译并行单元多时内存消耗会线性增长，建议根据可用内存限制并发任务数：

| 可用内存 | 建议 -j 参数 | 说明 |
|---------|-------------|------|
| < 8 GB  | `-j 2`      | 保守策略，降低 OOM 风险 |
| 8–16 GB | `-j 3`      | 平衡编译速度与稳定性 |
| 16–32 GB| `-j 4`      | 较快但仍留有余量 |
| ≥ 32 GB | 不指定或 `-j $(nproc)` | 资源充足，可全力编译 |

> 若多次出现编译进程被系统 kill（dmesg 中出现 `Out of memory: Kill process`），应立即降低 `-j` 值。

---

## 四、Swap 配置建议

如果物理内存 **小于 16 GB**，强烈建议配置足够的 swap 空间作为安全缓冲。

```bash
# 查看当前 swap 大小
free -h
swapon --show

# 快速创建 16GB swap 文件（示例）
sudo fallocate -l 16G /swapfile-vector
sudo chmod 600 /swapfile-vector
sudo mkswap /swapfile-vector
sudo swapon /swapfile-vector

# 验证
free -h
```

**建议的 swap 大小：**

| 物理内存 | 建议 swap 大小 |
|---------|---------------|
| < 8 GB  | ≥ 16 GB       |
| 8–16 GB | ≥ 8 GB        |
| ≥ 16 GB | 4–8 GB（可选） |

> 注意：swap 不能完全替代物理内存，频繁换页会显著降低编译速度，但能在峰值内存时避免 OOM。

---

## 五、使用 sccache / ccache 加速

在磁盘空间允许的前提下，启用 `sccache` 可以缓存编译中间产物，极大缩短重复编译时间。

### 安装与配置 sccache

```bash
# 安装 sccache
cargo install sccache --locked

# 设置缓存目录（建议放在大容量磁盘）
export SCCACHE_DIR=/data/sccache
export SCCACHE_CACHE_SIZE="50G"

# 启用 sccache 作为 rustc 封装器
export RUSTC_WRAPPER=sccache
```

**验证 sccache 是否生效：**

```bash
sccache -s
```

> 如果磁盘极其紧张（< 50G 剩余），可减小 `SCCACHE_CACHE_SIZE`，或改用 `ccache`（对 C/C++ 依赖有效）。

### ccache（C 依赖编译缓存）

Vector 的部分依赖（如 OpenSSL、zlib-ng）涉及 C/C++ 编译，可额外配置 `ccache`：

```bash
sudo apt install ccache  # Debian/Ubuntu
export CC="ccache gcc"
export CXX="ccache g++"
```

---

## 六、编译命令示例

### 6.1 完整编译命令

```bash
# 1. 设置环境变量
export CARGO_TARGET_DIR=/data/vector-target
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-C link-arg=-fuse-ld=lld -C lto=thin"
export RUSTC_WRAPPER=sccache
export SCCACHE_DIR=/data/sccache
export SCCACHE_CACHE_SIZE="50G"

# 2. 定义 feature 列表
FEATURES="api,unix,vrl/stdlib,sinks-greptimedb_wide_metrics,sinks-greptimedb_metrics,sinks-greptimedb_logs,sources-opentelemetry,transforms-remap,sinks-blackhole,sinks-console"

# 3. 执行编译（以 -j 3 为例）
cargo build --release --no-default-features --features "$FEATURES" -j 3
```

### 6.2 分步编译（进一步降低峰值内存）

如果直接编译 release 仍然 OOM，可以先编译依赖库，再编译主程序：

```bash
# 先仅编译 dependencies
FEATURES="api,unix,vrl/stdlib,sinks-greptimedb_wide_metrics,sinks-greptimedb_metrics,sinks-greptimedb_logs,sources-opentelemetry,transforms-remap,sinks-blackhole,sinks-console"

# 第一步：只检查/编译依赖（不编译 vector 本身）
cargo check --release --no-default-features --features "$FEATURES" -j 3

# 第二步：正式编译 release
cargo build --release --no-default-features --features "$FEATURES" -j 3
```

---

## 七、strip 与压缩建议

Release 编译出的二进制文件体积巨大，建议在部署前进行 strip 和压缩。

### 7.1 Strip 符号表

```bash
# 获取生成的二进制路径
BINARY="$CARGO_TARGET_DIR/release/vector"

# 保留最小符号（可选）
strip --strip-debug "$BINARY"

# 或完全去除符号（体积最小）
# strip "$BINARY"
```

### 7.2 UPX 压缩（可选）

如果目标环境允许运行时解压开销，可使用 UPX 进一步减小体积：

```bash
# 安装 UPX
sudo apt install upx  # Debian/Ubuntu

# 压缩
upx --best "$BINARY"
```

> ⚠️ UPX 压缩后的二进制启动时会有轻微延迟，且某些安全扫描工具可能报毒，生产环境请谨慎使用。

### 7.3 体积对比参考

| 处理方式 | 典型体积（Vector） |
|---------|------------------|
| 原始 release | ~150–250 MB |
| `strip --strip-debug` | ~120–180 MB |
| `strip` + `upx --best` | ~40–60 MB |

---

## 八、编译 OOM 的应对措施

如果编译过程中出现 OOM（进程被系统杀死），按以下优先级排查和解决：

### 8.1 立即降速
- 降低 `-j` 参数，从 `-j 4` → `-j 3` → `-j 2` → `-j 1`。
- 关闭 `sccache`（`unset RUSTC_WRAPPER`），观察是否因缓存进程额外占用内存导致。

### 8.2 增加虚拟内存
- 按第四节配置或扩容 swap。
- 若使用 tmpfs（如 `/tmp` 挂载在内存中），确保 `CARGO_TARGET_DIR` 指向物理磁盘，而非 tmpfs。

### 8.3 精简 feature 集合
- 暂时去掉非核心特性（如 `sinks-console`、`sinks-blackhole`），先保证主程序能编译通过。

### 8.4 禁用 LTO
- 将 `RUSTFLAGS` 中的 `lto=thin` 去掉，甚至可显式设置 `-C lto=off`：
  ```bash
  export RUSTFLAGS="-C link-arg=-fuse-ld=lld -C lto=off"
  ```
  这会增加二进制体积，但能显著降低链接阶段内存峰值。

### 8.5 使用 cargo 的 `-Z threads=N`（Nightly）
- 如果使用 Nightly Rust，可尝试：
  ```bash
  export CARGO_BUILD_JOBS=2
  ```
  或使用 `cargo build --config 'build.jobs=2'`。

### 8.6 监控内存使用

```bash
# 实时监控编译进程内存
watch -n 2 'ps aux | grep rustc | awk "{print \$2, \$4, \$11}"'

# 或查看系统日志确认 OOM
sudo dmesg | tail -n 20 | grep -i "killed process\|out of memory"
```

---

## 九、快速检查清单

在每次编译前，确认以下配置：

- [ ] `--no-default-features` 已启用
- [ ] feature 列表已精简到最小必需集合
- [ ] `CARGO_TARGET_DIR=/data/vector-target` 已设置
- [ ] `CARGO_INCREMENTAL=0` 已设置
- [ ] `RUSTFLAGS` 包含 `-fuse-ld=lld`
- [ ] `-j` 参数根据内存大小设置（建议 2–4）
- [ ] 内存 < 16G 时 swap 已配置
- [ ] `sccache` 已启用且缓存目录在剩余空间充足的磁盘上
- [ ] 编译完成后对二进制进行了 `strip`

---

## 十、参考链接

- [Vector Contributing Guide](CONTRIBUTING.md)
- [Cargo 官方文档 — Build Configuration](https://doc.rust-lang.org/cargo/reference/config.html)
- [sccache 文档](https://github.com/mozilla/sccache)
- [Rust Linker 优化指南](https://rustc-dev-guide.rust-lang.org/backend/libs-and-metadata.html)
