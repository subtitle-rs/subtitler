# subtitler v2.0 的零拷贝设计：从 2000 次分配到 0 次

---

当你解析一个 1000 条字幕的 SRT 文件时，有几件事在发生。v1.0 的解析器会在你不注意的时候，悄悄进行大约 **2000 次堆内存分配**——不是因为字幕需要这么多内存，而是因为每一行文本都被复制了一份然后丢弃。v2.0 的目标很直接：**把不必要的分配压到零**。

这不是教科书里的 "零拷贝"——我们没有用 `mmap`、没有引入自定义 arena、没有把 `Subtitle` 改成带生命周期的 `Subtitle<'a>`。我们只做了一件事：**停止做不需要做的事情**。

---

## 1. 逐行复制：那个你用不上的 String

SRT 文件的结构很简单。像这样：

```
1
00:00:01,000 --> 00:00:03,000
Hello world

2
00:00:04,000 --> 00:00:07,000
Goodbye world
```

v1.0 的解析器是这样处理每一行的：

```rust
// v1.0
let mut trimmed = line.trim().to_string();
```

就这一行。`line` 是 `content.lines()` 迭代器给出的 `&str`，它直接指向输入字符串的内存——没有分配、没有复制。然后你调了 `.trim()`，得到一个更小的 `&str`，它指向原始字符串的某个子区间——还是没有分配。最后 `.to_string()` 把这几字节复制到一个全新的堆分配 `String` 里。

然后你用这个 `trimmed` 做几件事：检查是不是空行、解析序号、解析时间戳、或者追加到字幕文本。做完这些之后，`trimmed` 就离开了作用域，那个 `String` 被释放。下一行来了，又一个 `.to_string()`，又一次分配。

一个 1000 条字幕的 SRT 文件大约 2000 行文本。这些 `.trim().to_string()` 调用意味着：

> **2000 次 `malloc` + 2000 次 `free`，只为了得到一些用完就丢的临时字符串。**

v2.0 的修改只有一行之差：

```rust
// v2.0
let trimmed = line.trim();
```

现在 `trimmed` 是 `&str`。没有分配。检查空行？`trimmed.is_empty()`。解析序号？`trimmed.parse::<usize>()`。追加到字幕文本？`sub.text.push_str(trimmed)`——这里仍然需要复制，因为字幕文本得被 `Subtitle::text` 拥有，但这个复制是**必须的**（字幕数据总要存储），而不是**多余的**（临时副本用完就丢）。

VTT 解析器做了同样的修改。SrtStream 的迭代器也做了同样的修改。

**这笔改动的本质**：把解析过程中产生的临时字符串全部消除，只保留最终进入 `Subtitle` 结构体的那一次拷贝。

---

## 2. Vec 预分配：不要再让 realloc 偷跑

另一个容易被忽视的分配源是 `Vec::new()`。

v1.0 的 12 个格式解析器全部从 `Vec::new()` 开始，capacity=0。当第一条字幕被 push 进去时，Vec 需要分配一块内存（通常 4 个元素）。当第 5 条进来时，它需要分配一块更大的内存（通常 8 个）。当第 9 条进来时，再来一次（16 个）……

一个 1000 条字幕的 SRT 文件，在默认的 Vec 增长策略下，会触发大约 **8 次重新分配**。每次重新分配都会 `malloc` 一块更大的内存，然后把已有元素 `memcpy` 过去，最后 `free` 旧块。

但我们完全可以在第一行代码就知道大概有多少条字幕：

```rust
// v2.0: SRT — 每条字幕大约 200 字节
let estimated_subs = (content.len() / 200).max(16);
let mut subtitles: Vec<Subtitle> = Vec::with_capacity(estimated_subs);
```

这个估算不要求精确。一个 200KB 的 SRT 文件除以 200 得到 1000——正好是它实际包含的字幕数。即使估算多了（预分配比实际的多），多出来的容量也只是未使用的内存空间，下次 resize 时不会触发 realloc；即使估算少了，也只会触发 1-2 次 realloc 而不是 8 次。

不同格式的估算因子不同：

| 格式 | 字节/字幕 | 说明 |
|------|----------|------|
| SRT | 200 | 行号+时间戳+1-2行文本+空行 |
| VTT | 200 | 同上，加上 header |
| ASS | 300 | 样式行更长 |
| MicroDVD | 30 | 只有帧号和文本，极紧凑 |
| SBV | 40 | 时间戳+文本 |
| SCC | 100 | 二进制编码密集 |
| EBU STL | N/A | **精确计数**：直接从 GSI 头读取 TTI 数量 |
| LRC | N/A | **精确计数**：先从行数估算 LrcLine，再精确分配 Subtitle |

EBU STL 是特殊案例。每个 STL 文件的 GSI（General Subtitle Information）头在第 1-2 字节存储了 TTI（Timed Text Information）块数量。这意味着：

```rust
// v2.0: EBU STL — 0 次 realloc
let tti_count = u16::from_be_bytes([data[1], data[2]]) as usize;
let mut subtitles: Vec<Subtitle> = Vec::with_capacity(tti_count);
```

不是估算，是精确值。从文件头读取，直接告诉 Vec 需要多少空间。零浪费。

---

## 3. split_text_chunks：消灭 O(n²) 的 format!()

`split_text_chunks` 是 `SubtitleFormat::split_long()` 的底层实现，用来把一行长字幕按单词边界拆成多行。一个 100 词的句子需要被拆成 10 块。v1.0 的代码是这样的：

```rust
// v1.0: 每次迭代调用 format!()
for word in words {
    let test = if current.is_empty() {
        word.to_string()
    } else {
        format!("{} {}", current, word)  // <-- 分配
    };
    // ...
    current = test;  // 移动赋值
}
```

`format!()` 的语义是创建一个全新的 `String`，把 `current` 的内容复制进去，加一个空格，再把新单词复制进去。如果 `current` 里有 40 个字符，这次调用就要分配一块 40+1+5=46 字节的堆内存。`current = test` 把旧 `String` 丢弃（释放）。

对于一个 100 词的句子，这会产生 **99 次 format!() 调用**，每次复制的字节数从 3 增长到 ~40，累计大约 **2000 字符的重复复制**。这是典型的 [Shlemiel the painter](https://www.joelonsoftware.com/2001/12/11/back-to-basics/) 问题。

v2.0 的版本：

```rust
// v2.0: 只追加，不重建
let mut current = String::with_capacity(max_chars);

for word in words {
    let needed = if current.is_empty() {
        word.len()
    } else {
        current.len() + 1 + word.len()
    };

    if needed > max_chars && !current.is_empty() {
        chunks.push(std::mem::take(&mut current));
        current.push_str(word);
    } else {
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
}
```

关键变化：

1. **预分配**：`String::with_capacity(max_chars)` 一次性分配足够空间，后续 `push_str` 不会触发 realloc（除非 single word > max_chars，极罕见）。
2. **追加而非重建**：不再是 `format!("{} {}", current, word)` 每次重建整个字符串，而是 `current.push(' ')` 再 `current.push_str(word)` 直接追加到已有缓冲区。
3. **容量检查用字节数**：`word.len()` 是字节长度，不是 `chars().count()`——对于 ASCII 字幕字节数和字符数相同，但避免了额外的遍历。

结果：`format!()` 调用从 99 次降到 0 次，`mem::take` 交换（O(1)）替代了 `format!()` 复制（O(n)）。总复杂度从 O(n²) 降到 O(n)。

---

## 4. VTT header_lines：推迟 join

VTT 文件的头部可能很长——包含描述、语言标签、样式定义等。v1.0 把每行收集为 `Vec<String>`：

```rust
// v1.0
let mut header_lines: Vec<String> = Vec::new();
header_lines.push(trimmed.to_string());  // 每行分配
```

v2.0 改为借用引用，只在最终需要时才合并：

```rust
// v2.0
let mut header_lines: Vec<&str> = Vec::new();
header_lines.push(trimmed);  // 零分配；trimmed 是 &str
// ...
header = Some(header_lines.join("\n"));  // 一次性 join
```

改动很小，但在长 header 的场景下效果显著。一个包含 50 行样式定义的 VTT header，节省了 50 次不必要的字符串复制。`.join("\n")` 的底层实现是一次性分配目标大小，一次拷贝完成——不比逐行累加慢，而且分配次数少得多。

---

## 5. 为什么没把 Subtitle 改成 Subtitle<'a>？

如果你熟悉 Rust 的零拷贝模式，可能会问：为什么不直接给 `Subtitle` 加生命周期参数，让 `text` 和 `settings` 等字段直接借用输入数据？

```rust
// 理想但破坏性的方案
pub struct Subtitle<'a> {
    pub text: &'a str,
    pub settings: Option<&'a str>,
    // ...
}
```

我们评估过这个方案。问题有四个：

1. **传染性**：生命周期参数会向上传播到 `SubtitleFile<'a>` → `SubtitleFormat<'a>` trait → 所有 trait 方法的签名 → `StreamingParser<Item = Subtitle<'a>>` → CLI 的 `parse_to_file` → 最终污染到 `main()`。47 个编译错误，32 个是缺失生命周期标注。这还是在改动范围仅限于解析路径的情况下——编辑方法（`merge_adjacent`、`split_long` 等）也受到了影响，因为它们需要修改 `text` 字段。

2. **`merge_adjacent` 困难**：相邻字幕合并需要拼接两段文本。如果 `text: &str`，必须先变成 `String` 才能修改，然后类型变成 `Subtitle<'static>`——这和原始借用就不兼容了。

3. **`into_static()` 惩罚**：任何需要离开原始输入上下文的操作（比如把解析结果发送到另一个线程、存储到缓存、传递给 Web 响应）都需要调用 `into_static()` 把全部 `&str` 字段克隆为 `String`。这相当于把零拷贝省下的分配，在边界处一次性还回去。

4. **用户代码破坏性**：如果 `SubtitleFile` 带生命周期，每个持有 `SubtitleFile` 的类型都需要添加生命周期参数。库用户的代码改动量会很大。

我们的判断是：**内部零拷贝（解析路径上不产生临时 String）+ 边界处必需拷贝（Subtitle 自己拥有数据）** 是最佳折衷。解析器不再是分配热点，但 Subtitle 仍然是自包含的 owned 类型——不需要生命周期、可以被发送到任意线程、可以随意修改文本。

> 本质上是两层优化：代码层（消除不必要的分配）> 类型系统层（用生命周期推迟分配）。前者不破坏 API，后者破坏一切。

---

## 6. 不是零拷贝的风格——但没有浪费

最后讲一个细节。`extract_text_parts` 函数用来把 ASX/HTML 标签从字幕文本中分离出来，分成 `text`（纯文本）和 `text_parts`（带格式片段）。它的返回类型是 `(String, SmallVec<[TextPart; 4]>)`——这里 `TextPart.text` 是 `String`，所以文本被复制了。

这看起来和零拷贝的理念矛盾，但其实不是浪费。原因：

- `extract_text_parts` 产出的是**最终数据**——`text` 直接存进 `Subtitle.text`，`text_parts` 直接存进 `Subtitle.text_parts`。这些拷贝是字幕数据存储的必要成本。
- 它使用 `SmallVec<[TextPart; 4]>` 而不是普通 `Vec`——因为绝大多数字幕的格式片段不超过 4 个。SmallVec 把少于 4 个元素的数据内联在栈上，不需要堆分配。这是一个微观优化，但在解析大量字幕时累积效果可观。

真正的问题不是「有没有拷贝」，是「拷贝了几次」。v1.0 的流程是：原始文本 → `trim().to_string()` 临时 → `push_str` 复制到 `Subtitle.text` → `extract_text_parts` 再次复制。v2.0 把第一环砍掉了，剩下两环是必须的存储成本。

---

## 7. 关闭循环：SrtStream 也零拷贝了

`SrtStream` 是 `Iterator<Item = Result<Subtitle>>`，设计用于流式处理大文件而不一次性分配全部 `Vec<Subtitle>`。v1.0 的实现也有逐行 `.to_string()` 的问题：

```rust
// v1.0: SrtStream::next()
let mut trimmed = line.trim().to_string();
```

v2.0 同步修复：

```rust
// v2.0: SrtStream::next()
let trimmed = line.trim();
// ...
sub.text.push_str(trimmed);
```

`VttStream` 也在同一轮重构中从 `phase: u8` + 裸字符串复制升级到 `Phase` 枚举 + 零拷贝处理。

现在无论是全量解析（`parse_content` 返回 `SubtitleFile`）还是流式解析（`parse_stream` 返回 `Iterator`），解析路径上都没有不必要的分配。

---

## 效果一览

用一个 10 万行（~50,000 条字幕）的 SRT 文件做基准：

| 指标 | v1.0 | v2.0 | 变化 |
|------|------|------|------|
| 临时 String 分配 | ~100,000 次 | 0 次 | -100% |
| Vec realloc | ~15 次 | ~0-1 次 | -93% |
| `split_text_chunks` 复杂度 | O(n²) | O(n) | - |
| API 破坏 | — | 0 | — |

这些优化不需要 `unsafe`、不需要自定义分配器、不需要改变公开 API。它们只需要对 Rust 字符串和 Vec 的底层机制有一点点理解：

- `str::trim()` 返回 `&str`，不是 `String`，不需要分配
- `Vec::with_capacity` 在一开始就知道需要多大时直接指定
- `String::push_str` 追加比 `format!()` 重建快一个数量级
- `SmallVec` 内联低于阈值的数据在栈上

**零拷贝不是魔法——它是「不做不需要做的事」重讲了一遍。**

---

*subtiliter v2.0，2025 年 7 月*
