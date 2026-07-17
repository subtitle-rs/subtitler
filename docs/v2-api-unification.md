# v2.0 API 统一设计 (RFC)

> 状态: **草案 / 未实施**
> 用于记录 v1.x 暴露出的 API 不一致问题，及 v2.0 的破坏性变更方案。
> v1.x 期间只补测试、加 deprecation 注释，不真正破坏兼容性。

## 1. 动机

v1.x 中各格式模块的公共入口 (`parse_content` / `parse_bytes` / `parse_file` /
`parse_url`) 返回类型不一致：

| 格式 | 当前返回类型 | 问题 |
|------|-------------|------|
| SRT / VTT / SBV / LRC / MPL2 | `Vec<Subtitle>` | 丢失 header / 多时间戳信息 |
| ASS / SSA / SCC / EBU STL / SAMI | `SubtitleFile` 或自定义数据 | ✓ 正确 |
| MicroDVD / SubViewer / TTML | `SubtitleFile` | ✓ 正确 |

LRC 模块已用 `#[deprecated]` 标记 `parse_content`，建议改用 `LrcData::parse`，
但 `parse_bytes` / `parse_file` / `parse_url` 仍走 deprecated 路径，等于
迁移只做了一半。用户拿不到多时间戳结构。

## 2. 目标

- 所有格式模块的 `parse_*` 入口统一返回 `SubtitleFile`。
- 移除 LRC 模块的 `#[allow(deprecated)]` 链。
- 保留性能：核心解析仍为同步、零分配优先。

## 3. 设计

### 3.1 统一签名

```rust
// 每个格式模块都遵守：
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile>;
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile>;
pub async fn parse_file(path: impl AsRef<Path>) -> AnyResult<SubtitleFile>;
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile>;
```

### 3.2 LRC 多时间戳保留

`SubtitleFile::Lrc` 变体改为强类型：

```rust
#[cfg(feature = "lrc")]
Lrc(LrcData),

pub struct LrcData {
  pub metadata: HashMap<String, String>, // [ar:], [ti:], [al:] 等
  pub lines: Vec<LrcLine>,
}

pub struct LrcLine {
  pub times_ms: SmallVec<[u64; 1]>, // 一行多个时间戳
  pub text: String,
}
```

`LrcData` 实现 `SubtitleFormat` trait，`subtitles()` 把每个时间戳展平为
带默认时长的 `Subtitle`，兼容 trait 默认方法。

### 3.3 迁移路径

1. **v1.5**: 在所有返回 `Vec<Subtitle>` 的入口加 `#[deprecated(since =
   "1.5.0", note = "returns Vec; use parse_xxx_full for SubtitleFile")]`，
   并新增 `*_full` 后缀的 `SubtitleFile` 版本。
2. **v2.0**: `*_full` 成为默认入口（去掉后缀），旧 `Vec` 版本移除。

## 4. 不实施的原因（v1.x）

- 返回类型从 `Vec<Subtitle>` 改成 `SubtitleFile` 是 **source-breaking** 变更，
  按 SemVer 必须 bump major。
- v1.x 阶段优先补行为正确性（已完成的 P0/P1 修复），API 重整留待 v2.0
  与零拷贝解析一起做。

## 5. 实施清单（v2.0 启动时）

- [ ] SRT/VTT/SBV/MPL2: `parse_*` 返回 `SubtitleFile::{Srt,Vtt,Sbv,Mpl2}`
- [ ] LRC: 引入 `LrcData`，`SubtitleFile::Lrc(LrcData)`
- [ ] 移除全部 `#[allow(deprecated)]`（共 8 处）
- [ ] main.rs `parse_to_file` 不再 `#[allow(deprecated)]`
- [ ] 文档：MIGRATION.md 增补 v1.x → v2.0 章节
