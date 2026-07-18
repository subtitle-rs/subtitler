# AGENTS.md

> 本文件是给开发者/AI 协作者的**工作手册**，固化了本仓库的开发规范、踩过的坑、以及发布流程。
> 改动前先读完，避免重复踩坑。

## 0. 项目快速画像

- **单 crate** 在 `subtitler/`（仓库根只是 git 容器，根无 `Cargo.toml`）。
- **13 种字幕格式**（SRT, VTT, ASS, SSA, MicroDVD, SubViewer, TTML, SBV, LRC, SAMI, MPL2, SCC, EBU STL），每个一个 feature flag。
- 时间戳统一用**毫秒 (`u64`)**，不是秒。
- `src/lib.rs` = 库根，`src/main.rs` = CLI 二进制。
- 当前 MSRV: **1.85**（Edition 2024）。
- 路线图见 `docs/superpowers/specs/2026-07-18-post-2.0-roadmap-design.md`。

---

## 1. 必读文档（按优先级）

| 顺序 | 文档 | 何时读 |
|------|------|--------|
| 1 | **本文（AGENTS.md）** | 每次开始工作前 |
| 2 | `docs/CODE_WIKI.md` | 需要理解架构 / API 全貌时（注意：可能滞后于最新版本，参考源码为准） |
| 3 | `CHANGELOG.md` | 需要知道某版本做了什么 / 写新 changelog 时 |
| 4 | `docs/superpowers/specs/2026-07-18-post-2.0-roadmap-design.md` | 决定下一个版本做什么时 |
| 5 | `MIGRATION.md` | 涉及跨版本 API 变更时 |

---

## 2. 构建、测试、Lint（命令清单）

```bash
# 构建
cargo build --verbose

# 测试（必须用 --all-targets，见下方"踩坑"）
cargo test --all-targets

# 格式检查（注意是 2 空格，见 rustfmt.toml，非 Rust 默认 4 空格）
cargo fmt -- --check

# Lint（必须用 --all-targets，否则漏掉 tests/examples/benches 里的违规）
cargo clippy --all-targets -- -D warnings

# 精简构建（关闭所有非必需格式）
cargo build --no-default-features --features srt

# examples 构建
cargo build --examples

# HTTP example 需显式启用 feature
cargo run --example parse-srt-http --features="http"

# 性能基准（criterion）
cargo bench
```

### CI 矩阵

`.github/workflows/rust.yml` 跑两个 feature 组合：
- `""`（default features，13 格式 + http）
- `--no-default-features --features srt`（最小构建）

**注意**：当前 CI 的 clippy job **只跑 default features**，最小构建的 `#[cfg]` 代码未被 lint。这是已知缺口（路线图 2.3 修）。

---

## 3. 本仓库的踩坑清单（重要！）

### 3.1 `cargo clippy` 必须加 `--all-targets`

**经历**：v2.0.0 发布时，`cargo clippy`（不带参数）通过了，但 `cargo clippy --all-targets` 失败 —— `tests/pipeline_integration.rs` 里有 3 个错误被默认 clippy 漏掉。

**规则**：永远用 `cargo clippy --all-targets -- -D warnings`。任何不带 `--all-targets` 的 clippy 检查都是**假阳性通过**。

### 3.2 改动前必须亲自编译验证，不要轻信 subagent 报告

**经历**：审查阶段 subagent 报告"`src/subviewer.rs:269` 有 needless borrow（和 lrc.rs 同款）"，写进计划。实施时改完编译报 `E0308: expected &[Subtitle], found Vec<Subtitle>` —— subviewer 的 `subs` 是 `Vec`，`&subs` 是必要的；lrc 的 `subs` 来自 `file.subtitles()` 返回 `&[Subtitle]`，所以 `&` 才是多余的。**同一段模式，不同上下文，结论相反**。

**规则**：subagent 报告的 bug、计划里的代码改动，在 commit 前必须实际跑编译/测试验证。每次改动后立刻 `cargo build` 或 `cargo test` 看是否真的过。失败立刻 revert，不要硬改。

### 3.3 "看起来重复"的东西可能是互补的，删前先看清内容

**经历**：CHANGELOG 里有两个 `## [1.4.0]` 头（一个 `-2026-07-17`，一个 `-2026-07-15`），原计划是删后者。实际读了内容才发现：07-17 那份是 New Format Support，07-15 那份是 Performance/Fixed/Added/Changed —— 是同一版本的两半被误写成两个 header。正确做法是**合并**而非删除。

**规则**：删除任何看起来重复的内容前，先完整读它，确认是不是真的冗余。

### 3.4 `Cargo.lock` 在 version bump 后会变，要单独 commit

**经历**：`Cargo.toml` 改完 `version = "2.0.1"` 后，`Cargo.lock` 里 `[[package]] name = "subtitler"` 的 version 也会跟着变，但不会被 `cargo build` 自动 stage。最终验证时 `git status` 才显示 `M Cargo.lock`。

**规则**：version bump 后检查 `git status`，把 `Cargo.lock` 的改动一起 commit。

### 3.5 时间戳是毫秒，不是秒；帧格式用 `ms_to_frames` / `frames_to_ms`

全库统一 `u64` 毫秒。MicroDVD/MPL2/SCC/EBU STL 是帧格式，转 ms 用 `model::convert::{ms_to_frames, frames_to_ms}`。

### 3.6 `generate()` 默认覆写，不是追加

所有 `generate()` 用 `OpenOptions::write(true).truncate(true)`。要拒绝覆写用 `WritePolicy::RefuseIfExists`，要追加用 `WritePolicy::Append`。

### 3.7 `parse_url` 需要 `http` feature

`parse_url` / `parse_url_with` 只在 `cfg(feature = "http")` 下编译。HTTP example 必须显式 `--features="http"`。

### 3.8 TTML 的 `write_stream` 用同步 `std::io::Write`，其余格式用 async

TTML 因为 `quick-xml` 限制，`write_stream<W: std::io::Write>` 是同步的。其余格式 `write_stream<W: tokio::io::AsyncWrite + Unpin>` 是异步的。**不能把 TTML 的 write_stream 塞进 async 管线**。（路线图 2.2 修）

### 3.9 EBU STL 是二进制格式，不要先跑文本解码

`src/main.rs:96-126` 的 `parse_to_file` 和 `cmd_parse` 当前**对所有输入先跑 `encoding::decode_to_string`**，对二进制 STL 文件会误报 `InvalidEncoding`。这是 P1 bug（路线图 2.1 修）。在此期间处理 STL 要注意。

### 3.10 `tests/proptest.proptest-regressions` 是本地文件，不该提交

proptest 的失败重放文件是开发本地产物。`.gitignore` 里要加这一条（路线图 2.3 修）。当前已误提交，不要 `git add` 它的新改动。

---

## 4. Feature Flags

`default = ["srt", "vtt", "ass", "ssa", "microdvd", "subviewer", "ttml", "sbv", "lrc", "sami", "mpl2", "scc", "ebu_stl", "http"]`

每个格式一个 feature，通过 `#[cfg(feature = "xxx")]` 控制模块声明、`Format`/`SubtitleFile` 枚举变体、所有 `match` 分支。

精简依赖：
```toml
[dependencies]
subtitler = { version = "2.0", default-features = false, features = ["srt", "vtt"] }
```

---

## 5. CLI

```
subtitler parse <input>                              # 解析并展示（input = path / URL / `-` for stdin）
subtitler convert <input> <output>                   # 格式转换
subtitler validate <input> [--max-cps N --max-chars N --max-gap MS]
subtitler edit <input> --output <out>                # sort / shift / merge / split / transform-fps
subtitler shift <input> <ms> --output <out>          # 正数=延迟，负数=提前
subtitler normalize <input> --output <out>           # --all 或 --fix-ocr / --strip-hi / --quotes / --whitespace
subtitler quality <input> [--json]
subtitler info <input>
subtitler detect <input>
subtitler pipeline <input> <output> --config ops.json  # 声明式流水线（v2.0+）
```

格式自动检测：**内容签名优先**，扩展名/URL 作 hint。

---

## 6. 开发流程（每个改动都要走）

### 6.1 分支策略

- **小改动 / hotfix**（patch 级，无 API 变更）：直接在 `main` 上做，但**需要用户显式授权**（"在 main 上做 OK 吗？"）。
- **正常 feature / breaking change**：新建 `release/x.y.x` 或 `feature/<name>` 分支，验证通过后由用户 merge。
- **绝不**在没有用户授权的情况下 push 到 `main`。

### 6.2 TDD / 验证门禁

每个改动后跑这套（缺一不可）：

```bash
cargo fmt -- --check                              # 0 diff
cargo clippy --all-targets -- -D warnings         # 0 警告（注意 --all-targets！）
cargo test --all-targets                          # 全过，且测试数不减少
cargo build --no-default-features --features srt  # 最小构建仍工作
cargo build --examples                            # 示例仍工作
```

### 6.3 每个 bug 修复配套 regression 测试

不"先修后补测"。修 bug 的同一个 commit / PR 里必须有能复现该 bug 的测试（修复前 fail，修复后 pass）。

### 6.4 Commit 粒度

- 一件事一个 commit（不要把多个无关改动塞一个 commit）。
- 失败的尝试要立刻 `git revert`，不要在错的改动上硬修。
- Commit message 用 Conventional Commits：`feat:` / `fix:` / `docs:` / `chore:` / `test:` / `refactor:`。
- hotfix / minor 改动也要写 CHANGELOG 条目。

### 6.5 CHANGELOG 规则

- **顶部永远是 `## [Unreleased]`**（Keep a Changelog 约定），下面是正在开发但未发布的改动。
- 发布时把 `[Unreleased]` 改成 `## [x.y.z] - YYYY-MM-DD`，并在顶部新建空的 `[Unreleased]`。
- **新版本条目位置**：紧跟 `[Unreleased]` 之后，**按发布时间倒序**（newest-first）。
- **绝不**用 `## [Unreleased] — vX.Y.Z` 这种把版本号塞进 Unreleased 头的写法（v2.0.0 发布时就犯了这错，导致 crates.io 把 1.4.0 当成最新 release notes）。
- 删除看似重复的版本头前，**先读内容**确认是不是真的重复（参考 §3.3）。

---

## 7. 发布流程（SemVer）

### 7.1 版本号策略

- `patch`（x.y.Z）：bug 修复、文档、零行为变更。例：2.0.0 → 2.0.1。
- `minor`（x.Y.0）：新增 API、minor breaking change（配 MIGRATION 说明）。例：2.1.0。
- `major`（X.0.0）：重大架构 / 大量新 API 面。例：3.0.0。

### 7.2 发布前检查

1. **所有改动已 commit 且 working tree 干净**：`git status` 空。
2. **本地全部门禁通过**：见 §6.2 全套命令。
3. **`Cargo.toml` 的 `version` 已 bump 到目标版本**。
4. **`Cargo.lock` 的 version 同步更新**（参考 §3.4，单独 commit）。
5. **CHANGELOG 顶部已有对应 `## [x.y.z] - YYYY-MM-DD` 条目**。
6. **`cargo publish --dry-run` 通过**（见 §7.4 网络注意）。

### 7.3 发布步骤（顺序很重要）

```bash
# 1. 打 annotated tag（不要用 lightweight tag，annotated 才有 release notes）
git tag -a vX.Y.Z -m "vX.Y.Z: <一句话说明>

<详细 release notes，可从 CHANGELOG 复制>"

# 2. push tag（触发 .github/workflows/release.yml，cargo-dist 构建 GitHub Release artifact）
git push origin vX.Y.Z

# 3. 发布到 crates.io（注意网络！见 §7.4）
cargo publish --registry crates-io

# 4. 验证（见 §7.5）
```

### 7.4 ⚠️ crates.io 网络注意（国内开发环境）

**问题**：本机 cargo 默认配置 `rsproxy-sparse` 镜像替代 crates.io。`cargo publish` 不带参数会报：
```
error: crates-io is replaced with non-remote-registry source registry `rsproxy-sparse`;
include `--registry crates-io` to use crates.io
```

**解法**：
```bash
cargo publish --registry crates-io           # 必须显式指定 registry
```

**进一步问题**：crates.io 主域名国内可能超时（30s timeout 失败）。**解法**：
```bash
# 加长超时
CARGO_HTTP_TIMEOUT=180 cargo publish --registry crates-io

# 如果还超时，走代理（按你本地代理端口替换）
HTTPS_PROXY=http://127.0.0.1:7890 HTTP_PROXY=http://127.0.0.1:7890 \
  cargo publish --registry crates-io
```

**重试安全**：crates.io publish 是事务性的 —— 失败时 crate 不会半成品上架，**重试不会冲突**，可以反复试。

### 7.5 验证发布成功

```bash
# crates.io：查 API（如果网络通）
curl -s "https://crates.io/api/v1/crates/subtitler/X.Y.Z" | grep -E '"(version|published_at)"'

# GitHub Release：查 API（公开端点，无需 gh auth）
curl -s "https://api.github.com/repos/subtitle-rs/subtitler/releases/tags/vX.Y.Z" \
  | grep -E '"(tag_name|html_url|published_at|draft)"'

# 或者直接拉取测试
cargo add subtitler@X.Y.Z --dry-run   # 看版本能否解析
```

**注意**：本机 `gh` CLI 未认证，`gh run list` 不能用。只能用 GitHub 公开 REST API。

### 7.6 两条独立的发布渠道

| 渠道 | 触发方式 | 产物 |
|------|---------|------|
| **crates.io** | 手动 `cargo publish` | Rust 开发者 `cargo add` 用 |
| **GitHub Release** | push tag 自动触发 `release.yml` | cargo-dist 构建的 `.tar.gz` 二进制 artifact，给 CLI 用户下载 |

两条互相独立。crates.io 失败不影响 GitHub Release，反之亦然。**两边都要发才算完整发布**。

---

## 8. 当前已知问题（2026-07-18 快照）

按路线图 (`docs/superpowers/specs/2026-07-18-post-2.0-roadmap-design.md`) 排期修复：

- **2.1（正确性）**：EBU STL round-trip 损坏、SCC drop-frame 错算、encoding UTF-16 不剥 BOM、parse_to_file 对二进制误解码、main.rs unwrap 脆弱、split_long 产零时长。
- **2.2（API）**：13 格式公共 API 不统一（generate/parse_stream/write_stream/返回类型）。
- **2.3（测试/CI）**：WASM 零测试、cross-format 矩阵 ~3%、CI 不测 WASM/MSRV、CODE_WIKI 过时。
- **dependabot**：GitHub 提示仓库有 4 个依赖漏洞（3 moderate + 1 low），2.1 顺手 `cargo update`。

---

## 9. 常用链接

- 仓库: https://github.com/subtitle-rs/subtitler
- crates.io: https://crates.io/crates/subtitler
- docs.rs: https://docs.rs/subtitler
- 路线图: `docs/superpowers/specs/2026-07-18-post-2.0-roadmap-design.md`
- 当前版本: `2.0.1`（见 `Cargo.toml`）
