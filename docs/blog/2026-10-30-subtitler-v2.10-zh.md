# subtitler v2.10 发布：3 个社区 PR、一个一天修掉的 issue，和关于 uuencode 的真相

> [subtitler](https://github.com/subtitle-rs/subtitler) 是一个解析、转换、校验和生成字幕的 Rust 库 + CLI，支持 17 种字幕格式。这次一起发布两个版本：**v2.9.0** 收编了贡献者 localcc 的三个 PR（嵌入式字体、样式转发、定位发射），**v2.10.0** 修掉了一个真实用户报告的转换问题，并把 CLI 依赖变成了可选项。测试数从 417 涨到 492。

## 先试试在线版

如果不想装任何东西：[**Subtitle Workbench**](https://subtitle-rs.github.io/subtitle-format-conversion/) —— 把 subtitler 编译成 WebAssembly 做的在线字幕工具台。拖一个字幕文件进去就能转换（自动检测源格式）、做质量校验、文本规范化和信息查看。**100% 浏览器内运行，没有后端，文件不离开你的设备。**

## v2.9.0：把三个 stacked PR 收编成一次发布

字幕圈的硬骨头大多在 ASS/SSA 上。贡献者 [localcc](https://github.com/localcc) 一个夏天提交了三个层层叠叠的 PR：

1. **ASS `[Fonts]` 嵌入字体** —— 解析和回写字体块，新增公开的 `AssData.fonts`；
2. **ASS → TTML 样式转发** —— `Subtitle.style_props`（`StyleProps`），TTML 侧输出 `<styling>` 块、解析继承链，顺带修了 VTT 实体转义；
3. **Cue 定位** —— `\pos` / `\move` / `\an` 解析成格式无关的 `CuePosition`，TTML 侧用 `<layout>` region 发射和读回，交叉淡化自动去重叠。

三个 PR 在 8 月收到详细 review 后改动完成，但一直没等到 re-review。这次我们逐条验证了所有 review 意见后整栈落地（原始 commit 原样保留、作者署名），并顺手补上了两个 review 遗留的 blocking 项：`write_stream` 与 `to_string` 管线一致性（两个入口输出逐字节相同，有回归测试钉住），以及 region id 与样式 id 的防碰撞。

### 一个关于 uuencode 的真相

review 时我们曾把"uuencode 必须带每行长度前缀"列为 blocking——**这个前提是错的**。拉了 Aegisub 的 `libaegisub/ass/uuencode.cpp` 和 libass 的 `ass.c` 逐行对质：ASS 的字体编码是 3 字节 → 4 字符、偏移 33、80 字符/行、无前缀，两个参考实现互相印证。更妙的是，把 PR 的实现与两个参考实现做了 0..=400 全长度、双向、含 CRLF 换行的逐字节对拍——**完全一致**。当年 review 里那条"标准编码应该是 `$!"##!&`"的校验向量，本身就是手算错的。

这件事的教训固化进了仓库的开发手册（AGENTS.md）：**review 结论要用一手源码验证，数值预期要用脚本独立验证**。

### 附赠的正确性修复

- `[Fonts]` 段内改用 libass 同款"已知 section 头白名单"——uuencode 字母表里含 `[` 和 `]`，真实字体数据可能恰好排出 `[!!]` 形状的行，通用判定会把字体块截断；
- ASS 浮点 `Style:` 行不再被静默丢弃；override 标签不再泄漏进转换结果；
- VTT 读写补上 `&`/`<`/`>` 实体转义（proptest 抓到的）。

## v2.10.0：一个 issue 的 24 小时

10 月 29 日，用户 [danatationn](https://github.com/danatationn) 报告：Apple Music 风格的 TTML 歌词转 LRC 时，背景人声和主唱粘在一起——`Ahahahaha(Oh, you like that?)`。

诊断结果比表面有意思：解析器**并不丢空格**（源里有空格的样例输出正常）——是源文件里主唱和 `<span ttm:role="x-bg">` 之间本来就没有空格字符，扁平化时缺的是"换声分隔"这个语义。修复落在解析层：嵌套感知地跟踪 `x-bg` 区域，贴合处插入一个空格；行内样式 span 完全不受影响。**报告次日，v2.10.0 发布**，报告者的原文件输出 `[00:55.54]Ahahahaha (Oh, you like that?)`。

同一个版本里还合入了 [huou07](https://github.com/huou07) 的 PR：`clap` 和 `tracing-subscriber` 移到 opt-in 的 `cli` feature（仍在 default 里，`cargo install subtitler` 不变）。只当库用的消费者关掉 default features，依赖图直接少 22 个包。

## 质量是怎么攒出来的

- **492 个测试**（单元 315 + 集成 177），每个 bug 修复必须带能复现它的回归测试；
- CI 矩阵：fmt、`clippy --all-targets -D warnings`、双 feature 组合测试、WASM 构建、MSRV 1.85、examples；
- 每个发布前跑最小构建 ×5（`--no-default-features` 配不同格式组合），`cargo +1.85 build` 兜底 MSRV；
- 时间码、帧数这类数值预期，一律先用脚本独立验证再写进测试。

## 上手

```toml
[dependencies]
subtitler = "2.10"
```

```rust
use subtitler::{parse_bytes, Format, SubtitleFormat};

// 解析任意格式（内容签名自动检测，bytes 进 bytes 出）
let file = parse_bytes(&raw)?;

// 转换成任意其他格式
let lrc = file.to_string_with_format(&Format::Lrc);
```

> `parse_file` / `parse_url`（异步）同样自动检测；`SubtitleFormat` trait 提供了
> `subtitles()` / `validate()` 等方法，记得 bring into scope。

CLI 一把梭：

```sh
cargo install subtitler
subtitler convert input.ass output.srt        # 自动检测 + 转换
subtitler validate input.srt --max-cps 20     # 广播级 QC 校验
subtitler pipeline in.ass out.srt --config ops.json   # 声明式流水线
```

## 链接

- GitHub：https://github.com/subtitle-rs/subtitler
- crates.io：https://crates.io/crates/subtitler
- docs.rs：https://docs.rs/subtitler
- 在线工具：https://subtitle-rs.github.io/subtitle-format-conversion/
- v2.9.0 / v2.10.0 完整变更：[CHANGELOG](https://github.com/subtitle-rs/subtitler/blob/main/CHANGELOG.md)

localcc 和 huou07 的 PR 从提交到落地走完了完整周期，danatationn 的 issue 一天内修完发版——**这是想告诉大家：这里的 issue 和 PR 是真的有人看、真的会合**。欢迎来提 issue、提 PR，或者就 star 一个。
