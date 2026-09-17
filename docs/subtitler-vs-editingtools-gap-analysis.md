# 功能差距分析：subtitler vs editingtools.io 字幕工具

> 分析日期：2026-09-17
> 分析基准：subtitler（工作树 Cargo.toml v2.1.0，13 格式，344 测试基线参考 AGENTS §8）
> 参考来源：https://editingtools.io/subtitles/（Subtitle Tool & Converter V 0.76，英文原版 + 中文版交叉核对）
> 状态：完整版（v2，补充首轮遗漏的 6 个功能域）
>
> **⚠️ 状态核对（2026-09-17 实施时修正）**：首轮代码分析基于 7 月的 v2.1.0 快照（13 格式），而当前工作树实为 **v2.6.1 / 15 格式**——**DFXP 与 Whisper JSON 已于 v2.4.0 实现**（含检测、parse/generate、CLI、测试）。下文相应条目已标注"已实现"；P1 剩余项（guidelines 预设 / min-gap / dedup 双语义）已在 `feature/p1-gap-analysis` 分支落地。

---

## 目录

1. [格式支持对比](#1-格式支持对比)
2. [功能特征对比（六大功能域）](#2-功能特征对比六大功能域)
3. [缺口严重性分级](#3-缺口严重性分级)
4. [架构级观察与实现草图](#4-架构级观察与实现草图)
5. [综合建议与版本规划](#5-综合建议与版本规划)
6. [明确不做的项](#6-明确不做的项)
7. [Quick Summary](#7-quick-summary)

---

## 1. 格式支持对比

editingtools.io 的转换是三向的：**输入（From）/ 输出（Convert To）** 支持的格式集合不同。下面按 subtitler 视角整理。

### 1.1 双方都支持（核心字幕格式）

| 格式 | subtitler | editingtools.io | 备注 |
|------|:---------:|:---------------:|------|
| SRT | ✅ | ✅ 输入+输出 | |
| VTT / WebVTT | ✅ | ✅ 输入+输出 | editingtools 另有 "No Numbering in Export (VTT)" 选项 |
| ASS | ✅ | ✅ 输入+输出 | |
| SUB / SubViewer | ✅ | ✅ 输入+输出 | |
| TTML | ✅ | ✅ 输入+输出 | |
| SBV (YouTube) | ✅ | ✅ 输入+输出 | |
| SCC | ✅ | ✅（仅输入，见中文版页面） | |
| EBU STL (N19) | ✅ | ✅ 输入 | Avid Media Composer 导出路径也有 |

### 1.2 editingtools.io 有、subtitler 无（按价值排序）

#### 🔴 高价值

| 格式 | 说明 | 建议优先级 | 实现思路 |
|------|------|:---------:|---------|
| **DFXP** (.dfxp) | W3C Distribution Format Exchange Profile，TTML 前身，namespace 不同结构几乎一致 | ✅ **已实现**（v2.4.0，`dfxp.rs`，与 TTML 复用解析） |
| **Whisper AI Transcript** (.json) | OpenAI Whisper 转录 JSON（`segments[]: {start, end, text, id}`，秒为单位的浮点） | ✅ **已实现**（v2.4.0，`whisper.rs`，含 round-trip） |
| **iTT (iTunes Timed Text)** (.itt) | Apple iTunes Store/TV 交付规范，IMSC1 profile，带严格校验规则（字体/区域/阅读速度约束） | 🟡 P2 | TTML/IMSC 家族第三成员，复用 `ttml.rs` 底座 + 额外校验层；对标 editingtools 输出项 |
| **Whisper Subtitles** (.srt) | Whisper 生成的 SRT 变体 | 🟡 P2 | srt 模块容忍性解析即可覆盖，验证现有解析器是否兼容 |

#### 🟡 中等价值

| 格式 | 说明 | 建议优先级 | 实现思路 |
|------|------|:---------:|---------|
| **Spruce STL** (.stl) | Spruce DVD Maestro 字幕（文本格式，`00:00:01:00` 帧时码 + 图形引用），与 EBU STL 完全不同 | 🟡 P2 | 独立新模块；注意与现有 `ebu_stl` 的 `.stl` 扩展名冲突需检测签名区分 |
| **SubRip + Speaker Names** (.srtx) | DaVinci Resolve / 带 `<speaker>:` 前缀的 SRT | 🟡 P2 | subtitler 的 `Subtitle.actor` 字段可直接映射，srt 模块小扩展 |
| **QuickTime TeXML** (.txt) | QuickTime Pro 文本字幕 | 🟢 P3 | 简单文本格式 |
| **DS Caption / Subcap** (.txt) | Avid 字幕文本格式 | 🟢 P3 | 简单文本格式 |
| **Transcript** (.txt 有/无时码) | 纯文本转录导出 | 🟢 P3 | subtitler 导出侧一行 `map` 的事；可作为 `--to transcript` 输出选项 |

#### 🟢 低价值 / 出圈范畴

| 格式 | 说明 | 建议优先级 |
|------|------|:---------:|
| **表格类**（xlsx / xls / CSV / TSV / ODS / Numbers） | 字幕 → 表格，QC 审阅用 | 🟢 CSV/TSV 可作 P3 顺带项；xlsx/ods 不做（引入重依赖） |
| **PDF** | 字幕 → PDF 阅读文档 | 🟢 不做（排版库重依赖） |
| **RTF** | 富文本 | 🟢 不做 |
| **AutoDesk XML / Subtitle Horse JSON / Subtext / EDITINGTOOLS.IO JSON** | 小众专有格式 | 🟢 不做（维护成本 > 收益） |
| **NLE 标记类**（Premiere Sequence/Markers XML+CSV、FCPXML/FCP7 XML、Avid Marker TXT/XML、DaVinci EDL、Audition/Prelude/Frame.io CSV、Pro Tools .mid/.ptx） | 字幕 ↔ NLE 标记/标题互转 | ⚪ 战略选项，见 §4.5 |

### 1.3 subtitler 有、editingtools.io 无（差异化优势）

| 格式 | 说明 |
|------|------|
| LRC | 歌词格式（TTML 歌词教程已有 example） |
| MPL2 | MPL2 格式 |
| SAMI | Microsoft SAMI |
| MicroDVD | 帧格式 |
| SSA | SubStation Alpha（editingtools 只支持 ASS） |

> **格式小结**：格式面 subtitler 13 vs editingtools 核心字幕 ~11，广度已领先。最值得补的是 **DFXP + Whisper JSON**（AI 生态桥接，合计 ~1 天），其次是 **iTT**（Apple 交付生态）和 **Spruce STL**（DVD 母版流程）。注意 `.stl` 扩展名有两个不同格式（Spruce vs EBU），检测签名必须区分——这是新格式引入时的检测器设计教训点。

---

## 2. 功能特征对比（六大功能域）

editingtools 的工具定位是 "convert + check subtitles for guidelines, repair or correct"。按功能域展开：

### 2.1 清理（Cleanup）

| editingtools 功能 | subtitler 现状 | 缺口 |
|------------------|---------------|------|
| Remove superfluous spaces | ✅ `normalize_whitespace` | — |
| Remove empty subtitles | ✅ `PipelineOp::FilterEmpty` | — |
| Remove subtitles with 0 frames duration | ⚠️ `validate()` 能报 ZeroDuration，但无过滤 PipelineOp | 小 |
| Remove speaker / marker names | ⚠️ `strip_hearing_impaired` 内置 `RE_SPEAKER_LABEL`（模式硬编码 `>>`/`- `/`NAME:`） | 小——可参数化 |
| **Convert roll-up captions → full subtitles** | ❌ | **中**——SCC 专属：滚动字幕去重转逐条 |
| **Remove repeating lines** | ❌ | **高**——连续重复文本行去重 |
| **Merge identical subtitles** | ❌ | **高**——时间重叠/相邻且文本相同的 cue 合并（与上行语义不同：这个合并时间轴，那个删行） |
| Remove individual words or tags | ❌ | 小——参数化词/标签删除 |
| Remove text between `[ ]` `( )` `{ }` 自定义 | ⚠️ `strip_hearing_impaired` 硬编码括号模式 | 小——提取为可配置 `remove_between(open, close)` |
| Remove `<font>` / text formatting | ✅ `Subtitle::strip_tags` | — |
| **Remove timecodes（导出纯文本转录）** | ❌ | 小——`--to transcript` 输出模式 |
| **Replace line breaks with `\|`** | ❌ | 小 |
| **Remove line breaks（≤42 字符的 cue 合并行 / 全部合一行）** | ❌ | 中——注意 "42" 是 guideline 语义，应参数化 |
| **Clear all subtitles (Translation Layout)** | ❌ | 小——保留时间轴清空文本，翻译模板用 |
| **语言字符过滤器**（20 语种白名单：英/西/德/法/意/波/葡/日/韩/乌/俄/芬/挪/瑞/丹/泰/越/阿/希/土） | ❌ | **中**——按 Unicode 区间过滤非目标语种字符 |

### 2.2 时序（Timing）

| editingtools 功能 | subtitler 现状 | 缺口 |
|------------------|---------------|------|
| Fix overlapping subtitles | ✅ `remove_overlaps` | — |
| **Set minimum duration** | ✅ `enforce_min_duration` | —（CLI 未暴露，仅库 API + Pipeline） |
| **Set maximum duration** | ✅ `enforce_max_duration` | —（同上） |
| **Set minimum gap between subtitles** | ❌ **无 enforce_min_gap**；`ValidationIssue` 也只有 TooLongGap 没有 TooShortGap | **高**——广播规范硬性要求（Netflix：cue 间最少 2 帧）。验证 + 强制两侧都缺 |
| Shift by +/- | ✅ `shift`（仅毫秒） | 小——CLI 支持 frames/seconds/minutes 单位（内部换算即可） |
| **Extend / Shorten all subtitles by 1 frame** | ❌ | 小——实现是 `enforce` 的特例，1 行逻辑 |
| **Add start placeholder subtitle + Sequence Start TC** | ❌ | 小——插入空 placeholder cue 对齐时间线起点 |
| **Change framerate（drop-frame 感知）** | ⚠️ `transform_framerate` 是线性比例缩放；SCC 已有 DF 算法（v2.1 C2 修复）但通用变换不感知 DF；无 "total length stays the same"（抽帧补齐）模式 | **中**——帧格式转换的正确性关键 |
| Fix misinterpreted out times | ❌ | 小——修复解析歧义产生的结束时间（如 VTT 无结束时间的 cue） |
| **Merge multiple subtitles within a defined range** | ⚠️ `merge_adjacent(max_gap_ms)` 按间隙合并 | 小——加按时间窗合并模式 |
| **Merge word-by-word transcript into subtitle** | ❌ | **中**——Whisper 词级时间戳 → cue（对 AI 转录后处理关键） |

### 2.3 规范检查（Guidelines / QC）⭐ 最大价值缺口

editingtools 内置广播机构规范预设：**ARD / ORF / SRF / ZDF、BBC、Netflix、TED、Channel4**。

| subtitler 现状 | 缺口分析 |
|---------------|---------|
| `validate_extended(max_chars, max_gap_ms, max_cps)` 三个全局阈值 | ✅ 通用校验已有底座 |
| `ValidationIssue` 7 种变体 | ❌ 缺：**TooShortGap**（Netflix 2 帧规则）、**LineCountExceeded**（每 cue 最多 2 行）、**TooShortDuration**（Netflix 最短 5/6 秒）、**ReadingSpeed 变体**（WPS 口径）、**起始/结束位置规范**（cue 不跨越 shot change） |
| 无预设概念 | ❌ 缺 **GuidelinePreset** 抽象：`Netflix` / `BBC` / `TED` / `ARD` / `Channel4` 各自的 (max_chars, max_lines, min_duration, max_duration, min_gap, max_cps) 参数包 |

**这是报告 v2 新增的最大发现**：subtitler 的验证是"参数面板"，editingtools 是"一键规范体检"。广播/流媒体交付是字幕工具的专业付费场景，Netflix 规范预设是刚需（Netflix Timed Text Style Guide 是事实标准）。实现成本低（预设 = 参数包 + 2-3 个新 ValidationIssue 变体），价值高。

### 2.4 字符与标签（Characters & Tags）

| editingtools 功能 | subtitler 现状 | 缺口 |
|------------------|---------------|------|
| Fix invalid tags | ⚠️ 解析侧容忍；无"修复"操作 | 小——如 `<b>` 无闭合配对修复 |
| **Spacing of opening hyphen**（对话破折号 `-Hello` → `- Hello`） | ❌ | 小——normalize 扩展，欧语对话字幕规范细节 |
| **Caps lock to normal casing** | ❌ | 小——`SCREAMING` → `Sentence case` 启发式 |
| **Transliterate Cyrillic alphabet** | ❌ | 小——西里尔 → 拉丁音译表 |

### 2.5 场景切换（Shot Changes）⭐ 专业级缺口

editingtools 支持 **导入 EDL/XML shot change 列表 + "frames before/after shot change" 规则**——即根据镜头切点调整字幕切分，Netflix 规范要求 cue 不得跨越 shot change（除非对话延续）。

subtitler：完全空白。这是连接 §2.3 guideline 预设的自然延伸，输入正好可以是 editingtools 同款 EDL——而 EDL 解析是 §1.2 中标记为"出圈"的格式，在这里有了新的入圈理由（作为 shot list 输入而非字幕转换目标）。

建议：`subtitler apply-shot-changes <sub> <shotlist.edl> --before 2 --after 12` 这类 API/CLI；核心逻辑是纯时序计算，不需要视频解析。

### 2.6 AI 能力

| editingtools | subtitler 现状 | 缺口 |
|--------------|---------------|------|
| AI - Subtitle Translator（LLM 翻译） | `Translator` trait + `DummyTranslator`（`src/quality.rs`） | **P1**——trait 已就绪，缺真实 adapter（OpenAI/DeepL/本地 Ollama）。路线图 v3.0 已规划 |
| Whisper 转录导入（JSON/txt/srt） | ❌ | **P1**——见 §1.2 |
| AI - Scene Cut Detection | ❌ | ⚪ 需要视频分析，超出字幕库范畴（但见 §2.5，可接收外部 shot list） |
| AI - Face Detection / Audio 系列 | ❌ | ⚪ 出圈，不做 |

### 2.7 subtitler 独有优势（保持并强化）

| 功能 | 说明 |
|------|------|
| **Pipeline 声明式流水线** | JSON 配置操作链——editingtools 是 GUI 选项堆叠，无等价物；本报告新增 op 建议全部应落入 `PipelineOp` 保持此优势 |
| 质量报告 | CPS/WPM/断行质量（`quality.rs`） |
| 深度时序验证 | 7 种 ValidationIssue |
| OCR 纠错 + 听障标签剥离 | `normalize.rs` |
| 智能断句 | `split_long` + `optimize_line_breaks`（词边界 + 标点/连词优先断点） |
| StreamingParser / WASM / HTTP 输入 | 架构层优势 |
| Builder API | `SubtitleBuilder` 链式 |

---

## 3. 缺口严重性分级

### 🔴 P1 — 立即着手（低投入高回报）

| # | 缺口 | 估算 | 落点 | 状态 |
|---|------|------|------|------|
| 1 | **GuidelinePreset 规范预设**（Netflix/BBC/TED/ARD/Channel4 参数包 + TooShortGap/LineCountExceeded/TooShortDuration 3 个新 ValidationIssue） | 1 天 | `guidelines.rs` / `model/validation.rs` | ✅ 已落地（`validate_guideline` + CLI `--guideline`） |
| 2 | **Remove repeating lines + Merge identical subtitles**（两个 dedup 语义都要） | 0.5 天 | 2 个新 PipelineOp | ✅ 已落地（`RemoveRepeatingLines` + `MergeIdentical`） |
| 3 | **enforce_min_gap**（强制最小间隙，配 TooShortGap 验证） | 0.5 天 | `model/trait.rs` + PipelineOp | ✅ 已落地（`EnforceMinGap`） |
| 4 | **DFXP 格式** | 0.5 天 | `ttml.rs` 家族 | ✅ 上游 v2.4.0 已实现 |
| 5 | **Whisper JSON 导入** | 0.5 天 | 新模块，serde 直映射 | ✅ 上游 v2.4.0 已实现 |

### 🟡 P2 — 中期（专业场景，合计 ~1.5 周）

| # | 缺口 | 估算 |
|---|------|------|
| 6 | 语言字符过滤器（20 语种白名单） | 4h |
| 7 | normalize 扩展包：短行合并/全合一行/换行→`\|`/破折号间距/大小写归一/括号内容可配置删除 | 1 天 |
| 8 | Drop-frame 感知的 framerate 变换（+ "总时长不变"模式） | 1-2 天 |
| 9 | roll-up → full subtitle 转换（SCC 模块） | 1 天 |
| 10 | 词级时间戳合并（Whisper word → cue） | 1 天 |
| 11 | Shot change 规则（EDL/XML shot list 输入 + before/after 帧数） | 2 天 |
| 12 | **iTT (iTunes Timed Text)** 格式 | 2 天 |
| 13 | Spruce STL 格式（注意 `.stl` 签名区分） | 1-2 天 |
| 14 | AI 翻译 adapter（OpenAI/DeepL，feature-gated `ai-openai`/`ai-deepl`） | 2-3 天 |
| 15 | srtx（speaker names）映射到 `Subtitle.actor` | 0.5 天 |

### 🟢 P3 — 低优先级（顺手做或战略搁置）

| # | 缺口 | 说明 |
|---|------|------|
| 16 | Extend/Shorten all by N frames | enforce 特例，做 P1#3 时顺带 |
| 17 | CLI 单位扩展（frames/seconds/minutes 换算） | shift/duration/gap 全家 |
| 18 | Start placeholder + Sequence Start TC | 小 |
| 19 | Clear text (translation layout) | 小 |
| 20 | Transcript 导出模式（--to transcript） | 小 |
| 21 | CSV/TSV 表格导出 | QC 审阅用，无新依赖 |
| 22 | 西里尔音译表 | 小 |
| 23 | 批量处理 CLI | shell 循环可替代 |
| 24 | VTT 导出无编号选项 | 小 |

---

## 4. 架构级观察与实现草图

### 4.1 GuidelinePreset 设计草图（P1#1）

```rust
// src/quality.rs（或新 src/guidelines.rs）
pub struct Guideline {
    pub name: &'static str,
    pub max_chars_per_line: usize,   // Netflix: 42
    pub max_lines: usize,            // Netflix: 2
    pub min_duration_ms: u64,        // Netflix: 833 (5/6 s)
    pub max_duration_ms: u64,        // Netflix: 7000
    pub min_gap_ms: u64,             // Netflix: 2 frames ≈ 83ms @24fps
    pub max_cps: f64,                // Netflix: 20 (adult), TED: ~25?
}

pub enum GuidelinePreset { Netflix, Bbc, Ted, Ard, Channel4, Custom(Guideline) }

impl GuidelinePreset {
    pub fn guideline(&self) -> Guideline { /* 参数包 */ }
}

// validate 侧新增 ValidationIssue 变体：
// TooShortGap { index, gap_ms, min_gap_ms }
// LineCountExceeded { index, lines, max_lines }
// TooShortDuration { index, duration_ms, min_ms }
// SpanShotChange { index }  ← 配合 §2.5 shot list 输入
```

> 注意：各机构的具体数值**不能纸面推导**（AGENTS §6.4 教训），写预设前须逐条对照各官方 Style Guide 原文并用 fixture 测试锁定。

### 4.2 dedup 双语义设计草图（P1#2）

```rust
// 语义 A：Remove repeating lines —— 连续相同文本的 cue 删后者（保留时间轴覆盖）
PipelineOp::RemoveRepeatingLines
// 语义 B：Merge identical —— 文本相同且时间重叠/相邻的 cue 合并时间轴
PipelineOp::MergeIdentical
```

两者易混淆，文档必须给出对照示例；测试要覆盖 "文本相同但时间不相邻 → 不动" 的反例。

### 4.3 Whisper JSON 映射（P1#5）

```rust
#[derive(Deserialize)]
struct WhisperJson { segments: Vec<WhisperSegment> }
#[derive(Deserialize)]
struct WhisperSegment {
    id: usize,
    start: f64,  // 秒！必须 ×1000 转毫秒（时间戳统一 u64 毫秒，AGENTS §0）
    end: f64,
    text: String,
}
// 词级时间戳（words[] 数组，若存在）留给 P2#10
```

### 4.4 最小间隙强制（P1#3）

```rust
// model/trait.rs 新增 default method，模式仿 enforce_min_duration：
fn enforce_min_gap(&mut self, min_gap_ms: u64) {
    // 前一条 end = max(前一条 end, 后一条 start - min_gap_ms) 时须防负时长：
    // 若 prev.duration 被压破 min_duration，优先保 min_gap 并对 prev.end 取 max(prev.start, ...)
}
```

边界用例：两 cue 完全重叠时 min_gap 与 remove_overlaps 的组合顺序——应先 `remove_overlaps` 再 `enforce_min_gap`，Pipeline 文档要写明推荐顺序。

### 4.5 NLE 互转：战略判断

editingtools 一半的格式面是 NLE 标记/标题互转（Premiere/FCP/Avid/Resolve）。这对 subtitler 是**出圈诱惑**：

- 反对：路线图 §5 明确 "不做视频嵌入/时间轴编辑，这是 NLE 的职责"；marker XML 格式多且无正式规范，维护黑洞。
- 支持：字幕 → FCPXML Titles / Premiere Graphics 是真实交付流需求；且 EDL 作为 shot list 输入（§2.5）已经需要最小 EDL 读取能力。

**建议**：3.0 前不碰 NLE 输出；P2#11 只做 "EDL 作为 shot list 的只读解析"，用 `#[cfg(feature = "edl_shotlist")]` 隔离，不承诺通用 EDL 转换。

### 4.6 AI 翻译 adapter 注意点

- `Translator::translate` 逐行调用会打爆 API 限速：adapter 需要批量合并 + 重试 + `reqwest` 已有依赖（`http` feature 下）。
- 翻译会改变文本长度 → 翻译后必须跑 `auto_extend_for_cps` / `split_long` 收尾，CLI `translate` 命令应把这两步作为默认后处理。
- feature-gate：`ai-openai` / `ai-deepl` 不进 default features（网络 API 不该是默认依赖）。

---

## 5. 综合建议与版本规划

结合现有路线图节奏（2.x 主题制、每版本单一主题）：

```
近期版本（正确性/格式主题）
├── P1#4 DFXP + P1#5 Whisper JSON        ✅ 上游 v2.4.0 已实现
└── P1#1 GuidelinePreset + P1#3 min_gap  ✅ 已落地（feature/p1-gap-analysis）

中期版本（专业场景主题）
├── P1#2 dedup 双语义                    ✅ 已落地（feature/p1-gap-analysis）
├── P2#6 语言过滤 + P2#7 normalize 包    ✅ 已落地（21 语种 + 3 新函数 + CLI 补课）
├── P2#8 DF 感知变换 + P2#9 roll-up      ✅ 已落地（Timebase + reinterpret + snap + rollup）
├── P2#10 词级合并 + P2#12 iTT             ✅ 已落地（whisper --from-words + itt 格式，16 格式）
└── P2#13 Spruce STL（母版主题，待做）

3.0（已规划方向不变）
├── P2#14 AI 翻译 adapter（v3.0 AI 能力集成，已有 trait 底座）
├── P2#11 Shot change（配合 v3.0 深挖：IMSC/CEA-708 同属专业交付）
└── NLE 输出：继续观望（见 §4.5）
```

**落地顺序上有一条依赖链**：P1#1（TooShortGap 验证）→ P1#3（min_gap 强制）→ P2#8（DF 感知，因为 min_gap 以帧计的规范值需要正确帧换算）。按此顺序实现可避免返工。

---

## 6. 明确不做的项

| 项 | 理由 |
|----|------|
| PDF / xlsx / ods 导出 | 重依赖（排版/表格库），违背精简依赖原则；CSV 已够 QC 审阅 |
| AI 视频类（Scene Cut 检测/人脸/音频系列） | 需要视频分析引擎，超出字幕库范畴；shot change 走外部输入（§2.5） |
| 通用 NLE 标记/序列互转 | 路线图 YAGNI 边界（§5），维护黑洞；仅保留 EDL 只读 shot list 特例 |
| 密码保护导出 / ZIP 打包 / 邮件通知 | editingtools 的 SaaS 运营功能，非库职责 |
| Font/Resolution/Text Style 预设（渲染侧参数） | 渲染是 libass/NLE 的职责 |
| Pro Tools .ptx / .mid 标记 | 音频工程范畴 |
| AutoDesk/Subtitle Horse/Subtext/专有 JSON | 过于小众 |

---

## 7. Quick Summary

> subtitler 格式广度领先（15 vs ~11 核心格式），工程底座（Pipeline/验证/normalize/streaming/WASM）全面强于 editingtools 的转换器。
>
> **本轮深挖新发现的三大专业缺口**（v1 报告未覆盖）：
> 1. **广播规范预设**（Netflix/BBC/TED/ARD/Channel4）——验证体系从"参数面板"升级为"一键规范体检"，这是专业付费场景的入场券，且实现成本低；
> 2. **最小间隙**（TooShortGap）——验证和强制两侧全缺，而它是 Netflix 规范的硬性条款；
> 3. **Shot change 规则**——Netflix 规范的另一半，只需 EDL 只读解析 + 纯时序计算。
>
> **落地进度（2026-09-17）**：P1 全部完成——guidelines 预设、enforce_min_gap、dedup 双语义在 `feature/p1-gap-analysis` 分支落地（24 个新测试）；DFXP/Whisper JSON 上游 v2.4.0 已实现。下一批建议：语言过滤器 + normalize 扩展包（清理主题），然后 DF 感知变换（依赖链最后一环）。

---

## 附录 A：editingtools.io 原始功能清单（抓取存档）

<details>
<summary>输入格式（From，完整列表）</summary>

ASS, DS Caption/Subcap (.txt), DFXP, STL-Spruce, STL-EBU N19, SUB-SubViewer, TTML, SRT, VTT, SBV, SCC, TSV, TXT, RTF, 无时码 TXT, JSON, EDL(locators), EDL(clip names), ProTools Midi Marker(.mid), Avid MC(DS Caption/EBU N19/Marker Text/Marker XML), Premiere(Subtitles SRT/Transcript CSV/Sequence Markers XML/Essential Graphics XML/Text in Graphics XML/Markers CSV/Text Panel TXT), DaVinci(Timeline Markers EDL/Subtitles .srtx), FCPX(.fcpxml titles/.fiojson), FCP7(Timeline&Clip Marker XML/Text Fields XML), Frame.io CSV, Audition CSV, Prelude XML, Whisper Segments TXT/Whisper SRT/Whisper JSON
</details>

<details>
<summary>输出格式（Convert To，完整列表）</summary>

SRT, VTT, SBV, ASS, DFXP, <b>iTT (.itt)</b>, QT txt, Spruce STL, SUB, TTML, srtx(speaker names), FCPXML(Titles), PDF, TXT, Transcript(formatted/unformatted), xlsx, xls, Google Sheets CSV, Numbers CSV, ODS, CSV(comma/semicolon), TSV, EDITINGTOOLS.IO JSON, Avid(DS Caption/EBU N19/Marker Text 8/16 色/Marker XML/ScriptSync/PTX), Premiere(Sequence Markers XML), FCPX/DaVinci(Subtitles as Titles), Sonaar.io TTML
</details>

<details>
<summary>清理 / 时序 / 规范 / 字符（完整列表）</summary>

清理: Remove superfluous spaces / empty subtitles / 0-frame subtitles / speaker names / roll-up→full / repeating lines / individual words or tags / characters of other languages / line breaks(≤42 chars 或全部) / Tags between [ ] ( ) { } / `<font>` / timecodes / line breaks→`|` / Clear all (Translation Layout)

时序: Fix overlapping / Merge identical / Merge in range (seconds) / Merge word-by-word transcript / Min duration / Max duration / Min gap / Shift ±(frames/ms/s/min) / Start placeholder + Sequence Start TC / Change framerate (NDF/DF, total-length-preserving) / Fix misinterpreted out times / ±1 frame

规范预设: ARD/ORF/SRF/ZDF, BBC, Netflix, TED, Channel4

字符: Fix invalid tags / opening hyphen spacing / Caps lock→normal casing / Transliterate Cyrillic / VTT no numbering

Shot changes: Add EDL/XML shot change rules (frames before/after shot change)
</details>
