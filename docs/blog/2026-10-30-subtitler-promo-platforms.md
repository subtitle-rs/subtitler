# 字幕看起来永远是对的，直到播放器把它渲染出来

> 我把字幕交付前那些本该人工检查的事——格式互转、Netflix 规范质检、镜头切点守卫、OCR 清理——做成了一个开源工具：`subtitler`，Rust 库 + 命令行，17 种格式，另有免安装的在线版。这篇文章是它解决的问题，以及几个真实踩坑记录。

做过字幕交付的人都有这种感觉：文件在编辑器里看着一切正常，直到送到平台被打回、直到播放器里字体没加载、直到歌词的背景人声和主唱粘成一个词。字幕的坑几乎都不在肉眼可见的地方。

下面这些都是真事，来自这个项目近两年的迭代记录。

## 一条 43 个字符的字幕，和一套能跑的质检

Netflix 的交付规范里，单行 42 字符、阅读速度 20 CPS、时长 833–7000ms、最小间隔 83ms。人工核对这些数字，五分钟就会开始眼花。

`subtitler` 把它们做成了一条命令。拿一个我故意写坏的文件（长行、重叠、超短时长、超高阅读速度）：

```console
$ subtitler validate demo.srt --guideline netflix
Guideline preset: Netflix (max 42 chars/line, 2 lines, 833–7000ms duration, ≥83ms gap, ≤20 CPS)
Found 7 issues in 5 subtitles:

  [OVERLAP] subtitle 0 (ends at 3500ms) overlaps with subtitle 1 (starts at 3400ms)
  [LONG_TEXT] subtitle 0 has 43 characters (max recommended: 42)
  [SHORT_DUR] subtitle 1 lasts 800ms (minimum required: 833ms)
  [SHORT_DUR] subtitle 2 lasts 300ms (minimum required: 833ms)
  [SHORT_DUR] subtitle 3 lasts 200ms (minimum required: 833ms)
  [HIGH_CPS] subtitle 3 has 70.0 chars/second (max recommended: 20.0)
  [LONG_TEXT] subtitle 4 has 52 characters (max recommended: 42)
```

有问题时退出码非 0——所以它能直接挂进 CI。预设除了 Netflix，还有 BBC、TED、ARD-ORF-SRF-ZDF 和 Channel 4，都是按各平台公开的规范写的。

## 镜头切点：字幕横跨转场，是会被打回的

剪辑软件能导出 EDL 剪辑表（CMX3600），`subtitler` 能读它。给定“切点前留 2 帧、切点后留 12 帧”的守卫规则（Netflix 风格），一条横跨切点的字幕会被自动修剪到安全区：

```srt
# 修复前
1
00:00:01,500 --> 00:00:02,500
这条字幕横跨了镜头切点

2
00:00:02,100 --> 00:00:03,000
这条在切点之后起头

# 修复后
1
00:00:01,500 --> 00:00:01,920
这条字幕横跨了镜头切点

2
00:00:02,480 --> 00:00:03,000
这条在切点之后起头
```

切点在 2.000s：第一条收在切点前 80ms（2 帧），第二条从切点后 480ms（12 帧）起。跨切的字幕保留内容更多的一侧，完全落在守卫区里的小字幕不会被拦腰截断——这些规则都有测试钉着。

## 十几种格式，和“看起来像 AI 转写”的输入

今天做视频的人手里最多的字幕其实不是 SRT，而是 Whisper 这类转写工具输出的 JSON，带着词级时间戳。`convert` 自动识别源格式，不用猜：

```console
$ subtitler convert transcript.json transcript.srt
Converted: transcript.json -> transcript.srt (srt)
```

支持的 17 种格式：SRT、WebVTT、ASS/SSA、MicroDVD、SubViewer、TTML/IMSC、SBV、LRC、SAMI、MPL2、SCC、EBU STL、Spruce STL、DFXP、iTT、Whisper JSON。ASS 的嵌入字体 `[Fonts]` 能解析也能回写，`\pos`/`\an` 定位会映射进 TTML 的 layout region；SCC 的 drop-frame 时间码按 SMPTE 规范算——这些细节决定了转换是“能用”还是“送去播出会被打回”。

## 不想装东西？浏览器里直接换

[**Subtitle Workbench**](https://subtitle-rs.github.io/subtitle-format-conversion/) 是把同一个库编译成 WebAssembly 的在线版：拖入文件即可转换、质检、规范化。**没有后端，文件不离开你的浏览器**。目前覆盖 13 种主流格式，正在补齐全部 17 种。

## 给开发者的部分

```rust
use subtitler::{parse_bytes, Format, SubtitleFormat};

let file = parse_bytes(&raw)?;                        // 任意格式，自动检测
let lrc = file.to_string_with_format(&Format::Lrc);   // 转成任意目标格式
```

Rust 库 + CLI 是这个项目的一等公民：17 种格式各自一个 feature flag（只做 SRT 的设备可以编译出个位数依赖的精简版，连 CLI 依赖都是可选的），时间戳统一毫秒整数，async I/O，WASM target 可编译。crates.io 上 `cargo add subtitler` 即可。

## 为什么值得信：两个真实的踩坑记录

**其一**。有人报告 Apple Music 风格的 TTML 歌词转 LRC 时，背景人声和主唱粘在一起。第一反应是“解析器丢空格”，但复现后发现源文件里本来就没有空格——真正缺的是“换声分隔”。修复落在解析层：识别 `ttm:role="x-bg"` 区域，贴合处补一个空格，行内样式完全不受影响。**报告次日发布修复版本**。

**其二**。审查 ASS 字体功能时，我们一度认为它的 uuencode 编码“必须带每行长度前缀”。拉来 Aegisub 和 libass 的一手源码逐行对质后发现：ASS 格式根本没有前缀，两个参考实现互相印证。最后把实现和两个参考实现做了 0 到 400 字节全长度、双向的逐字节对拍——完全一致。

这两件事解释了项目的脾气：**结论用一手源码验证，数值用脚本验证，每个 bug 修复必须附带能复现它的回归测试。** 现在全库 492 个测试，CI 跑 fmt、clippy 零警告、双 feature 矩阵、WASM 构建和 MSRV 1.85。

## 它不是什么

不是 GUI 字幕编辑器——没有时间轴拖拽和所见即所得。它的定位是**自动化和批处理**：把重复、易错、需要逐条核对的活交给命令和流水线，人去做翻译和润色。在线版也有同样定位。

## 链接

- GitHub（欢迎 Star / Issue / PR）：https://github.com/subtitle-rs/subtitler
- crates.io：https://crates.io/crates/subtitler ｜ 文档：https://docs.rs/subtitler
- 在线版（免安装，文件不上传）：https://subtitle-rs.github.io/subtitle-format-conversion/
- 更新日志：https://github.com/subtitle-rs/subtitler/blob/main/CHANGELOG.md

字幕看起来永远是对的——所以让它经得起播放器和平台的检验，才值得写这些工具。
