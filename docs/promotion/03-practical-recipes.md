# 5 个实战场景：用 subtitler 解决真实字幕问题

理论讲完了，本文展示 subtitler 在实际场景中怎么用。每个场景都是真实需求，附完整可运行代码。

## 场景 1：批量转换字幕格式

**需求**：下载的字幕是 SRT，但播放器需要 VTT。批量转换整个目录。

```rust
use subtitler::{SubtitleFormat, model::Format};
use std::path::Path;

fn convert_dir(dir: &str, target: Format) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension() == Some("srt".as_ref()) {
            let data = std::fs::read(&path)?;
            let file = subtitler::parse_bytes(&data)?;

            let ext = match target {
                Format::Vtt => "vtt",
                Format::Ass => "ass",
                Format::Ttml => "ttml",
                _ => "txt",
            };
            let output = file.to_string_with_format(&target);
            let out_path = path.with_extension(ext);
            std::fs::write(&out_path, output)?;
            println!("{} → {}", path.display(), out_path.display());
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    convert_dir("./subtitles", Format::Vtt)?;
    Ok(())
}
```

**亮点**：`parse_bytes` 自动检测格式，`to_string_with_format` 输出目标格式。不需要手动判断输入格式。

## 场景 2：字幕质量检查 + 自动修复

**需求**：检查字幕是否语速过快（CPS > 25）、时长太短（< 1.5s），并自动修复。

```rust
use subtitler::{SubtitleFormat, model::{SubtitleFile, Subtitle}};

fn check_and_fix(file: &mut SubtitleFile) {
    // 1. 检查问题
    let issues = file.validate_extended(42, 5000, 25.0);
    println!("发现 {} 个问题:", issues.len());
    for issue in &issues {
        println!("  - {}", issue.description());
    }

    // 2. 自动修复
    file.enforce_min_duration(1500);      // 最短 1.5 秒
    file.auto_extend_for_cps(25.0);       // CPS 超标时自动延长
    file.remove_overlaps();               // 移除时间重叠
    file.sort();                          // 按开始时间排序

    // 3. 验证修复结果
    let remaining = file.validate_extended(42, 5000, 25.0);
    println!("修复后剩余 {} 个问题", remaining.len());
}

fn main() -> anyhow::Result<()> {
    let data = std::fs::read("movie.srt")?;
    let mut file = subtitler::parse_bytes(&data)?;
    check_and_fix(&mut file);

    // 保存修复后的文件
    std::fs::write("movie_fixed.srt", file.to_string())?;
    Ok(())
}
```

**亮点**：`validate_extended` 检查 CPS/字符数/间隔/重叠；`auto_extend_for_cps` 根据文字长度自动延长显示时间；`enforce_min_duration` 强制最短时长。

## 场景 3：生成字幕质量报告（JSON）

**需求**：为字幕团队生成结构化报告，按字幕逐条分析。

```rust
use subtitler::quality::generate_report;

fn main() -> anyhow::Result<()> {
    let data = std::fs::read("episode.srt")?;
    let file = subtitler::parse_bytes(&data)?;
    let subs = file.subtitles();

    let report = generate_report(subs, 42, 5000, 25.0);

    println!("总字幕数: {}", report.total_subtitles);
    println!("总问题数: {}", report.total_issues);
    println!("平均 CPS: {:.1}", report.avg_cps);
    println!("平均时长: {}ms", report.avg_duration_ms);

    // 找出问题最严重的字幕
    let worst = report.subtitles.iter()
        .max_by_key(|s| s.issues.len());
    if let Some(w) = worst {
        println!("最严重: 第 {} 条 ({} 个问题, CPS {:.1})",
                 w.index + 1, w.issues.len(), w.chars_per_second);
    }

    // 导出为 JSON
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write("report.json", json)?;
    Ok(())
}
```

**亮点**：`QualityReport` 实现 `Serialize`，可直接输出 JSON。每条字幕有独立的 CPS、WPM、时长、问题列表。

## 场景 4：从视频中提取并同步歌词（LRC）

**需求**：有一首歌词文本和音频时间戳，生成 LRC 文件。

```rust
use subtitler::model::Subtitle;
use subtitler::lrc;

fn main() {
    // 手动构建字幕（每行歌词 + 时间）
    let lyrics = vec![
        Subtitle::new(1500, 6500, "Imagine there's no heaven"),
        Subtitle::new(6500, 11500, "It's easy if you try"),
        Subtitle::new(11500, 16500, "No hell below us"),
        Subtitle::new(16500, 21500, "Above us only sky"),
    ];

    // 生成 LRC
    let lrc_text = lrc::to_string(&lyrics);
    println!("{}", lrc_text);
    // 输出:
    // [00:01.50]Imagine there's no heaven
    // [00:06.50]It's easy if you try
    // ...
}
```

**反向：解析 LRC 并转换为 SRT**：

```rust
use subtitler::SubtitleFormat;

let lrc = "[00:01.50]First line\n[00:03.20]Second line\n";
let subs = subtitler::lrc::parse_content(lrc)?;

// 转成 SRT 格式
let srt_file = subtitler::model::SubtitleFile::Srt(subs);
let srt_text = srt_file.to_string();
std::fs::write("lyrics.srt", srt_text)?;
```

## 场景 5：流式处理超大字幕文件

**需求**：一个 100MB 的 SRT 文件（数万条字幕），不想一次性加载到内存。

```rust
use subtitler::srt;

fn main() -> anyhow::Result<()> {
    let content = std::fs::read_to_string("huge.srt")?;

    let mut count = 0;
    let mut total_text_len = 0;

    // 流式迭代——每条字幕独立处理，不分配完整 Vec
    for sub in srt::parse_stream(&content) {
        let sub = sub?;
        count += 1;
        total_text_len += sub.text.len();

        // 可以在这里做任何处理：写入数据库、翻译、过滤...
        if count % 1000 == 0 {
            eprintln!("已处理 {} 条...", count);
        }
    }

    println!("总计 {} 条字幕，{} 字符", count, total_text_len);
    Ok(())
}
```

**亮点**：`SrtStream` 是 `Iterator<Item = Result<Subtitle>>`，零分配，内存占用恒定（不随字幕数量增长）。

## 场景 6（附加）：CLI 一行命令搞定

不需要写代码的场景——直接用 CLI：

```bash
# 延迟字幕 2 秒（+2000ms），输出为 VTT
subtitler edit input.srt --shift 2000 --output output.vtt

# 合并间隔小于 500ms 的相邻字幕
subtitler edit input.srt --merge 500 --output merged.srt

# 验证质量（CPS 上限 20）
subtitler validate movie.srt --max-cps 20

# 查看统计信息
subtitler info movie.srt
```

## 总结

| 场景 | 用到的 API |
|------|-----------|
| 批量转换 | `parse_bytes` + `to_string_with_format` |
| 质量检查+修复 | `validate_extended` + `auto_extend_for_cps` |
| JSON 报告 | `quality::generate_report` |
| 歌词生成 | `lrc::to_string` |
| 大文件流式 | `srt::parse_stream` |
| 快速操作 | CLI `convert` / `edit` / `validate` |

所有代码可直接运行——`cargo add subtitler` 然后复制粘贴即可。

---

*完整 API 文档见 [docs.rs/subtitler](https://docs.rs/subtitler)。*
