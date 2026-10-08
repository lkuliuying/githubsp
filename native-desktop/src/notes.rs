use crate::{presentation::model, NoteBlock, NoteTableRow};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Default)]
struct Block {
    markdown: String,
    plain: String,
    kind: i32,
    level: i32,
    rows: Vec<Vec<String>>,
    cell: String,
}

fn escaped(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        if "\\`*_{}[]<>#!|~".contains(ch) {
            result.push('\\');
        }
        result.push(ch);
    }
    result
}

fn flush(block: &mut Block, output: &mut Vec<NoteBlock>) {
    if !block.plain.trim().is_empty() {
        // 解析器输出只用于排版；不支持的排版安全降级为可读文本，不执行 HTML。
        let content = slint::StyledText::from_markdown(&block.markdown)
            .unwrap_or_else(|_| slint::StyledText::from_plain_text(&block.plain));
        output.push(NoteBlock {
            content,
            plain: block.plain.clone().into(),
            kind: block.kind,
            level: block.level,
            table: model(
                block
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(index, cells)| NoteTableRow {
                        cells: model(cells.iter().map(|cell| cell.as_str().into()).collect()),
                        header: index == 0,
                    })
                    .collect(),
            ),
        });
    }
    *block = Block::default();
}

pub fn render(markdown: &str) -> Vec<NoteBlock> {
    let mut blocks = Vec::new();
    let mut block = Block::default();
    let mut links: Vec<Option<String>> = Vec::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut quote_depth = 0;
    let mut image_depth = 0;
    let mut table = false;
    let mut code = false;
    for event in Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    ) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut block, &mut blocks);
                block.kind = 1;
                block.level = level as i32;
                block.markdown.push_str("**");
            }
            Event::End(TagEnd::Heading(_)) => {
                block.markdown.push_str("**");
                flush(&mut block, &mut blocks);
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush(&mut block, &mut blocks);
                block.kind = 2;
                code = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                flush(&mut block, &mut blocks);
                code = false;
            }
            Event::Start(Tag::Table(_)) => {
                flush(&mut block, &mut blocks);
                table = true;
                block.kind = 3;
            }
            Event::End(TagEnd::Table) => {
                flush(&mut block, &mut blocks);
                table = false;
            }
            Event::Start(Tag::TableHead | Tag::TableRow) => block.rows.push(Vec::new()),
            Event::End(TagEnd::TableCell) => {
                if let Some(row) = block.rows.last_mut() {
                    row.push(std::mem::take(&mut block.cell));
                }
                block.plain.push_str("  │  ");
            }
            Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                block.plain.push('\n');
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush(&mut block, &mut blocks);
                quote_depth += 1;
                block.kind = 4;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush(&mut block, &mut blocks);
                quote_depth -= 1;
            }
            Event::Start(Tag::List(start)) => lists.push(start),
            Event::End(TagEnd::List(_)) => {
                flush(&mut block, &mut blocks);
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                flush(&mut block, &mut blocks);
                let prefix = match lists.last_mut() {
                    Some(Some(value)) => {
                        let prefix = format!("{value}. ");
                        *value += 1;
                        prefix
                    }
                    _ => "• ".into(),
                };
                block.plain.push_str(&prefix);
                block.markdown.push_str(&escaped(&prefix));
            }
            Event::End(TagEnd::Item) => flush(&mut block, &mut blocks),
            Event::Start(Tag::Paragraph) => {
                if quote_depth > 0 {
                    block.kind = 4;
                }
            }
            Event::End(TagEnd::Paragraph) => flush(&mut block, &mut blocks),
            Event::Start(Tag::Strong) => block.markdown.push_str("**"),
            Event::End(TagEnd::Strong) => block.markdown.push_str("**"),
            Event::Start(Tag::Emphasis) => block.markdown.push('*'),
            Event::End(TagEnd::Emphasis) => block.markdown.push('*'),
            Event::Start(Tag::Strikethrough) => block.markdown.push_str("~~"),
            Event::End(TagEnd::Strikethrough) => block.markdown.push_str("~~"),
            Event::Start(Tag::Image { .. }) => {
                image_depth += 1;
            }
            Event::End(TagEnd::Image) => {
                image_depth -= 1;
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let target = if image_depth == 0 {
                    githubsp_lib::updates::validate_update_link(&dest_url)
                        .ok()
                        .map(|url| url.to_string().replace('(', "%28").replace(')', "%29"))
                } else {
                    None
                };
                if target.is_some() {
                    block.markdown.push('[');
                }
                links.push(target);
            }
            Event::End(TagEnd::Link) => {
                if let Some(Some(target)) = links.pop() {
                    block.markdown.push_str(&format!("]({target})"));
                }
            }
            Event::Code(text) => {
                if table {
                    block.cell.push_str(&text);
                }
                block.plain.push_str(&text);
                let fence = "`".repeat(text.chars().filter(|c| *c == '`').count() + 1);
                block.markdown.push_str(&format!("{fence} {text} {fence}"));
            }
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                if table {
                    block.cell.push_str(&text);
                }
                block.plain.push_str(&text);
                block.markdown.push_str(&escaped(&text));
            }
            Event::SoftBreak | Event::HardBreak => {
                if table {
                    block.cell.push('\n');
                }
                block.plain.push('\n');
                block.markdown.push('\n');
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "☑ " } else { "☐ " };
                block.plain.push_str(marker);
                block.markdown.push_str(marker);
            }
            Event::Rule => {
                flush(&mut block, &mut blocks);
                block.plain = "──────────────────".into();
                block.markdown = block.plain.clone();
                flush(&mut block, &mut blocks);
            }
            _ => (),
        }
        if table {
            block.kind = 3;
        }
        if code {
            block.kind = 2;
        }
    }
    flush(&mut block, &mut blocks);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Model;
    #[test]
    fn markdown_retains_blocks_and_displays_html_and_image_alt_only() {
        let input = "# 更新\n\n**重点** 与 `代码`\n\n> 引用\n\n- 条目\n\n```rust\nfn main() {}\n```\n\n| 名称 | 值 |\n| --- | --- |\n| A | B |\n\n<script>alert(1)</script>\n\n![图片说明](https://example.com/private.png)\n\n[安全](https://github.com/owner/repo/compare/v1...v2) [危险](javascript:alert(1))";
        let rows = render(input);
        assert!(rows.iter().any(|r| r.kind == 1));
        assert!(rows.iter().any(|r| r.kind == 2));
        assert!(rows.iter().any(|r| r.kind == 3));
        let table = &rows.iter().find(|r| r.kind == 3).unwrap().table;
        assert_eq!(table.row_count(), 2);
        assert!(table.row_data(0).unwrap().header);
        assert_eq!(
            table.row_data(0).unwrap().cells.row_data(0).unwrap(),
            "名称"
        );
        assert_eq!(table.row_data(1).unwrap().cells.row_data(1).unwrap(), "B");
        assert!(rows.iter().any(|r| r.kind == 4));
        let plain = rows
            .iter()
            .map(|r| r.plain.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("<script>alert(1)</script>"));
        assert!(plain.contains("图片说明"));
        assert!(!plain.contains("private.png"));
        assert!(!plain.contains("javascript:"));
        assert!(plain.contains("安全"));
        assert!(plain.contains("危险"));
    }
    #[test]
    fn empty_notes_have_no_blocks_and_unicode_is_preserved() {
        assert!(render("").is_empty());
        assert_eq!(render("中文 **路径**")[0].plain, "中文 路径");
    }
}
