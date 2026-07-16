# subtitler 1.0 迭代优化方案

> 作者：代码审查助手
> 日期：2026-07-16
> 对应：[code-review-2026-07-16.md](./code-review-2026-07-16.md)
> 目标：在 [code-review-2026-07-16.md](./code-review-2026-07-16.md) 第 13 轮修复之后，把剩余 12 项中低优先级建议按"独立成 commit / 单独可发 / 从无破坏到高破坏"分轮消化，**最终发布 1.0.0**。

---

## 0. 总览

| Iter | 报告条目 | 内容 | 风险 | API 破坏 | 估计工时 | 单独可发 |
|---|---|---|---|---|---|---|
| 1 | §9.1 / §9.4 / §9.2 | Cargo 依赖清理 | 🟢 低 | 否 | 2-3h | ✅ |
| 2 | §3.1 | encoding 真解码 | 🟢 低 | 否 | 4-6h | ✅ |
| 3 | §8 / §13.6 | 文档补齐 | 🟢 低 | 否 | 3-4h | ✅ |
| 4 | §5.3 | Generate WritePolicy | 🟡 中 | 加 API | 2-3h | ✅ |
| 5 | §7.2.2 | Proptest 骨架 | 🟡 中 | 否 | 4-6h | ✅ |
| 6 | §2.10 | LRC round-trip 模型重构 | 🔴 高 | **是** | 6-8h | ⚠️ 需 1.0 前 |
| 7 | §5.1 | API 统一 | 🔴 高 | **是** | 4-6h | ⚠️ 需 1.0 前 |
| 8 | §1.2.1 | Subtitle 字段瘦身 | 🔴 高 | **是** | 6-8h | 可延后到 1.1 |
| 9 | §4.4 | Streaming parser | 🟡 中 | 加 API | 8-12h | ✅ |
| 10 | — | 发版准备 | 🟢 低 | 否 | 1-2h | — |

- **总工时**：~40-50h
- **建议路径**：1 → 2 → 3 → 4 → 5 → 6 → 7 → 9 → 10，然后 1.0；§8（Iter 8 字段瘦身）可推到 1.1
- **Cargo.toml 起点建议**：version 从 `1.0.0` 回退到 `0.10.0`（标签已存在但尚未发布），全部跑完后升 `1.0.0`

---

## 1. 关键决策（开始前需拍板）

| # | 决策项 | 建议 | 备选 |
|---|---|---|---|
| 1 | Iter 6/7/8 是否真做 | 6 + 7 做，8 留 1.1 | 全做 / 全留 1.1 |
| 2 | Iter 9 是否做 | 时间紧可放 1.1 | 1.0 一起发 |
| 3 | 是否从 0.10.0 起步 | **强烈建议** | 直接 1.0.0 起步 |
| 4 | encoding_rs 是否引入 | 引入 + GBK/SJIS 映射表 | 保持现状（假装支持） |
| 5 | Iter 4 WritePolicy 形态 | enum（`Overwrite` / `RefuseIfExists` / `Append`） | 仅布尔 `overwrite: bool` |

---

## 2. 每轮详情

### Iter 1 — Cargo 依赖清理

**报告条目**：§9.1 / §9.4 / §9.2
**风险**：🟢 低 — 仅改 `[dependencies]` 元数据，**零行为变化**
**API 破坏**：否

**目标**：减依赖体积、避免 OpenSSL 链接、统一 tracing 角色。

**Cargo.toml 修改**：

```toml
# 1) reqwest 禁 default features + 改用 rustls
[dependencies]
reqwest = { version = "0.13", default-features = false, features = ["rustls-tls"], optional = true }

# 2) tokio 减 feature
tokio = { version = "1", features = ["fs", "io-util", "rt", "macros"] }
# 原: ["fs", "io-util", "rt-multi-thread", "macros"]
# 理由：#[tokio::main(flavor="current_thread")] 不需要 thread pool

# 3) tracing 处置（二选一）
#   方案 A: 移除 tracing 依赖（库内 0 处使用）
#   方案 B: 在 lib.rs 给 parse_* 加 #[instrument]
#   推荐方案 A
```

**验证**：
```bash
cargo build --all-features
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
# 三平台 CI 跑一遍（macOS/Windows/Linux）
```

**回滚策略**：`git revert` 即可。

---

### Iter 2 — encoding 真解码

**报告条目**：§3.1
**风险**：🟢 低 — 加依赖 + 加分支，零破坏
**API 破坏**：否

**目标**：让 ASS/SSA 接收 GBK/Shift_JIS/Big5 等真实编码文件能成功解析。

**Cargo.toml 修改**：

```toml
[dependencies]
encoding_rs = "0.8"
```

**src/encoding.rs 关键修改**：

```rust
use encoding_rs::Encoding;

pub fn decode_to_string(data: &[u8]) -> AnyResult<String> {
    let encoding_name = detect_encoding(data);
    match encoding_name {
        "UTF-8" | "UTF-8-BOM" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| anyhow!("Invalid UTF-8: {}", e))?
                .trim_start_matches('\u{FEFF}');
            Ok(s.to_string())
        }
        "UTF-16BE" | "UTF-16LE" => {
            // 现有实现保留
        }
        other => {
            // 真解码：走 encoding_rs
            let enc = Encoding::for_label_no_replacement(other.as_bytes())
                .ok_or_else(|| anyhow!("Unsupported encoding: {}", other))?;
            let (cow, _, had_errors) = enc.decode(data);
            if had_errors {
                eprintln!("warning: {} decoding had errors", other);
            }
            Ok(cow.into_owned())
        }
    }
}
```

**新增 fixture 测试** `tests/encoding_test.rs`：

```rust
#[test]
fn ass_gbk_decodes() {
    // fixture 准备：
    //   iconv -f UTF-8 -t GBK examples/utf8.ass > examples/gbk.ass
    let bytes = include_bytes!("../examples/gbk.ass");
    let result = subtitler::ass::parse_bytes(bytes);
    assert!(result.is_ok(), "GBK ASS should parse");
    assert!(!result.unwrap().subtitles().is_empty());
}
```

**注意事项**：
- chardetng 报告名可能是 `"Shift_JIS"` / `"GBK"` / `"Big5"` / `"EUC-KR"` / `"windows-1252"` 等；`encoding_rs::Encoding::for_label_no_replacement` 接受 IANA/MIME 名，需要实测全命中。未识别名 fall back 到 `windows-1252`。
- `examples/gbk.ass` 文件需 commit 进仓库。
- 现有 `examples/` 已有 UTF-8 副本，需补 GBK 版本做 cross-encoding 验证。

**回滚策略**：`git revert` + 保留 fixture 文件（无其他依赖）。

---

### Iter 3 — 文档补齐

**报告条目**：§8 / §13.6
**风险**：🟢 低 — 仅 Markdown
**API 破坏**：否

**目标**：docs.rs 渲染完整、推广文档不与代码矛盾。

**README.md 三处补全**：

1. **TTML / SBV / LRC 三个 format 的最小示例**（仿 SRT/VTT/ASS 段）
2. **editing API 总表**：
   - `shift` / `merge` / `split` / `validate` / `quality` / `normalize` / `framerate` / `extract_range` / `concatenate`
3. **QualityReport 字段说明**

**CHANGELOG.md**：补 1.0.0 段，列出本 PR 系列：
- `4eda5e3`（第 1 轮修复）
- `f7b5802`（第 2 轮修复）
- `7c307f2`（第 3 轮修复）
- `d76c631`（第 4 轮修复）
- 本方案 iter 1-9

**docs/promotion/04-english-reddit-hn.md**：
- 删 "zero panic" 措辞
- 改为 "production paths use Result-based error handling; remaining `expect` calls document their invariants in-place"

**验证**：
```bash
cargo doc --no-deps --all-features
# 人工 review docs.rs 渲染
```

**回滚策略**：`git revert`。

---

### Iter 4 — Generate WritePolicy

**报告条目**：§5.3
**风险**：🟡 中 — 新增公共 enum 与参数
**API 破坏**：否（仅加 API，参数默认 `None` → `Overwrite` 保持兼容）

**目标**：默认仍 overwrite，但允许拒绝已存在文件 / 追加。

**src/io.rs 新增**：

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WritePolicy {
    /// Overwrite the destination if it exists (current behavior).
    #[default]
    Overwrite,
    /// Refuse to write if the destination exists; return an error.
    RefuseIfExists,
    /// Append to the destination; create if missing. (Future-proofing.)
    Append,
}
```

**各 format 模块的 generate* 接受 `Option<WritePolicy>`**：

```rust
pub fn generate(
    subs: &[Subtitle],
    path: impl AsRef<Path>,
    policy: Option<WritePolicy>,
) -> AnyResult<()> {
    let policy = policy.unwrap_or_default();
    if matches!(policy, WritePolicy::RefuseIfExists) && path.as_ref().exists() {
        return Err(anyhow!(
            "Refusing to overwrite existing file: {}",
            path.as_ref().display()
        ));
    }
    // ... 现有 OpenOptions 逻辑
}
```

**新增测试**：

```rust
#[test]
fn generate_refuses_existing() {
    let path = temp_path();
    generate(&[], &path, None).unwrap();
    let result = generate(&[], &path, Some(WritePolicy::RefuseIfExists));
    assert!(result.is_err());
}
```

**回滚策略**：`git revert`（参数默认 `None` → 旧行为，向后兼容）。

---

### Iter 5 — Proptest 骨架

**报告条目**：§7.2.2
**风险**：🟡 中 — 加 dev-dependency
**API 破坏**：否

**目标**：至少 SRT / VTT / ASS / parse_timestamp 关键路径有 property-based 测试。

**Cargo.toml 修改**：

```toml
[dev-dependencies]
proptest = "1.5"
```

**tests/proptest_srt.rs**：

```rust
use proptest::prelude::*;
use subtitler::{srt, Subtitle};

fn arb_subtitle() -> impl Strategy<Value = Subtitle> {
    (
        0u64..3_600_000,        // start
        0u64..3_600_000,        // end
        "[a-zA-Z0-9 ,.!?]{0,200}", // text
    ).prop_map(|(start, end, text)| {
        let (s, e) = if start <= end { (start, end) } else { (end, start) };
        Subtitle::new(s, e, &text)
    })
}

proptest! {
    #[test]
    fn srt_round_trip_preserves_text_and_times(sub in arb_subtitle()) {
        let s = srt::to_string(&[sub.clone()]);
        let parsed = srt::parse_content(&s).unwrap();
        prop_assert_eq!(parsed.len(), 1);
        prop_assert_eq!(parsed[0].start, sub.start);
        prop_assert_eq!(parsed[0].end, sub.end);
        prop_assert_eq!(parsed[0].text, sub.text);
    }
}
```

**仿照写**：`tests/proptest_vtt.rs`、`tests/proptest_ass.rs`、`tests/proptest_timestamp.rs`

**验证**：
```bash
PROPTEST_CASES=10000 cargo test --all-features proptest
```

**回滚策略**：`git revert`（仅 dev-dep）。

---

### Iter 6 — LRC round-trip 模型重构

**报告条目**：§2.10
**风险**：🔴 高 — 引入新公共类型
**API 破坏**：是（lrc 解析的推荐入口变化）

**目标**：让多时间戳行 LRC `[00:10.00][00:30.00]text` round-trip 不可逆问题解决。

**src/lrc.rs 关键修改**：

```rust
/// Multi-timestamp LRC data: each lyric line may have multiple start times.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LrcData {
    /// One entry per lyric line; each line has 1+ timestamps and one text.
    pub lines: Vec<LrcLine>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LrcLine {
    /// All timestamps (ms) at which this lyric line is sung.
    pub times_ms: Vec<u64>,
    pub text: String,
}

impl LrcData {
    pub fn parse(content: &str) -> AnyResult<Self> { /* ... */ }
    pub fn to_string(&self) -> String { /* ... */ }
}

// 保持 Vec<Subtitle> 解析路径作为 deprecated 入口
#[deprecated(
    since = "0.10.0",
    note = "use LrcData::parse to preserve multi-timestamp lines"
)]
pub fn parse_content(content: &str) -> AnyResult<Vec<Subtitle>> { /* ... */ }
```

**验证**：
- 现有 `lrc_multi_timestamp_round_trip` 测试改为测 `LrcData` round-trip
- 旧 API 仍通过但打 warning

**回滚策略**：复杂。`Vec<Subtitle>` 路径保留，**实际无运行期破坏**，仅类型与 warning。

---

### Iter 7 — API 统一

**报告条目**：§5.1
**风险**：🔴 高
**API 破坏**：是

**目标**：`parse_bytes` / `parse_file` 全部返回 `SubtitleFile`。

**三处修改**：

1. **src/subviewer.rs 改返回 `SubtitleFile::SubViewer`**：

```rust
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> { /* ... */ }
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> { /* ... */ }
pub async fn parse_file(path: ...) -> AnyResult<SubtitleFile> { /* ... */ }
```

2. **src/vtt.rs 合并 `_full` 版本**：
   - `parse_bytes_full` → `parse_bytes_keep_header?`
   - 或：`parse_bytes` 接受 `Option<keep_header>` 参数
   - 推荐：`parse_bytes` 返回 `SubtitleFile::Vtt { header, subtitles }`

3. **src/microdvd.rs 已在 f7b5802 统一**（确认）

**BREAKING 风险**：
- `subviewer::parse_bytes` 返回类型变了
- `vtt::parse_bytes_full` 名字变了
- 所有调用方需要更新

**适配策略**：
- `SubtitleFile::subtitles()` 已存在，向后兼容访问字幕
- 旧 API 用 `#[deprecated]` 包装 1-2 个版本

**验证**：
- 跑全量 tests
- 跑全量 examples（12 个 binary）

**回滚策略**：复杂；建议发 `0.11.0-rc.1` → `0.11.0` → `1.0.0` 路径。

---

### Iter 8 — Subtitle 字段瘦身（可延后 1.1）

**报告条目**：§1.2.1
**风险**：🔴 高
**API 破坏**：是
**建议**：本轮**可选**做方案 A（破坏小），或**完全推到 1.1**。

**目标**：SRT 字幕每个 `Subtitle` 实例省 ~80 字节（9 个 `Option` 字段对 SRT 是 100% 浪费）。

**方案 A（推荐，破坏小）**：

```rust
// AssData 持有 ASS-only 字段
pub struct AssData {
    pub info: HashMap<String, String>,
    pub styles: Vec<AssStyle>,
    pub subtitles: Vec<Subtitle>,
    // layer / margin_v / effect 不再让 Subtitle 兼任
}

// Subtitle 留 8 字段
pub struct Subtitle {
    pub start: u64,
    pub end: u64,
    pub text: String,
    pub index: Option<usize>,
    pub settings: Option<String>,   // VTT
    pub text_parts: Vec<TextPart>,
    pub style: Option<String>,      // ASS
    pub actor: Option<String>,      // ASS
    pub is_comment: bool,
}
```

**方案 B（无破坏）**：

```rust
pub trait SubtitleExt {
    fn layer(&self) -> Option<i32>;
    fn margin_v(&self) -> Option<i32>;
    fn effect(&self) -> Option<&str>;
}
impl SubtitleExt for Subtitle { /* 读内部 Option */ }
```

**回滚策略**：方案 A 的回滚 = 还原字段定义，复杂。

---

### Iter 9 — Streaming parser

**报告条目**：§4.4
**风险**：🟡 中 — 加新公共类型
**API 破坏**：否（仅加法）

**目标**：除 SRT 外其他 format 也提供流式入口。

**src/microdvd.rs 模板**：

```rust
pub struct MicroDvdStream<'a> { /* ... */ }
impl<'a> Iterator for MicroDvdStream<'a> {
    type Item = AnyResult<Subtitle>;
    fn next(&mut self) -> Option<Self::Item> { /* ... */ }
}
pub fn parse_stream(content: &'a str) -> MicroDvdStream<'a> { /* ... */ }
```

**同样加**：
- `src/ass.rs::AssStream`（注意 `[Script Info]` / `[V4+ Styles]` / `[Events]` 段分隔，streaming 时需要 buffer）
- `src/sbv.rs::SbvStream`（简单，按行迭代）
- `src/lrc.rs::LrcStream`（简单，按行迭代）
- `src/subviewer.rs::SubViewerStream`（简单）
- `src/microdvd.rs::MicroDvdStream`（简单）
- `src/vtt.rs::VttStream`（已有 SRT 模式参考）

**验证**：对比 batch 解析结果与 stream 收集结果一致。

---

### Iter 10 — 发版准备

**Cargo.toml 检查**：
- `version = "1.0.0"`（确认）
- `rust-version = "1.85"`（确认）
- 所有 feature flag 互相组合可编译（matrix）

**新增文件**：
- `CHANGELOG.md` 1.0.0 段
- `MIGRATION.md` 从 0.9 → 1.0 的迁移说明（重点是 iter 6/7 的 breaking 改动）

**git 操作**：
- `git tag v1.0.0`

**CI 验证**：
```bash
cargo build --all-features
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo doc --no-deps --all-features
cargo publish --dry-run
```

**release workflow 检查**：
- `.github/workflows/release.yml` — 确认 cargo-dist 或 release-please 配置

---

## 3. 时间线建议

| 周 | 计划 | 备注 |
|---|---|---|
| 第 1 周 | Iter 1 → 2 → 3 | 低风险，3 个 PR，可在 1.0 之前任意点单发 |
| 第 2 周 | Iter 4 → 5 | 中风险，2 个 PR |
| 第 3 周 | Iter 6 → 7 | API 收尾；2 个 PR，**可能要走 `0.11.0-rc` → `0.11.0` 路径** |
| 第 4 周 | Iter 9 → 10 | 流式 + 发版 |
| **Iter 8** | **推到 1.1** | 字段瘦身是非关键路径上的优化 |

---

## 4. 每轮必跑的验证脚本

```bash
# 1. 构建
cargo build --all-features

# 2. 测试
cargo test --all-targets --all-features

# 3. Lint
cargo clippy --all-targets --all-features -- -D warnings

# 4. 格式
cargo fmt --all -- --check

# 5. 文档（Iter 3 / 8 必跑，其他可选）
cargo doc --no-deps --all-features
```

---

## 5. 回滚矩阵

| Iter | 回滚难度 | 回滚策略 | 备注 |
|---|---|---|---|
| 1 | 🟢 极易 | `git revert` | 仅改 `[dependencies]` |
| 2 | 🟢 易 | `git revert` + 保留 fixture | 加新分支 + fixture |
| 3 | 🟢 极易 | `git revert` | 仅 Markdown |
| 4 | 🟢 易 | `git revert` | 参数默认 `None` 兼容 |
| 5 | 🟢 极易 | `git revert` | 仅 dev-dep |
| 6 | 🟡 中 | 旧 API 仍在，仅 deprecation | 实际无运行期破坏 |
| 7 | 🔴 难 | 需发 `0.11.0-rc.1` → `0.11.0` | **API 破坏** |
| 8 | 🔴 难 | 需发 `1.1.0` | **API 破坏** |
| 9 | 🟢 易 | `git revert` | 仅加法 |
| 10 | 🟢 极易 | 不发 tag 即可 | 仅元数据 |

---

## 6. 关键风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| chardetng 与 encoding_rs 编码名不匹配 | 中 | Iter 2 假阳性多 | 写映射表，fall back 到 windows-1252 |
| Iter 6/7 破坏调用方 | 高 | 1.0 延期 | 走 `0.11.0-rc.1` → `0.11.0` 路径 |
| Proptest 发现真实 round-trip bug | 中 | Iter 5 测试失败需修 | 标 `#[ignore]` + 修源码 |
| 字段瘦身引发序列化兼容 | 中 | Iter 8 需要 `#[serde(default)]` | 暂不推 serde 支持 |

---

## 7. 验证完成标准（1.0 之前）

- [ ] Iter 1-9 全部 commit + reviewed
- [ ] `cargo test --all-targets --all-features` 全绿
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 干净
- [ ] `cargo fmt --all -- --check` 干净
- [ ] `cargo doc --no-deps --all-features` 无警告
- [ ] `cargo publish --dry-run` 元数据完整
- [ ] CHANGELOG.md 1.0.0 段就位
- [ ] MIGRATION.md 0.9 → 1.0 段就位
- [ ] `git tag v1.0.0` 准备

---

## 8. 附录：相关文件路径

- 审查报告：[code-review-2026-07-16.md](./code-review-2026-07-16.md)
- AGENTS.md：`/Users/mankong/volumes/code/subtitle-rs/subtitler/AGENTS.md`
- Cargo.toml：`/Users/mankong/volumes/code/subtitle-rs/subtitler/Cargo.toml`
- 推广文档：`/Users/mankong/volumes/code/subtitle-rs/subtitler/docs/promotion/`

---

> 维护者注：每完成一个 iter，请在本文件对应行打勾 + 写 commit SHA；最终 1.0 之前删除 §7 前的"建议"，转成 "Done 状态" 表格。
