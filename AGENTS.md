# AGENTS.md

> 本文件是给开发者/AI 协作者的**工作手册**，固化了本仓库的开发规范、踩过的坑、以及发布流程。
> 改动前先读完，避免重复踩坑。

## 0. 项目快速画像

- **单 crate** 在 `subtitler/`（仓库根只是 git 容器，根无 `Cargo.toml`）。
- **15 种字幕格式**（SRT, VTT, ASS, SSA, MicroDVD, SubViewer, TTML, SBV, LRC, SAMI, MPL2, SCC, EBU STL, DFXP, Whisper JSON），每个一个 feature flag。
- 时间戳统一用**毫秒 (`u64`)**，不是秒。
- `src/lib.rs` = 库根，`src/main.rs` = CLI 二进制。
- 当前 MSRV: **1.85**（Edition 2024）。
- 路线图见 `docs/superpowers/specs/2026-07-18-post-2.0-roadmap-design.md`。

---

## 1. 必读文档（按优先级）

| 顺序 | 文档 | 何时读 |
|------|------|--------|
| 1 | **本文（AGENTS.md）** | 每次开始工作前 |
| 2 | `docs/CODE_WIKI.md` | 需要理解架构 / API 全貌时（每版本同步，见 §6.7） |
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
- `""`（default features，15 格式 + http）
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

### 3.11 不要并行派多个 implementation subagent 改同一 working tree

**经历**：2.1 Task 1-3 尝试并行派 3 个 subagent 同时改 `main.rs`/`model/trait.rs`/`encoding.rs`（"无文件重叠"看似安全）。结果：Task 1 的 subagent 看到另两个 subagent 写了一半的未提交改动，报告"working tree 脏，疑似别人在改"，被迫 stash 验证；Task 2 的 subagent 被 cancel 时留下半截改动污染了基线测试数。

**规则**：subagent-driven 模式下，implementation subagent **必须串行**（一个完成 + commit 后再派下一个）。即使文件不重叠，working tree 是共享资源。superpowers skill 也明确禁止"parallel implementation subagents"，原因就是这个。要做并行只能用 git worktree 隔离。

### 3.12 TDD 数值预期必须独立交叉验证，不能纸面推导

**经历**：2.1 Task 6 SCC drop-frame，spec 里我手算"00:10:00;00 (drop) = 599800ms"，实施时测试失败（实际 600000ms）。Python 模拟才发现手算错了 —— drop-frame 的"整 10 分钟对齐"性质让 17982/29.97 × 1000 正好 600000，不是 ~599800。两次修正预期值才对。

**规则**：spec / plan 里凡有具体数值预期（时间码、帧数、字节偏移、浮点比较），**必须用脚本（Python / Rust 单测 / calculator）独立验证一遍**，不能纸面推导。在 spec 里标注"数值经 Python 模拟验证"，避免实施者怀疑测试代码写错。

### 3.13 clippy 在新代码上的高频错误（提前规避）

实施时常见、容易踩的 clippy lint（写代码时主动规避，省一轮 review）：
- `clippy::redundant_closure`: `Ok(x?)` → 直接 `return x;`（`x` 已是 `Result`，`?` 解包后 `Ok` 重复包装）。
- `clippy::needless_range_loop`: `for i in 0..len { data[i] }` → 用 iterator 或 `data[3..1024].fill(...)`。
- `clippy::len_zero`: `xs.len() >= 1` → `!xs.is_empty()`。
- `clippy::manual_range_contains`: `x >= a && x <= b` → `(a..=b).contains(&x)`。
- `clippy::needless_borrow`: `f(&x)` 当 `x` 已是 `&T` 时 → `f(x)`（参考 §3.2 的 lrc.rs/subviewer.rs 案例）。
- 移动值后又被借用 → 用 `.clone()` 或改借用顺序（如 `cmd_parse` 重构时 format 被 move 后面又要 println）。

### 3.14 路线图分支与发布节奏（一图记）

```
2.0.1 ✅  2.1.0 ✅  2.2.0 ✅  2.3.0 ✅  2.4.0 ✅  2.4.1 ✅  2.5.x ✅  2.6.x ✅
hotfix    正确性    API拉齐   测试/CI   gap收编  外部测试  CI修复    代码质量
```

当前专注打磨 2.x。每次开始工作前看 §8 确定当前版本范围，避免范围蔓延。

### 3.15 新增/删除格式时，必须更新所有文档中的格式列表和计数

**经历**：v2.4 新增 DFXP + Whisper JSON（13→15），commit 了功能代码，但 Cargo.toml `description`、README、SKILL.md、CODE_WIKI、AGENTS.md 五处仍写"13 格式"。

**涉及文件清单**（新增格式时，逐文件检查）：

| 文件 | 要更新的内容 |
|------|------------|
| `Cargo.toml` | `description` 字段的格式列表 + 计数 |
| `README.md` | 第 8 行 "X subtitle formats" 列表 |
| `SKILL.md` | frontmatter `description` + line 8 intro + Supported Formats 表 |
| `docs/CODE_WIKI.md` | §1 格式列表 + 计数 |
| `AGENTS.md`（本文件） | §0 格式列表 + 计数 |

**检查命令**：
```bash
grep -rn "13 格式\|13 formats\|13 subtitle\|13 种" README.md SKILL.md docs/CODE_WIKI.md AGENTS.md Cargo.toml
# 验证最小构建（新增 feature flag 后必须做）
cargo build --no-default-features --features srt
cargo build --no-default-features --features dfxp,ttml
cargo build --no-default-features --features whisper
```

> 反面教材：v2.4.0 crates.io 展示的 description 曾写"13 formats"。

### 3.16 新增格式时，必须验证最小构建 + 最小测试

**经历 v2.4**：新增 DFXP 和 Whisper，但 cli.rs 漏了 `#[cfg(feature)]` gate → `cargo build --no-default-features --features srt` 失败。

**经历 v2.5**：`cargo build` 通过但 `cargo test --lib` 失败 —— `utils.rs` 里 `test_parse_timestamp_vtt` 用了 `Format::Vtt` 无 `#[cfg(feature = "vtt")]`。build 只编代码不编测试，**所以 build 过了不代表 CI 过**。

**规则**：每次添加 feature flag 或新增格式后，跑：
```bash
cargo build --no-default-features --features srt
cargo test --no-default-features --features srt --lib     # ★ 必须加，build 通过不等于 test 通过
cargo build --no-default-features --features dfxp,ttml
cargo build --no-default-features --features whisper
```

---

## 4. Feature Flags

`default = ["srt", "vtt", "ass", "ssa", "microdvd", "subviewer", "ttml", "sbv", "lrc", "sami", "mpl2", "scc", "ebu_stl", "dfxp", "whisper", "http"]`

每个格式一个 feature，通过 `#[cfg(feature = "xxx")]` 控制模块声明、`Format`/`SubtitleFile` 枚举变体、所有 `match` 分支。

精简依赖：
```toml
[dependencies]
subtitler = { version = "2.4", default-features = false, features = ["srt", "vtt"] }
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

**触发条件要对**：测试输入必须真能触发 bug，不能凭直觉写。例：split_long 的零时长 bug 触发条件是 `num_chunks > duration`（按词拆的 chunks 数），不是 `chars > max_chars`。先用最小输入跑一遍确认 fail，再修。

### 6.4 spec / plan 里的数值预期必须独立验证

写 spec 或 plan 时，凡涉及具体数值（时间码 → ms、帧数、字节偏移、浮点比较），**不能纸面推导**，必须用脚本独立验证：

```bash
# Python 是最快的草稿验证工具
python3 -c "
def to_ms(h, m, s, f, fps, drop):
    nf = round(fps)
    tf = h*3600*nf + m*60*nf + s*nf + f
    if drop:
        tm = h*60 + m
        tf -= 2*(tm - tm//10)
    return round(tf / fps * 1000)
print(to_ms(1, 0, 0, 0, 29.97, True))  # 3600000
"
```

在 spec 里标注"数值经 Python 模拟验证"，避免实施者怀疑测试代码写错。TDD 流程里测试失败时，**先怀疑预期值**，再怀疑实现 —— 两种都查。

### 6.5 Commit 粒度

- 一件事一个 commit（不要把多个无关改动塞一个 commit）。
- 失败的尝试要立刻 `git revert`，不要在错的改动上硬修。
- Commit message 用 Conventional Commits：`feat:` / `fix:` / `docs:` / `chore:` / `test:` / `refactor:`。
- hotfix / minor 改动也要写 CHANGELOG 条目。

### 6.6 CHANGELOG 规则

- **顶部永远是 `## [Unreleased]`**（Keep a Changelog 约定），下面是正在开发但未发布的改动。
- 发布时把 `[Unreleased]` 改成 `## [x.y.z] - YYYY-MM-DD`，并在顶部新建空的 `[Unreleased]`。
- **新版本条目位置**：紧跟 `[Unreleased]` 之后，**按发布时间倒序**（newest-first）。
- **绝不**用 `## [Unreleased] — vX.Y.Z` 这种把版本号塞进 Unreleased 头的写法（v2.0.0 发布时就犯了这错，导致 crates.io 把 1.4.0 当成最新 release notes）。
- 删除看似重复的版本头前，**先读内容**确认是不是真的重复（参考 §3.3）。

### 6.7 文档维护节奏（README / AGENTS / CODE_WIKI / MIGRATION）

四份文档各有职责，更新时机不同：

| 文档 | 职责 | 何时更新 |
|------|------|---------|
| `README.md` | 用户面向（入门、API 示例、格式表） | **每次公共 API 变更**或格式数变化时 |
| `AGENTS.md` | 开发者/AI 协作者手册（踩坑、流程、runbook） | **踩到新坑 / 新流程经验**时滚动更新 |
| `MIGRATION.md` | 跨版本升级指南 | **有行为变更 / API 变更**的版本（minor/major）发版前 |
| `docs/CODE_WIKI.md` | 架构百科（17 章 API 全貌） | **发版前**强制同步到当前版本（见 §7.2 第 7 项） |

**CODE_WIKI 的"发版前强制更新"清单**：
- 头部版本号（`> 版本: vX.Y.Z`）。
- 测试数（§14.1）—— 跑 `cargo test --all-targets 2>&1 | grep 'test result' | awk '{s+=$4} END {print s}'` 得当前数。
- §16 路线图进度表（标 ✓/⏳）。
- 任何新增模块（src/ 新文件、tests/ 新文件、新 feature flag）→ §3 目录结构、§5 模块职责、§14 测试体系。
- 任何行为变更（如 SCC drop-frame、UTF-16 BOM 处理）→ §17 设计决策。
- **格式计数**（若有新增/删除格式）→ 同步更新以下文件的格式列表和计数：`Cargo.toml` description、`README.md`、`SKILL.md`、`AGENTS.md` §0。详见 §3.15。

**反面教材**：v2.0.0/v2.0.1/v2.1.0 连发三版期间 CODE_WIKI 一直停在 v1.4.0（216 测试、无 Pipeline/WASM、还把"v2.0 计划做零拷贝"当未来工作）。这种积压让 Wiki 失去参考价值，新人会被误导。**禁止跨版本积压**。

**文档一致性自检**（每次发版前）：
```bash
# 版本号一致
grep '^version' Cargo.toml
grep '^> 版本:' docs/CODE_WIKI.md
grep -m 1 '^## \[' CHANGELOG.md
# 格式计数一致（见 §3.15）
grep -rn "15 格式\|15 formats\|15 subtitle\|15 种" README.md SKILL.md docs/CODE_WIKI.md AGENTS.md Cargo.toml
```

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
6. **MIGRATION.md 已更新**（若有行为变更、API 变更、或 minor/major 版本）。
7. **`docs/CODE_WIKI.md` 已更新到当前版本**（见 §6.7）。**禁止跨版本积压**——2.0/2.0.1/2.1.0 三版没更新 Wiki 是反面教材。
8. **`cargo publish --dry-run` 通过**（见 §7.4 网络注意）。

> **自查命令**（发布前快速核对版本一致性）：
> ```bash
> grep '^version' Cargo.toml                                      # 期望版本号
> grep -n '版本:' docs/CODE_WIKI.md                                # Wiki 头部版本
> grep -nE "^## \[(Unreleased|x\.y\.z)\]" CHANGELOG.md | head -3  # CHANGELOG 顶部
> ```
> 三处的版本号必须一致（或 Wiki 是最新已发布版本）。

### 7.3 发布步骤（顺序很重要）

> **铁律：绝不 force-update / 删除已 push 的 tag。**
> 一旦 tag 推送到 GitHub，即使发现 bug 也不碰它。
> crates.io 不可撤回已发布的版本。
> force-update tag 会让 GitHub tag 与 crates.io 包内容不一致——两个渠道对同一版本号给出不同代码，用户无法信任。
>
> **发现已发布版本有 bug 时**：直接 bump 新 patch 版本（x.y.Z+1），走正常发布流程。旧版本留在历史里，用户自动拿到新版。

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

**注意**：`cargo publish --dry-run`（§7.2 第 6 步）也要带 `--registry crates-io`，否则同样会因 rsproxy-sparse 替换报错。

**进一步问题**：crates.io 主域名国内可能超时（30s timeout 失败）。**解法**：
```bash
# 加长超时
CARGO_HTTP_TIMEOUT=180 cargo publish --registry crates-io

# 如果还超时，走代理（按你本地代理端口替换）
HTTPS_PROXY=http://127.0.0.1:7890 HTTP_PROXY=http://127.0.0.1:7890 \
  cargo publish --registry crates-io
```

**重试安全**：crates.io publish 是事务性的 —— 失败时 crate 不会半成品上架，**重试不会冲突**，可以反复试。

**版本可跳过**：若某版本因网络没发到 crates.io（如 2.0.1），可以直接发下一版（如 2.1.0） —— crates.io 不要求版本连续，只要比当前已发布的最新版本新即可。GitHub Release 那条线不受影响（tag 推送即触发 cargo-dist）。

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

### 7.7 依赖升级（dependabot 漏洞响应）

GitHub Dependabot 会定期报安全漏洞。处理流程：

```bash
# 1. 预览 cargo update 会改什么（不写盘）
cargo update --dry-run

# 2. 应用 SemVer 范围内的升级（安全，不改 Cargo.toml）
cargo update

# 3. 跑全套门禁（§6.2）
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --no-default-features --features srt
cargo build --examples
```

**升级出问题的处理**：
- clippy 新警告：在新版本下修代码，或 `#[allow(clippy::xxx)]` 暂时压制 + comment 说明。
- minimal build 断：某个 crate 的 minor bump 引入了新 `#[cfg]` 路径，定位 + 修。
- 真有 breaking change（跨 major 版本）：在 `Cargo.toml` pin 回原版本，CHANGELOG 注明"X 因 breaking 推迟到下版本"。

**commit 粒度**：依赖升级单独一个 commit，commit message 列出具体升级的 crate + 版本号（参考 commit `b10d118`）。

---

## 8. 路线图进度（2026-07-18 更新）

按 `docs/superpowers/specs/2026-07-18-post-2.0-roadmap-design.md`：

| 版本 | 状态 | 范围 |
|------|------|------|
| **2.0.1** | ✅ | hotfix：clippy/CHANGELOG/README/Cargo.toml 文档对齐 |
| **2.1.0** | ✅ | 6 项 P1 正确性修复 + dependabot 升级。13 新测试，总 286 |
| **2.2.0** | ✅ | API 拉齐：13 格式 generate/parse_stream/write_stream 统一 + TTML write_stream_async + header 保留。7 新测试，总 293 |
| **2.3.0** | ✅ | 测试/CI 夯实：WASM 测试、cross-format 矩阵 23 对、proptest 扩展、chardetng fixture、CI WASM/MSRV/clippy-matrix/bench jobs。32 新测试，总 325 |
| **2.4.0** | ✅ | gap analysis P1 收编：DFXP + Whisper JSON + 去重 PipelineOp + normalize 4 扩展。15 新测试，总 340 |
| **2.4.1** | ✅ | 外部测试修复：SCC 文本解码 P1、DFXP namespace、SubViewer 检测、SBV 两行格式、iTT SMPTE。4 新测试，总 344 |
| **2.6.x** | ✅ | 代码质量优化：error 类型统一 + magic number 常量化 + MSRV let-chain 修复 + CI clippy-matrix 修正。344 测试 |

**当前版本**：`2.6.1`（见 `Cargo.toml`）。**专注打磨 2.x**，暂不规划 3.0。

**历史 commit 可参考**（开发范例）：
- `019a423` AGENTS.md 手册化（文档型 commit 范例）
- `3fe6d1a` SCC drop-frame（复杂算法 commit 范例 + spec 引用 + 不变量说明）
- `6af9918` parse_to_file 重构（含 helper 提取 + 测试设计思考）
- `b10d118` 依赖升级（dependabot 响应范例）
- `62a7760` srt/vtt DRY 重构（共享 helper 提取范例）

---

## 9. 常用链接

- 仓库: https://github.com/subtitle-rs/subtitler
- crates.io: https://crates.io/crates/subtitler
- docs.rs: https://docs.rs/subtitler
- 路线图 spec: `docs/superpowers/specs/2026-07-18-post-2.0-roadmap-design.md`
- 2.1 spec: `docs/superpowers/specs/2026-07-18-2.1-correctness-debt-design.md`
- 2.1 plan: `docs/superpowers/plans/2026-07-18-v2.1-correctness-debt.md`
- 当前版本: `2.4.1`（见 `Cargo.toml`）
