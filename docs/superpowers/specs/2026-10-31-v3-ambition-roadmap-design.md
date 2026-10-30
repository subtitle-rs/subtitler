# Rust 生态最强字幕库路线图（v2.11 → v3.0 Design）

> 状态: Draft（待用户确认 §0.2 决策点）
> 范围: v2.11 → v2.12 → v2.13 → v3.0
> 取向: 先还债、再拉开身位、最后做生态（"最强大"靠护城河，不靠口号）
> 前序 spec: `2026-07-18-post-2.0-roadmap-design.md`（本文件接棒其未完成的 3.0 规划）
> 基线: v2.10.0 · 17 格式 · 492 tests · fmt/clippy/minimal×5/MSRV 全绿

---

## 0. 背景与诊断

### 0.1 现状盘点（2026-10-31）

已具备的（Rust 生态内单项领先）:

| 支柱 | 现状 | Rust 生态内最强对手 |
|------|------|--------------------|
| **格式广度** | 17 格式 + 内容签名自动检测 | `subparse` ↓48k（3 类）；`rsubs-lib` ↓34k（3 类）；`subtp` ↓36k（SRT+VTT） |
| **正确性纪律** | 492 tests、回归测试强制、MSRV 1.85 矩阵 | 无竞品有公开的回归纪律 |
| **专业 QC** | Netflix/BBC/TED/ARD/C4 预设 + min-gap + shot-change 守卫帧 | 无（`caption-convert` ↓75 是玩具） |
| **自动化管线** | 声明式 Pipeline DSL + normalize + dedup | `sub-batch` ↓17k 仅批量改名/换帧率 |
| **WASM** | lib + CI job + 在线 Workbench | `subtile` ↓20k 无 WASM |
| **AI 时代输入** | Whisper JSON + 词级合并 | `franken_whisper` ↓287 是 ASR 引擎不是字幕库 |

关键缺口（"最强大"缺的拼图）:

1. **跨语言触达为零** —— 没有 Python/Node/C 绑定。字幕工具的最大用户群在 Python（数据/AI 管线）和 JS（Web 播放器）。
2. **智能时序为零** —— 自动对轴（alass 式锚点同步）、卡拉OK（`\k`）都没有。`alass` 有 CLI 但没有干净的库 API。
3. **广播深格式为零** —— IMSC 1.1 校验、MCC、CEA-708、EBU-TT 在 Rust 里全是 v0.1 玩具（↓60–2k），是空位不是竞争。
4. **性能没有卖点** —— 分配密集型解析，无 zero-copy，无 SIMD 编码检测，bench 不对外。
5. **已知潜伏债务** —— `--no-default-features --features <beyond-srt> --lib` 在 main 上编译失败（~50 errors，测试缺 cfg gate）；benches 无 feature gate；SSA 无 round-trip；wasm.rs 零测试覆盖；proptest ASCII-only。

### 0.2 待用户决策（写入实施 spec 前确认）

| # | 决策点 | 推荐 | 备选 |
|---|--------|------|------|
| D1 | 版本结构 | **2.11→2.12→2.13→3.0，每版单一主题**（沿用 07-18 spec 的节奏阀门经验） | 直接 3.0 big-bang（review 难、回滚难，不推荐） |
| D2 | 绑定优先级 | **Python (PyO3+maturin) 先**，Node (napi-rs) 随 3.0.x 跟进 | 两个一起做（人力翻倍，收益不翻倍） |
| D3 | 智能对轴边界 | **纯 text+time 锚点同步**（alass 算法路线，零外部依赖）；音频驱动（ffsubsync 式）不做进核心库 | 音频版做成可选 feature（引入音频解码依赖，违背精简原则） |
| D4 | 3.0 破坏性授权 | workspace 拆分 + zero-copy API 重构 = breaking，**授权在 3.0 做**（沿用"破坏只在 major"纪律） | 3.0 保持兼容、推 4.0 |

---

## 1. 总体结构与发布节奏

```
2.11          2.12            2.13            3.0
──────────    ────────────    ────────────    ────────────────────
还债与地基     智能时序         广播深格式        生态与架构
test-gates    anchor-sync     IMSC 校验        workspace 拆分
SSA 回程       karaoke \k      MCC 只读         zero-copy API
unicode 模糊   对轴 CLI         EBU-TT-D 只读    PyO3 / napi / C-FFI
fuzz 起步      时间轴算子补全   CEA-708 (P2)     mdBook 文档站
workbench 同步
~1-2 周       ~2-3 周         ~3-4 周          ~6-8 周
minor         minor           minor            major
```

### 1.1 贯穿原则（沿袭 07-18 spec，全部继续有效）

1. 每个 bug 修复配套 regression 测试；触发条件先用最小输入验证。
2. 每版本单一主题，便于 review / release notes / 回滚。
3. 破坏性变更只出现在 3.0；2.11–2.13 严格向后兼容。
4. 数值预期一律脚本独立验证（§6.4）；review 结论用一手源码/规范文档验证（uuencode 教训）。
5. 每版本发布前 CODE_WIKI 强制同步，禁止跨版本积压。

### 1.2 "最强大"的验收定义（本路线图的北极星）

到 v3.0 结束时，同时满足：

1. **广度**: ≥20 种格式（新增 srv3/EBU-TT-D/MCC 等）+ IMSC 1.1 校验模式；
2. **正确性**: cargo-fuzz ≥1 周 CPU-days 无 crash；W3C IMSC 测试套件子集通过率 ≥90%；全格式 golden round-trip；
3. **触达**: PyPI + npm 包发布且周下载非零；C-FFI 头文件发布；在线 Workbench 与核心同步到同一版本；
4. **性能**: bench 数据公开发布，10k cue 解析/写出对比 2.10.0 基线有可引用的数字；
5. **唯一性**: 上述五项同时满足的 crate 在 crates.io 上只有一个。

---

## 2. 版本详细范围

### 2.11 — v2.11.0「还债与地基」（minor，~1-2 周）

**主题**: 把潜伏债务清零，让后续每个版本的 clippy-matrix 和 fuzz 有干净地基。**不加新格式**。

| # | 项 | 内容 | 类型 |
|---|----|------|------|
| H1 | 最小 feature 测试门禁 | 修复 `--features dfxp,ttml --lib` 等 ~50 个 cfg 缺失 error（测试逐个 gate），CI clippy-matrix/test 矩阵扩到 `dfxp,ttml` 组合 | 测试 |
| H2 | SSA round-trip | 补 SSA 端到端 golden 测试（§14.5 盲区），含 `Format:` 行差异 | 测试 |
| H3 | proptest 升级 | 从 ASCII-only 扩展到 Unicode/CRLF/标签/emoji 字符类；先跑一周观察 flaky | 测试 |
| H4 | wasm.rs 覆盖 | `src/wasm.rs` 6 个函数接入 wasm-bindgen-test（已有 9 个测试的基建） | 测试 |
| H5 | benches feature gate | `[[bench]] required-features` + bench 内 cfg，`--all-targets` 在最小 feature 下可编译 | 构建 |
| H6 | fuzz 起步 | `fuzz/` 目录 + `cargo-fuzz` targets（每格式 parse 一张），CI 加每周定时长跑 job（非阻塞） | 测试 |
| H7 | Workbench 同步 | subtitle-format-conversion 的 wasm 包装层升到 2.11，补 Dfxp/Whisper/Itt/Spruce 四臂（外部仓库，见 §4 协作） | 外部 |
| H8 | normalize 扩展 | `--fix-tags`（修复非法标签配对）+ Transliterate Cyrillic（gap analysis 附录已识别，纯映射表零依赖） | 小功能 |

**验收**: clippy-matrix 三组合全绿；fuzz 每目标 ≥24h 无 crash；proptest 跑 10 万例无 flaky；Workbench 发布新版本。

### 2.12 — v2.12.0「智能时序」（minor，~2-3 周）

**主题**: 从"格式转换器"升级为"字幕时间轴引擎"。这是与所有竞品拉开代差的版本。

| # | 项 | 内容 | 类型 |
|---|----|------|------|
| S1 | `auto_sync` | 锚点同步（alass 算法路线）：给定参考字幕（或 (time,text) 锚点列表），按文本相似度 + 时序单调性求解最优分段线性偏移（支持分段/漂移）。纯 text+time，零外部依赖（D3）。API: `SubtitleFormat::auto_sync(&reference, opts) -> SyncReport` | 新功能 |
| S2 | 对轴 CLI | `subtitler sync <wrong.srt> --reference ref.srt --output fixed.srt`，`--json` 输出 SyncReport（偏移量、锚点数、置信度） | 新功能 |
| S3 | 卡拉OK | ASS `\k`/`\kf`/`\ko` 标签解析进 `TextPart`（新增 per-segment timing），LRC 增强词级 `<mm:ss.xx>` 输出；`split` 支持按音节切开 | 新功能 |
| S4 | 时间轴算子补全 | `stretch`（斜率变换：起点锚定 + 比例缩放）、`snap-to-shot`（吸附到 shot-change，衔接 2.8.0）、`max-duration` 强制拆分 | 新功能 |
| S5 | SyncReport serde | 报告可序列化，pipeline 可用（`PipelineOp::AutoSync`） | API |

**验收**: 用真实错位字幕对（不同 release 的同内容）构造 fixture，锚点同步结果与 alass CLI 输出对齐（±1 cue）；`\k` round-trip 无损；S1–S5 全带测试。

**风险**: 文本相似度算法选型（编辑距离 vs 词袋 vs 归一化编辑距离）——先实现归一化编辑距离 + 时间单调 DP，够用且可测；不要过度调参。

### 2.13 — v2.13.0「广播深格式」（minor，~3-4 周）

**主题**: 进军"深格式"空位——这些领域 Rust 现状全是 v0.1 玩具（↓60–2k），先到先得。

| # | 项 | 内容 | 类型 |
|---|----|------|------|
| B1 | IMSC 1.1 校验模式 | `validate --imsc`：对 TTML/DFXP/iTT 做 IMSC1.1 Profile 约束校验（允许的属性/值域/单位）。以 W3C IMSC1 测试套件子集为 fixture（选 ~50 个代表性用例入库），通过率 ≥90% 写进验收 | 新功能 |
| B2 | MCC 只读 | MacCaption MCC（广播交付实际格式）→ 内部模型，输出侧转 TTML/SCC。解析器按 `mccaption` v0.1.0 未覆盖的完整规范写，spec 用真实样本 fixture（找公开样例 ≥3 份互证） | 新格式 |
| B3 | EBU-TT-D 只读 | EBU-TT Part D（XML）解析——EBU STL 的 XML 亲属，欧洲广播刚需；与 2.7.0 的 STL 共享 EBU 语义 | 新格式 |
| B4 | CEA-708（P2，可延后） | 708 本体服务块解析复杂度高；先做 608-in-708 包装读取（复用 SCC 知识）。若工期超支，砍给 3.0.x | 新格式（可延） |
| B5 | 格式计数流程 | 新格式触发 §3.15 全套文档同步 + 最小构建 ×7 | 流程 |

**验收**: IMSC 套件子集通过率达标；MCC/EBU-TT-D 各有 golden round-trip-or-read 测试；格式计数五处一致（§3.15 自查命令通过）。

**风险**: MCC/EBU-TT 规范细节深，样本获取难——策略：只读、以互证样本为准、解析失败明确报错而非静默跳过。

### 3.0 — v3.0.0「生态与架构」（major，~6-8 周）

**主题**: 让"用上它"这件事在全生态都是一行命令。**破坏性变更集中在此版**（D4）。

| # | 项 | 内容 | 类型 |
|---|----|------|------|
| W1 | workspace 拆分 | `subtitler-core`（纯解析/模型，零 CLI 依赖）+ `subtitler`（CLI，依赖 core）+ `subtitler-wasm`（现 wasm 模块迁出）。编译隔离 + 各自 feature 独立 | 架构 |
| W2 | zero-copy 解析 | `parse_content` 走 `Cow<'_, str>` / 索引式切片，`Subtitle` 生命周期化（或 `Arc<str>` 权衡版）；10k cue 解析内存/耗时对比基线写进 bench 报告 | 架构 |
| W3 | Python 绑定 | PyO3 + maturin，包名 `pysubtitler`：`parse/convert/validate/sync` 对齐 CLI 能力；PyPI 发布 + CI 构建 macos/linux/windows wheel | 生态 |
| W4 | Node 绑定 | napi-rs，包名 `subtitler-node`（prebuild 二进制分发）；Workbench 的 JS 侧可直接受益 | 生态 |
| W5 | C FFI | `cdylib` + `cbindgen` 生成头文件，覆盖 parse/convert/validate/sync 四个入口，error code 约定 | 生态 |
| W6 | async 现代化 | `SubtitleFormat` trait 的 async 方法改 native `async fn in trait`（edition 2024 已稳），删除 Arc-wrapped workaround | 架构 |
| W7 | 文档站 | mdBook（教程 + 每格式一个 chapter + pipeline 配方库）；docs.rs 全类型示例化 | 文档 |
| W8 | MIGRATION | 3.0 迁移指南：workspace 路径变化 + zero-copy 生命周期影响 + trait 方法签名 | 文档 |

**验收**: PyPI/npm 包 `pip install`/`npm i` 即用且与 CLI 同版本发布；bench 报告发布（对比 2.10.0）；MIGRATION 完整；`cargo +1.85`（或届时新 MSRV 决策）全绿。

**风险**: zero-copy 重构波及全部格式模块——策略：以 `Arc<str>` 共享所有权做第一版（风险可控），`&'a str` 生命周期版降级为 3.1 目标；W1/W2 分两个 PR 系列串行，每步保持门禁全绿。

---

## 3. 竞品对照（v3.0 完成时的预期格局）

| 能力 | subtitler 3.0 | subparse ↓48k | webvtt-parser ↓54k | subtile ↓20k | alass | SubtitleEdit (C#) |
|------|--------------|---------------|--------------------|--------------|-------|--------------------|
| 格式数 | **20+** | 3 类 | 1 | PGS 系 | 内部 | 200+（GUI） |
| 自动检测+互转 | ✅ | 部分 | ❌ | ❌ | ❌ | ✅（GUI） |
| 广播 QC 预设 | ✅ 唯一 | ❌ | ❌ | ❌ | ❌ | 部分（GUI） |
| 锚点自动对轴（库 API） | ✅ 唯一 | ❌ | ❌ | ❌ | CLI 形态 | ❌ |
| IMSC 校验 | ✅ 唯一 | ❌ | ❌ | ❌ | ❌ | 部分 |
| WASM | ✅ + 在线版 | ❌ | ❌ | ❌ | ❌ | ❌ |
| PyPI / npm | ✅ / ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| fuzz + 回归纪律 | ✅ 唯一 | ❌ | ❌ | ❌ | ❌ | ❌ |

（↓数字为 2026-10-31 crates.io 下载量快照）

---

## 4. 协作与外部仓库

- **subtitle-format-conversion（Workbench）**: 2.11 H7 同步核心版本 + 补齐 4 格式臂；3.0 W4 之后 JS 侧可直接换 `subtitler-node`，WASM 包瘦身。
- **subtitler skill（`~/.agents/skills/subtitler/`）**: 新 CLI 能力（sync/validate --imsc）落地后同步更新 SKILL.md。

---

## 5. 明确不做（沿袭 gap analysis §6 并新增）

| 项 | 理由 |
|----|------|
| 图像字幕 OCR（VobSub/PGS 识别） | 需 Tesseract 等重依赖；容器解析可议（P2 之后再评估），OCR 永不做 |
| 音频驱动的自动对轴（ffsubsync 式） | 需音频解码/频谱分析栈；锚点同步（S1）已覆盖绝大多数场景（D3） |
| 视频解码 / scene detection | 走外部输入（EDL），不进库（gap analysis §6 既有结论） |
| 字幕渲染 / GUI | libass/NLE 的职责 |
| 云服务 / SaaS 功能 | 非库职责 |

---

## 6. 成功指标与度量

| 维度 | 指标 | 基线（2026-10-31） | 3.0 目标 |
|------|------|--------------------|----------|
| 广度 | 格式数 / 校验模式 | 17 / 5 预设 | 20+ / 预设+IMSC |
| 正确性 | fuzz CPU-days 无 crash / IMSC 套件 | 0 / 无 | ≥7 / ≥90% |
| 触达 | crates.io ↓ / PyPI / npm | 5.4k / ❌ / ❌ | 排名第一梯队 / 发布 / 发布 |
| 性能 | 10k cue parse+write | 无公开基线 | bench 报告 + 同比提升 |
| 社区 | 外部 PR 合入数 | 2（localcc、huou07） | 翻倍 |

---

## 7. 风险登记

| 风险 | 概率 | 缓解 |
|------|------|------|
| zero-copy 重构失控 | 中 | 第一版用 `Arc<str>` 保守方案；串行小步 PR；门禁全绿才进下一步 |
| MCC/EBU-TT 规范样本不足 | 中 | 只读 + 多样本互证 + 明确报错；无样本则砍范围 |
| 绑定维护负担（3 平台 × 3 OS） | 高 | CI matrix 全自动构建；绑定 API 面刻意小（四入口）；版本与核心严格同拍 |
| 范围蔓延（"最强大"诱惑） | 高 | 每版单一主题；新想法一律进 §5 非目标清单或下一版 spec，不进当版 |
| 与 07-18 spec 的 3.0 定义冲突 | 低 | 本文件显式取代其 3.0/3.1 章节（AI 翻译集成除外——归入 2.12 S1 的输入端支持） |
