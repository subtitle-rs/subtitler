# 功能差距分析：subtitler vs editingtools.io 字幕工具

> 分析日期：2026-07-18
> 分析基准：subtitler v2.1.0（13 格式 ✓），路线图 v2.2/v2.3/v3.0 已规划
> 参考来源：https://zh.editingtools.io/subtitles/

---

## 1. 格式支持对比

### 双方都支持的格式（9 种）

| 格式 | subtitler | editingtools.io | 备注 |
|------|-----------|-----------------|------|
| SRT | ✅ | ✅ | |
| VTT / WebVTT | ✅ | ✅ | |
| ASS | ✅ | ✅ | |
| SUB (SubViewer) | ✅ | ✅ | |
| TTML | ✅ | ✅ | |
| SBV (YouTube) | ✅ | ✅ | |
| SCC | ✅ | ✅ | |
| EBU STL | ✅ | ✅ (标注 EBU N19 Caption File) | |
| STL (Spruce) | ❌ | ✅ | Spruce DVD Maestro 字幕（与 EBU STL 不同）|

### editingtools.io 有、subtitler 无的格式

| 格式 | 说明 | 建议优先级 | 备注 |
|------|------|-----------|------|
| **DFXP** | W3C Distribution Format Exchange Profile（TTML 前身） | 🔴 高 | 与现有 TTML 模块高度复用，改动量极小 |
| **Whisper AI JSON** | OpenAI Whisper 转录 JSON 格式 | 🔴 高 | AI 工作流的桥接格式，正值 AI 热潮 |
| **Whisper Subtitles (.srt)** | Whisper 生成的 SRT 变体 | 🟡 中 | 变体级别，可在 srt 模块内扩展 |
| **Whisper Segments (.txt)** | Whisper 纯文本片段输出 | 🟢 低 | 纯文本格式，无时间码 |
| **Spruce STL** | Spruce DVD Maestro 字幕 | 🟡 中 | 专业影视用户的价值格式 |
| **Premiere Pro SRT (.srtx)** | Adobe Premiere 的 SRT 变体 | 🟢 低 | 工具专属扩展名 |
| **DaVinci Resolve SRT** | DaVinci Resolve 字幕导出格式 | 🟢 低 | 工具专属 |
| **AutoDesk Subtitle (.xml)** | Autodesk 软件用 XML 字幕格式 | 🟢 低 | 非常小众 |
| **Subtitle Horse (.json)** | subtitleshorse.com 的 JSON 格式 | 🟢 低 | 小众网站格式 |
| **Subtext (.txt)** | Subtext 软件用纯文本格式 | 🟢 低 | 功能有限 |
| **EDL（两款）** | Edit Decision List（locators / clip names） | 🟢 低 | 非字幕范畴 |
| **EDITINGTOOLS.IO JSON** | editingtools 自用 JSON 格式 | 🟢 低 | 专有格式 |
| **FCPXMLD** | Final Cut Pro XML | 🟢 低 | NLE 导出格式 |
| **Transcript (.txt / unformatted)** | 纯文本转录本 | 🟢 低 | 纯文本 |

### subtitler 有、editingtools.io 无的格式（5 种）

| 格式 | 说明 |
|------|------|
| LRC | 歌词格式 |
| MPL2 | MPL2 字幕格式 |
| SAMI | Microsoft SAMI |
| MicroDVD | MicroDVD 字幕 |
| SSA | SubStation Alpha（ASS 前身） |

> **格式小结**：最值得补充的是 **DFXP**（低投入高回报，与 TTML 代码复用率高）和 **Whisper JSON**（AI 生态关键桥接）。

---

## 2. 功能特征对比

### 2.1 editingtools.io 有、subtitler 无的功能

| 功能 | 说明 | 建议优先级 | 现有底座 |
|------|------|-----------|---------|
| **AI 字幕翻译** | 调用 LLM 翻译字幕文本 | 🔴 高 | `src/quality.rs` 已有 `Translator` trait + `DummyTranslator`；v3.0 路线图已列 |
| **去除重复字幕行** | 检测并移除内容相同的连续字幕 | 🔴 高 | 新增 PipelineOp 即可，实现简单 |
| **语言字符过滤器** | 仅保留指定语言字符（移除其他语种文本） | 🟡 中 | Unicode 区间检查 |
| **短行合并** | 字符数 ≤42 的字幕移除换行 | 🟡 中 | 一行逻辑，可与 `normalize` 合并 |
| **全部移除换行** | 所有换行改为空格 | 🟡 中 | 简单选项 |
| **换行替换为分隔符** | `\n` → `\|` | 🟡 中 | 参数化选项 |
| **Whisper 转录导入** | 导入 Whisper JSON/txt 转录 | 🟡 中 | 与 AI 翻译联动 |
| **roll-up captions → full subtitles** | SCC 专属：滚动字幕转标准逐条字幕 | 🟡 中 | SCC 模块扩展 |
| 批量处理 | 一次处理多个文件 | 🟢 低 | CLI 级功能 |
| 密码保护导出 | PDF/文档密码加密 | 🟢 低 | 非字幕范畴 |
| Font/Alignment/Style 预设 | 字幕样式预设（渲染侧） | 🟢 低 | 字幕库不应管渲染 |
| 2nd language | 生成第二语言字幕 | 🟢 低 | 本质是翻译 + 合并 |

### 2.2 subtitler 有、editingtools.io 无的功能（差异化优势）

| 功能 | 说明 |
|------|------|
| **Pipeline 声明式流水线** | JSON 配置多步操作链 🔥 独特卖点 |
| **质量报告** | CPS/WPM/阅读速度/断行质量 |
| **深度验证** | 时序验证（负时长、零时长、重叠、间隙） |
| **OCR 纠错** | 正则模式修复常见 OCR 错误（0↔o, l↔1） |
| **去除听障标签** | 剥离 (LAUGHS)、[APPLAUSE]、♪ 等 |
| **智能断句** | 基于词组/标点的 `split_long` + `optimize_line_breaks` |
| **Builder API** | 链式调用 + `SubtitleBuilder` |
| **StreamingParser** | 流式解析 |
| **WASM 支持** | 浏览器端运行 |
| **13 格式互转** | 比 editingtools.io 多 5 种格式 |

---

## 3. 缺口严重性分级与推荐优先级

### 🔴 P1 — 应该优先填补（低投入、高回报、AI 生态）

| # | 缺口 | 估算工作量 | 理由 |
|---|------|-----------|------|
| 1 | **AI 翻译真实实现**（OpenAI/DeepL/Whisper 适配器） | 2-3 天 | Trait 已存在，只差 adapter。路线图 v3.0 已列 |
| 2 | **DFXP 格式支持** | 0.5 天 | 与 TTML 几乎同结构（不同 namespace + root），`ttml.rs` 加 ~50 行即可 |
| 3 | **Whisper JSON 导入** | 0.5 天 | 简单 JSON 解析，与 AI 翻译联动 |
| 4 | **去除重复字幕行** | 2 小时 | PipelineOp 新增 + `dedup_by_key` |

### 🟡 P2 — 中等优先级（锦上添花）

| # | 缺口 | 估算工作量 |
|---|------|-----------|
| 5 | 语言字符过滤器（normalize 扩展） | 4 小时 |
| 6 | 短行合并 / 移除换行 / 换行替换（3 个 normalize 选项） | 3 小时 |
| 7 | roll-up → full subtitle 转换 | 1 天 |
| 8 | Spruce STL 格式支持 | 1-2 天 |

### 🟢 P3 — 低优先级（极窄场景）

| # | 缺口 | 理由 |
|---|------|------|
| 9 | AutoDesk / Subtitle Horse / Subtext / FCPXMLD | 过于小众，维护成本 > 收益 |
| 10 | Premiere / DaVinci 专属导出 | 工具专属，不适合通用库 |
| 11 | EDL 格式 | 非字幕范畴 |
| 12 | 批量处理 CLI | 可用 shell `for` 循环替代 |
| 13 | 密码保护导出 | 非字幕范畴 |

---

## 4. 架构级观察

### 4.1 AI 翻译的现状与机会

subtitler 已经在 `src/quality.rs` 定义了干净的 `Translator` trait：

```rust
pub trait Translator: std::fmt::Debug {
    fn translate(&self, text: &str, source_lang: &str, target_lang: &str) -> TranslatorResult;
    fn translate_file(&self, subtitles: &[Subtitle], ...) -> Vec<Subtitle> { ... }
}
```

但只有 `DummyTranslator`（no-op）。editingtools.io 提供的是 "AI - 字幕翻译员"。

**机会**：实现一个 `OpenAiTranslator` / `DeepLTranslator`，这直接对标 editingtools 的核心 AI 功能。路线图 v3.0 已规划此方向。

### 4.2 Whisper JSON 格式的桥接价值

Whisper JSON 格式结构：

```json
{
  "text": "Hello world",
  "segments": [
    {"start": 0.0, "end": 2.0, "text": "Hello world", "id": 0, "seek": 0, ...}
  ]
}
```

这几乎是 `SubtitleFile` 的直接映射。支持此格式可以让 subtitler 成为 Whisper 输出 → 任何字幕格式的管道中间件。

### 4.3 DFXP 的极低投入

DFXP（Distribution Format Exchange Profile）是 W3C TTML 的前身。解析方面几乎就是 TTML 的变体：

- 根元素不同：`<tt:tt xmlns:tt="http://www.w3.org/2006/04/ttaf1">`（DFXP）vs TTML 的命名空间
- 结构几乎一致：`<head>` + `<body>` + `<div>` + `<p begin="..." end="...">`
- subtitler 的 `ttml.rs` 通过 quick-xml 解析，加一个 `detect_format` 判断命名空间即可重写大部分逻辑

---

## 5. 综合建议

### 建议一：立即着手（低投入高回报）

1. **新增 DFXP 格式** → 与 TTML 共用 ~90% 代码，加 ~50 行即可。新增 feature flag `dfxp`。
2. **新增 Whisper JSON 格式** → 简单 JSON 解析，feature flag `whisper`。配合已有的 `Translator` trait 形成 "Whisper → 翻译 → 任意格式" 管线。
3. **新增 `remove_duplicates` PipelineOp** → 高使用频率的基础功能。

### 建议二：下一版本（2.4 或 3.0）

4. **实现 AI 翻译适配器**（OpenAI / DeepL）—— 已有 Trait 底座，只缺 adapter。这在路线图 v3.0 范畴。
5. **Normalize 扩展**：语言过滤器、短行合并、换行替换。

### 建议三：不做

6. 不支持 EDL / FCPXMLD（非字幕）
7. 不支持密码保护（非字幕库职责）
8. 不支持 Font/Style 预设（渲染侧职责）

---

## 6. 与现有路线图的映射

| 本报告缺口 | 路线图对应项 | 状态 |
|-----------|------------|------|
| AI 翻译实现 | v3.0 AI 能力集成 | ⏳ 已规划 |
| DFXP 格式 | 未规划（新发现） | ❌ 需加入 |
| Whisper JSON | 未规划（新发现） | ❌ 需加入 |
| 去除重复行 | 未规划（新发现） | ❌ 需加入 |
| Normalize 扩展 | 未规划（新发现） | ❌ 需加入 |
| 批量处理 CLI | 未规划 | 🟢 低优 |
| 语言过滤器 | 未规划（新发现） | 🟡 中优 |

---

## 7. Quick Summary

> subtitler 在格式广度上已超越 editingtools.io（13 vs 9 共有的），但在 AI 翻译、Whisper 集成、DFXP 支持、去重和语言过滤上有明确差距。前三项（AI 翻译适配器 ≈ 2-3 天、DFXP ≈ 0.5 天、Whisper JSON ≈ 0.5 天）属于低投入、高回报。差异化的 Pipeline 声明式流水线和深度验证是 subtitler 的独特优势，应该继续保持和强化。
