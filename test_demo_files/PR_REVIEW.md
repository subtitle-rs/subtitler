# Incoming PR 审查报告

> 审查日期: 2026-08-18
> 仓库: subtitle-rs/subtitler
> 贡献者: localcc
> 当前版本: 2.6.1
> 审查方式: diff 逐行审查 + 两个独立审查代理 + 交叉验证

---

## 一、PR 概览

| PR | 标题 | 规模 | 依赖 | 结论 |
|----|------|------|------|------|
| #2 | feat(ass): add font parsing | +239/-7, 9文件 | — | ⛔ **需要修改** |
| #3 | Better ttml style forwarding from ass | +833/-77, 15文件 | 依赖 #2 | ⛔ **需要修改** |
| #4 | Ass position emission | +2233/-116, 16文件 | 依赖 #2 #3 | ⛔ **需要修改** |

三个 PR 为栈式依赖，全部基于当前 main (8241748)，无合并冲突。

---

## 二、阻塞性问题（必须修复后才能合并）

### 🔴 B1: uuencode 模块与真实 ASS 嵌入字体格式不兼容 (三个 PR 共有)

`src/ass.rs` 新增的 `uuencode` 模块使用 **2字节→3字符** 的编码方案，但真实的 ASS/Aegisub/libass 嵌入字体格式（`libaegisub/ass/uuencode.cpp`）是 **3字节→4字符**，且每行带长度前缀字符。

**后果**：
- 用 subtitler 写出的 `[Fonts]` 无法被 libass/Aegisub 解码 → 播放器静默丢弃或损坏字体
- 解析真实 Aegisub 文件时解码出乱码（静默，不报错）

**交叉验证**：真实编码 `[0,16,130,24]` 应为 `$!"##!&`，本 PR 解码器返回 `[12,0,8,32]`。

**现有测试缺陷**：测试只验证自洽性（round-trip 自己编自己解），从未对照真实 Aegisub 样例。

### 🔴 B2: TTML write_stream/write_stream_async 绕过新管线 (PR #4)

只有 `to_string` 走了新加入的 `filter_for_output` + `assign_regions` + 去重 + deoverlap + 样式发射管线。流式路径：
- 对 `\p1` 绘图 cue 仍输出空 `<p></p>`
- 不处理交叉淡入淡出重叠
- 静默丢弃 `style_props` / `position`

违反 AGENTS.md §3.8（`write_stream_async` 是 TTML 推荐入口）。CHANGELOG 的 "Fixed" 声明只对 `to_string` 成立。

### 🔴 B3: 破坏性公共 API 变更未更新文档 (PR #3 #4)

| 变更 | 影响 |
|------|------|
| `Subtitle` 新增公开字段 `style_props`, `position` | 下游结构体字面量构造断裂 |
| `AssData` 新增公开字段 `fonts` | `SubtitleFile::Ass(AssData { .. })` 断裂 |
| `ass::to_string` 签名加 `fonts` 参数 | 所有调用方断裂 |
| `ass::write_stream` 签名加 `fonts` 参数 | 公开 API 断裂 |

- **MIGRATION.md 未更新**（违反 §6.7/§7.2，发版阻塞项）
- README.md L161/L241 仍记录旧的 `Subtitle` 结构和 `to_string` 签名
- CHANGELOG 未标记 breaking change，也未提及 ASS `[Fonts]` 功能

---

## 三、次要问题（建议修复）

| # | 问题 | 位置 | 严重性 |
|---|------|------|--------|
| C1 | `StyleProps::merge_from` 用 `\|=` 合并 bold/italic，子样式 `fontWeight="normal"` 无法取消继承 | model/subtitle.rs | 中 |
| C2 | `resolve_style` visited-set 从不 pop，菱形继承（diamond）会丢失属性 | ttml.rs | 低 |
| C3 | 样式去重复用第一个 sub 的 id，相同属性不同样式名的 sub 经过 round-trip 后样式名丢失 | ttml.rs | 中 |
| C4 | ASS 样式名含空格时 `xml:id="My Style"` 是非法 XML | ttml.rs | 低 |
| C5 | `[Fonts]` 段放在 `[Events]` 之前（Aegisub 放末尾） | ass.rs | 低 |
| C6 | style xml:id 与 region xml:id（`r_top`/`r_mid`/`r_bot`/`posN`）可能冲突，输出重复 id | ttml.rs | 中 |
| C7 | band cue（x/y=None）round-trip 后变成显式锚点，`position` 相等性不保留 | ttml.rs | 低 |
| C8 | uuencoded 行首尾为 `[` `]` 时可能被误认为 section header（概率极低） | ass.rs | 极低 |

---

## 四、做得好的地方

- **位置数学完全正确** — `ass_alignment_to_align`、`\pos`→百分比、anchor 计算经独立验证，测试数字手算正确
- **测试质量高** — 位置测试标注了 Python 交叉验证（符合 §3.12），样式/去重/区域/实体测试都有针对性
- **serde 向后兼容** — 新增字段都带 `default` + `skip_serializing_if`
- **特性门控正确** — 新类型不门控（格式中立），ass/ttml 模块正确门控
- **clippy/fmt/MSRV 全通过** — 验证在干净 checkout 上 `cargo clippy --all-targets -D warnings`、`cargo fmt --check`、`cargo test --all-targets` 全过，最小构建（srt / dfxp,ttml）也过
- **VTT 转义顺序正确** — `&`→`&amp;` 先转义、`&lt;`/`&gt;` 先反转义，嵌套情况处理正确

---

## 五、处理建议

**方案 A（推荐）— 要求贡献者修改后再合并：**
1. 在 #2 上留言指出 uuencode 格式不兼容（附真实 Aegisub 对照编码 `$!"##!&`），要求改回 3字节→4字符 + 长度前缀方案，并加真实样例测试
2. 要求 #4 修复 write_stream 绕过管线问题
3. 要求三个 PR 补 MIGRATION.md + README + CHANGELOG 的 breaking 变更说明
4. 顺手修 C1/C3/C6

**方案 B — 只接受核心逻辑，暂缓字体功能：**
- 如果不想等 uuencode 修复，可让贡献者去掉 `[Fonts]` 解析/发射（或改为"仅保留不解析"），先合并 TTML 样式转发 + ASS 位置（这两部分是纯增量、无格式兼容风险）
- 减少 API 面破坏（去掉 `fonts` 参数和 `AssFont`）

**不建议直接合并**：B1 会让库写出不兼容的字体数据，属于 P1 级互操作 bug；B2 违反自身 AGENTS 约定；B3 违反发版规范。

---

## 六、回复模板（可用于 PR 留言）

> Thanks for this substantial work — the style-forwarding core and position math are solid and well-tested. Before we can merge, there are a few blocking items:
>
> 1. **ASS font uuencode is incompatible with the real format.** Aegisub/libass encode 3 bytes → 4 chars (6-bit values + 33, with a per-line length prefix), but this PR packs 2 bytes → 3 chars. Real `.ass` files with fonts will decode to garbage, and fonts we emit won't load in players. Please switch to the spec-compliant scheme and add a test against a real Aegisub-produced sample.
> 2. **`write_stream`/`write_stream_async` bypass the new pipeline** — output diverges from `to_string` (empty `<p></p>` for drawing cues, no de-overlap, dropped style_props/position).
> 3. **Breaking public API changes need MIGRATION.md + README + CHANGELOG entries** (new `Subtitle`/`AssData` fields, `ass::to_string`/`write_stream` signature changes).
> 4. Also: style `xml:id` can collide with region ids (`r_top`/`r_mid`/`r_bot`/`posN`), and `merge_from`'s `|=` means `fontWeight="normal"` can't unset inherited bold.
>
> The position math itself verified clean (anchor round-trips, PlayRes percent conversion). Happy to re-review once these are addressed.