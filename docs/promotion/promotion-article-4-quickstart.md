# subtitler 5分钟快速上手指南

> 从安装到实战，带你快速掌握 Rust 字幕处理库

如果你正在寻找一个简单易用、功能完整的 Rust 字幕处理库，subtitler 就是你的最佳选择。本文将用最短时间，带你从零开始掌握 subtitler 的核心功能。

## 📦 第一步：安装（30秒）

### 方式一：Cargo 命令行（推荐）

```bash
cargo add subtitler
```

### 方式二：修改 Cargo.toml

```toml
[dependencies]
subtitler = "1.2"
```

### 最小化安装

如果只需要特定格式：

```toml
[dependencies]
subtitler = { version = "1.2", default-features = false, features = ["srt", "vtt"] }
```

## 🎯 第二步：基础解析（1分钟）

### 解析本地文件

```rust
use subtitler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 自动检测格式并解析
    let file = subtitler::parse_file("movie.srt").await?;

    // 查看基本信息
    println!("格式: {:?}", file.format());
    println!("字幕数量: {}", file.subtitles().len());

    // 查看第一条字幕
    if let Some(first) = file.subtitles().first() {
        println!("第一条: {}", first.text);
    }

    Ok(())
}
```

### 解析字符串内容

```rust
use subtitler::srt;

let content = r#"1
00:00:01,000 --> 00:00:03,500
Hello, world!

2
00:00:04,000 --> 00:00:06,500
Welcome to subtitler!
"#;

let subtitles = srt::parse_content(content)?;
println!("解析到 {} 条字幕", subtitles.len());
```

### 从 URL 解析

```rust
use subtitler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = subtitler::parse_url("https://example.com/subtitle.srt").await?;
    println!("从 URL 解析到 {} 条字幕", file.subtitles().len());
    Ok(())
}
```

## 🔧 第三步：基本编辑（1分钟）

### 时间轴调整

```rust
use subtitler;

let mut file = subtitler::parse_file("movie.srt").await?;

// 延迟 5 秒
file.shift_all(5000);

// 提前 2 秒（负数）
file.shift_all(-2000);
```

### 合并相邻字幕

```rust
// 合并间隔小于 200ms 的相邻字幕
file.merge_adjacent(200, 50);
```

### 拆分长字幕

```rust
// 拆分超过 5 秒的字幕
file.split_long(5000, 1000);
```

### 过滤字幕

```rust
// 只保留前 100 条
file.filter(|sub| sub.index.unwrap_or(0) <= 100);
```

## 💾 第四步：保存导出（30秒）

### 导出为字符串

```rust
use subtitler::srt;

let content = srt::to_string(file.subtitles());
std::fs::write("output.srt", content)?;
```

### 流式写入（v1.2 新增）

```rust
use subtitler::srt::write_stream;
use tokio::fs::File;
use tokio::io::BufWriter;

let file = File::create("output.srt").await?;
let mut writer = BufWriter::new(file);
write_stream(file.subtitles(), &mut writer).await?;
```

### 格式转换

```rust
use subtitler;

// 解析 SRT
let file = subtitler::parse_file("movie.srt").await?;

// 转换为 VTT
let vtt_content = subtitler::vtt::to_string(file.subtitles(), None);
std::fs::write("movie.vtt", vtt_content)?;

// 转换为 ASS
let info = std::collections::HashMap::new();
let styles = vec![];
let ass_content = subtitler::ass::to_string(&info, &styles, file.subtitles());
std::fs::write("movie.ass", ass_content)?;
```

## 🎓 第五步：质量检查（1分钟）

### 基础验证

```rust
// 检查重叠
let issues = file.validate();
if !issues.is_empty() {
    println!("发现 {} 个问题", issues.len());
}
```

### 扩展验证

```rust
// 检查更多问题（时长、显示速度等）
let issues = file.validate_extended(20.0, 1000);

for issue in &issues {
    println!("问题: {:?}", issue);
}
```

### 自动修复

```rust
// 修复短时长字幕（最小 1 秒）
file.enforce_min_duration(1000);

// 修复重叠
file.remove_overlaps();

// 调整显示速度（最大 20 字符/秒）
file.auto_extend_for_cps(20.0, 1000);
```

## 🚀 第六步：完整示例（1分钟）

### 字幕时间轴调整工具

```rust
use clap::Parser;
use subtitler;
use anyhow::Result;

#[derive(Parser)]
struct Args {
    /// 输入文件
    #[arg(short, long)]
    input: String,

    /// 输出文件
    #[arg(short, long)]
    output: String,

    /// 时间偏移（毫秒）
    #[arg(short, long, default_value = "0")]
    shift: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // 读取并解析
    let mut file = subtitler::parse_file(&args.input).await?;

    // 调整时间轴
    if args.shift != 0 {
        file.shift_all(args.shift);
        println!("时间轴调整: {}ms", args.shift);
    }

    // 保存
    std::fs::write(&args.output, file.to_string())?;
    println!("保存到: {}", args.output);

    Ok(())
}
```

使用：

```bash
# 编译
cargo build --release

# 运行（延迟 5 秒）
./subtitle-tool -i movie.srt -o movie_fixed.srt -s 5000
```

## 📚 进阶用法

### 流式解析大文件

```rust
use subtitler::srt::SrtStream;
use std::fs::File;
use std::io::BufReader;

let file = File::open("large.srt")?;
let content = std::io::read_to_string(BufReader::new(file))?;

let stream = SrtStream::new(&content);
for result in stream.take(100) {
    let subtitle = result?;
    println!("{}", subtitle.text);
}
```

### 批量处理

```rust
use futures::stream::{self, StreamExt};

let files = vec!["1.srt", "2.srt", "3.srt"];

let results: Vec<_> = stream::iter(files)
    .map(|path| async move {
        let file = subtitler::parse_file(path).await?;
        anyhow::Ok((path, file.subtitles().len()))
    })
    .buffer_unordered(10)
    .collect()
    .await;

for result in results {
    if let Ok((path, count)) = result {
        println!("{}: {} 条字幕", path, count);
    }
}
```

### 自定义配置解析

```rust
use subtitler::model::ParseConfig;

let config = ParseConfig {
    preserve_indices: true,
    lenient_mode: true,
    ..Default::default()
};

let file = subtitler::parse_file_with_config("movie.srt", config).await?;
```

## 🎯 常见问题

### Q: 如何检测文件编码？

A: subtitler 内置了 `chardetng` 自动检测编码：

```rust
// 自动检测并解码
let file = subtitler::parse_file("gbk_subtitle.srt").await?;
```

### Q: 如何处理 ASS 的样式？

A: ASS 格式包含完整的样式信息：

```rust
use subtitler::ass;

let file = subtitler::parse_file("styled.ass").await?;
if let subtitler::SubtitleFile::Ass { data, .. } = file {
    println!("样式数量: {}", data.styles.len());
    for style in &data.styles {
        println!("样式: {} ({})", style.name, style.fontname);
    }
}
```

### Q: 如何提取纯文本？

A: 使用 `normalize` 模块：

```rust
use subtitler::normalize;

let plain = normalize::plaintext("<b>Hello</b> <i>world</i>!");
println!("{}", plain); // Hello world!
```

### Q: 如何获取字幕质量报告？

A: 使用 `quality` 模块：

```rust
use subtitler::quality;

let report = quality::QualityReport::generate(&file)?;
let json = serde_json::to_string(&report)?;
println!("{}", json);
```

## 📖 下一步

- 📚 阅读完整文档：https://docs.rs/subtitler
- 💻 查看示例代码：https://github.com/subtitle-rs/subtitler/tree/main/examples
- 🐛 报告问题：https://github.com/subtitle-rs/subtitler/issues

---

**恭喜！你已经掌握了 subtitler 的核心用法！**

现在就开始在你的项目中使用 subtitler 吧：

```bash
cargo add subtitler
```

Happy coding! 🎉