# Subtitler vs EditingTools.io 功能对比

> 对比日期: 2026-07-18
> subtitler 版本: v2.4.0
> editingtools.io 版本: v0.76

---

## 一、格式支持对比

### 1.1 导入格式

| 格式 | Subtitler | EditingTools | 备注 |
|------|-----------|-------------|------|
| SRT (.srt) | ✅ | ✅ | |
| VTT/WebVTT (.vtt/.webvtt) | ✅ | ✅ | |
| ASS (.ass) | ✅ | ✅ | |
| SSA (.ssa) | ✅ | ❌ | |
| MicroDVD (.sub) | ✅ | ❌ | |
| SubViewer (.sub) | ✅ | ✅ (as SUB) | |
| TTML (.ttml/.xml) | ✅ | ✅ | |
| SBV (.sbv) | ✅ | ✅ | |
| LRC (.lrc) | ✅ | ❌ | |
| SAMI (.smi) | ✅ | ❌ | |
| MPL2 (.mpl) | ✅ | ❌ | |
| SCC (.scc) | ✅ | ❌ | |
| EBU STL (.stl) | ✅ | ✅ | |
| DFXP (.dfxp) | ✅ | ✅ | |
| Whisper JSON (.json) | ✅ | ✅ | |
| iTT (.itt) | ❌ (TTML可部分解析) | ✅ | |
| SRTX (.srtx) | ❌ | ✅ | 带说话者名称 |
| CSV/TSV | ❌ | ✅ | |
| EDL | ❌ | ✅ | 剪辑决策表 |
| FCPXML | ❌ | ✅ | Final Cut Pro |
| PTX (ProTools) | ❌ | ✅ | |
| Avid DS SubCap (.txt) | ❌ | ✅ | |
| Subtitle Horse JSON | ❌ | ✅ | |
| 纯文本 (.txt) | ❌ | ✅ | 无时间码 |

**Format count**: Subtitler **15** vs EditingTools **25+** (import)

### 1.2 导出格式

| 格式 | Subtitler | EditingTools |
|------|-----------|-------------|
| SRT | ✅ | ✅ |
| VTT | ✅ | ✅ |
| ASS | ✅ | ✅ |
| SSA | ✅ | ❌ |
| MicroDVD | ✅ | ❌ |
| SubViewer | ✅ | ✅ |
| TTML | ✅ | ✅ |
| SBV | ✅ | ✅ |
| LRC | ✅ | ❌ |
| SAMI | ✅ | ❌ |
| MPL2 | ✅ | ❌ |
| SCC | ✅ | ❌ |
| EBU STL | ✅ | ✅ |
| DFXP | ✅ | ✅ |
| Whisper | ✅ | ❌ |
| PDF | ❌ | ✅ |
| Excel (.xls/.xlsx) | ❌ | ✅ |
| FCPXML | ❌ | ✅ |
| EDL/PTX | ❌ | ✅ |
| JSON (editingtools) | ❌ | ✅ |

---

## 二、功能对比

### 2.1 核心 CLI 命令

| 功能 | Subtitler | EditingTools | 说明 |
|------|-----------|-------------|------|
| `parse` — 解析并展示 | ✅ | ❌ (在线工具) | |
| `convert` — 格式转换 | ✅ | ✅ | |
| `validate` — 定时校验 | ✅ | ❌ (内置于转换) | |
| `detect` — 格式检测 | ✅ | ✅ (自动检测) | |
| `info` — 文件统计信息 | ✅ | ❌ | |
| `quality` — 质量报告 | ✅ | ❌ | |
| `edit` — 编辑操作 | ✅ | ❌ (单独功能) | |
| `pipeline` — 声明式流水线 | ✅ | ❌ | |
| `normalize` — 文本标准化 | ✅ | ✅ | |
| `shift` — 时间偏移 | ✅ | ✅ | |

### 2.2 编辑操作

| 操作 | Subtitler | EditingTools |
|------|-----------|-------------|
| **sort** — 按开始时间排序 | ✅ | ❌ |
| **shift** — 时间偏移 | ✅ | ✅ |
| **merge adjacent** — 合并相邻字幕 | ✅ | ✅ (merge identical) |
| **merge within range** — 合并范围内字幕 | ❌ | ✅ |
| **split long** — 拆分长字幕 | ✅ | ❌ |
| **transform fps** — 帧率转换 | ✅ | ✅ |
| **remove overlaps** — 修复重叠 | ✅ | ✅ |
| **enforce min duration** — 最小时长 | ✅ | ✅ |
| **enforce max duration** — 最大时长 | ✅ | ✅ |
| **auto extend for CPS** — 自动延长满足 CPS | ✅ | ❌ |
| **extract range** — 提取时间段 | ✅ | ❌ |
| **concatenate** — 拼接文件 | ✅ | ❌ |
| **map/filter** — 函数式变换 | ✅ | ❌ |
| **remove duplicates** — 去重 | ✅ | ✅ (remove repeating) |

### 2.3 文本标准化

| 操作 | Subtitler | EditingTools |
|------|-----------|-------------|
| **normalize whitespace** — 空白规范化 | ✅ | ✅ (superfluous spaces) |
| **normalize quotes** — 智能引号转 ASCII | ✅ | ❌ |
| **normalize punctuation** — 标点修复 | ✅ | ❌ |
| **fix OCR errors** — OCR 错误修复 | ✅ | ❌ |
| **strip hearing-impaired** — 删除听力障碍标签 | ✅ | ❌ |
| **optimize line breaks** — 优化断行 | ✅ | ❌ |
| **remove all newlines** | ✅ | ✅ |
| **replace newlines with separator** | ✅ | ✅ (`|` 符号) |
| **filter language** — 按语言过滤字符 | ✅ | ✅ |
| **remove tags** — 删除标签 | ✅ | ✅ |
| **remove text between brackets** | ❌ | ✅ ([ ] < > { } ( )) |
| **remove speaker names** | ❌ | ✅ |
| **remove timecodes** | ❌ | ✅ |
| **fix misinterpreted out times** | ❌ | ✅ (±1帧) |
| **caps lock to normal** | ❌ | ✅ |
| **fix invalid tags** | ❌ | ✅ |
| **spacing of opening hyphen** | ❌ | ✅ |
| **transliterate Cyrillic** | ❌ | ✅ |
| **clear all subtitles** (保留时间码) | ❌ | ✅ |

### 2.4 校验/质量分析

| 功能 | Subtitler | EditingTools |
|------|-----------|-------------|
| **NegativeDuration** — 负时长 | ✅ | ❌ |
| **ZeroDuration** — 零时长 | ✅ | ✅ (移除0帧) |
| **Overlap** — 重叠 | ✅ | ✅ (修复) |
| **DecreasingStartTime** — 时间递减 | ✅ | ❌ |
| **TextTooLong** — 文本过长 | ✅ | ❌ |
| **CpsTooHigh** — 字符/秒过高 | ✅ | ✅ (CPS计算) |
| **TooLongGap** — 间隙过大 | ✅ | ❌ |
| **poor_line_break** — 断行不当 | ✅ | ❌ |
| **Guideline profiles** (Netflix/BBC/ARD) | ❌ | ✅ (5个预设) |
| **字数/词数/时长统计** | ✅ | ✅ |
| **CPS/WPM 逐字幕分析** | ✅ | ✅ |
| **JSON 输出** | ✅ | ❌ |

### 2.5 流水线 / 批处理

| 功能 | Subtitler | EditingTools |
|------|-----------|-------------|
| JSON config 声明式流水线 | ✅ | ❌ |
| 编程 API (Rust lib) | ✅ | ❌ |
| WASM (浏览器内) | ✅ | ❌ |
| 批量上传 | ❌ | ✅ (PRO: 15文件/ZIP) |
| API 接口 | ❌ (纯库) | ✅ (REST API) |

### 2.6 高级功能

| 功能 | Subtitler | EditingTools |
|------|-----------|-------------|
| **EDL/XML shot change rules** | ❌ | ✅ |
| **AI 翻译** | ❌ (DummyTranslator 只有接口) | ✅ (20+语言) |
| **密码保护导出** | ❌ | ✅ (PRO) |
| **FCPXML 定制** | ❌ | ✅ |
| **音频拉变 (pull up/down)** | ❌ | ✅ |
| **帧率 Drop-Frame 支持** | ✅ (SCC) | ✅ |
| **编码检测** | ✅ (chardetng) | ❌ |
| **流式解析** | ✅ | ❌ |
| **proptest 属性测试** | ✅ | ❌ |
| **基准测试 (criterion)** | ✅ | ❌ |

---

## 三、架构对比

| 维度 | Subtitler | EditingTools |
|------|-----------|-------------|
| **类型** | Rust 库 + CLI | Web 在线工具 |
| **部署方式** | `cargo install` / crates.io | Browser + REST API |
| **开源** | ✅ (GitHub) | ❌ (闭源 SaaS) |
| **离线可用** | ✅ | ❌ (需联网) |
| **编程集成** | ✅ (Rust crate) | ✅ (REST API) |
| **性能** | 本地编译执行，极快 | 依赖服务器+网络 |
| **大文件处理** | 无限制 (内存) | 1000 字幕限制 (免费) |
| **批量处理** | 脚本循环 | PRO 界面 |
| **价格** | 免费 | 免费/PRO 订阅 |

---

## 四、Subtitler 独有优势

1. **离线本地执行** — 无网络依赖，无文件上传隐私风险
2. **编程 API** — Rust 库可嵌入到任何应用
3. **WASM 支持** — 浏览器中直接运行解析
4. **流水线 (Pipeline)** — JSON 声明式多步操作，可复现
5. **流式解析** — 内存友好的大文件处理
6. **属性测试 (proptest)** — 模糊测试保证格式稳定性
7. **基准测试** — 性能分析工具
8. **15 种格式覆盖** — 包含 LRC, SCC, MicroDVD 等 niche 格式
9. **转码精度** — 时间戳统一用 `u64` 毫秒，无浮点误差
10. **UTF-16/GBK/Shift_JIS 编码** — 原生支持东亚编码
11. **分段替换断行优化** — 语义断行算法

## 五、EditingTools 独有优势

1. **更多导入/导出格式** — 25+ 格式，含 CSV/EDL/FCPXML/PDF/XLS
2. **Guideline 预设** — Netflix/BBC/ARD/ORF/SRF/ZDF/TED 一键检查
3. **AI 翻译** — 深度学习翻译 20+ 语言
4. **EDL/XML 镜头剪切规则** — 专业后期工作流集成
5. **更多文本处理** — 括号删除、说者名去除、标签修复、西里尔转写
6. **更多定时处理** — 合并范围内字幕、修复出场时间
7. **导出多样化** — PDF/Excel/FCPXML 等专业格式
8. **PRO 批量处理** — 界面上传 15 文件 / ZIP 1000 文件
9. **密码保护导出**
10. **REST API** — Zapier/Make 自动化集成

## 六、总结

| 维度 | Subtitler 优势 | EditingTools 优势 |
|------|---------------|-------------------|
| **格式** | 15 种，含 niche 格式 | 25+ 种，含专业后期格式 |
| **功能深度** | 编程 API、流水线、流式解析、WASM | EDL 镜头规则、Guideline 预设、AI 翻译 |
| **使用场景** | CLI/编程集成、离线批处理、CI/CD | 在线转换、非技术用户、专业后期 |
| **运行环境** | 本地 (macOS/Linux/Windows/WASM) | 浏览器 + REST API |
| **目标用户** | 开发者、技术运维 | 剪辑师、字幕翻译、内容创作者 |
| **价格** | 完全免费 (开源) | 免费有上限 / PRO 订阅 |

**结论**: Subtitler 适合 **程序化、离线、批处理** 场景；EditingTools 适合 **交互式、在线、专业后期** 场景。两者互补而非替代。
