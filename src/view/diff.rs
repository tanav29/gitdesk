use super::{
    ACCENT, ADDED, Desk, INK, MUTED, REMOVED, RULE, button, color, divider, label, scroll_list,
};
use rune::{El, Family, TextStyle, col, row, text};
use rune_kit::Kit;

const PAGE_LINES: usize = 600;

#[derive(Default)]
struct File<'a> {
    path: &'a str,
    previous: Option<&'a str>,
    status: Option<&'static str>,
    lines: Vec<Line<'a>>,
}

enum Line<'a> {
    Hunk(&'a str),
    Note(&'a str),
    Code {
        old: Option<usize>,
        new: Option<usize>,
        sign: char,
        code: &'a str,
    },
}

fn path_name(path: &str) -> &str {
    let path = path.trim_end_matches('\t').trim_matches('"');
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

fn hunk_start(header: &str) -> Option<(usize, usize)> {
    let mut fields = header.strip_prefix("@@ ")?.split_whitespace();
    let start = |range: &str, sign| -> Option<usize> {
        range.strip_prefix(sign)?.split(',').next()?.parse().ok()
    };
    Some((start(fields.next()?, '-')?, start(fields.next()?, '+')?))
}

fn parse<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<File<'a>> {
    let mut files: Vec<File<'a>> = Vec::new();
    let mut hunk = None;
    for raw in lines {
        if let Some(paths) = raw.strip_prefix("diff --git ") {
            let path = paths
                .rsplit_once(" \"b/")
                .or_else(|| paths.rsplit_once(" b/"))
                .map_or(paths, |(_, path)| path.trim_end_matches('"'));
            files.push(File {
                path,
                ..File::default()
            });
            hunk = None;
            continue;
        }
        if let Some(path) = raw
            .strip_prefix("diff --cc ")
            .or_else(|| raw.strip_prefix("diff --combined "))
        {
            files.push(File {
                path,
                ..File::default()
            });
            hunk = None;
            continue;
        }
        if files.is_empty() {
            files.push(File {
                path: "Patch",
                ..File::default()
            });
        }
        let file = files.last_mut().unwrap();
        if raw.starts_with("@@") {
            hunk = hunk_start(raw);
            file.lines.push(Line::Hunk(raw));
        } else if let Some((old, new)) = &mut hunk {
            if let Some(sign @ (' ' | '+' | '-')) = raw.chars().next() {
                let old_number = (sign != '+').then(|| {
                    let line = *old;
                    *old += 1;
                    line
                });
                let new_number = (sign != '-').then(|| {
                    let line = *new;
                    *new += 1;
                    line
                });
                file.lines.push(Line::Code {
                    old: old_number,
                    new: new_number,
                    sign,
                    code: &raw[1..],
                });
            } else {
                file.lines.push(Line::Note(raw));
            }
        } else if let Some(path) = raw.strip_prefix("+++ ") {
            if path != "/dev/null" {
                file.path = path_name(path);
            }
        } else if let Some(path) = raw.strip_prefix("--- ") {
            if path != "/dev/null" {
                file.path = path_name(path);
            }
        } else if let Some(path) = raw
            .strip_prefix("rename from ")
            .or_else(|| raw.strip_prefix("copy from "))
        {
            file.previous = Some(path);
        } else if let Some(path) = raw.strip_prefix("rename to ") {
            file.path = path;
            file.status = Some("Renamed");
        } else if let Some(path) = raw.strip_prefix("copy to ") {
            file.path = path;
            file.status = Some("Copied");
        } else if raw.starts_with("new file mode ") {
            file.status = Some("Added");
        } else if raw.starts_with("deleted file mode ") {
            file.status = Some("Deleted");
        } else if !raw.starts_with("index ") && !raw.starts_with("similarity index ") {
            file.lines.push(Line::Note(raw));
        }
    }
    files
}

fn mono(value: impl Into<String>, ink: &str) -> El {
    text(value.into())
        .font(
            TextStyle::new(12.0, 400.0)
                .family(Family::SystemMono)
                .lh(18.0),
        )
        .color(color(ink))
}

fn number(value: Option<usize>) -> El {
    row()
        .w(48.0)
        .px(6.0)
        .justify_end()
        .child(mono(value.map(|n| n.to_string()).unwrap_or_default(), MUTED).a11y_hidden())
}

fn line_view(line: Line<'_>) -> El {
    match line {
        Line::Hunk(header) => col()
            .px(10.0)
            .py(3.0)
            .fill(color("#222c33"))
            .child(mono(header, ACCENT).wrap_anywhere().selectable(true)),
        Line::Note(note) => col()
            .px(10.0)
            .py(3.0)
            .child(mono(note, MUTED).wrap_anywhere().selectable(true)),
        Line::Code {
            old,
            new,
            sign,
            code,
        } => {
            let (ink, background) = match sign {
                '+' => (ADDED, "#1e3028"),
                '-' => (REMOVED, "#35252a"),
                _ => (INK, "#171b20"),
            };
            row()
                .items_start()
                .min_h(18.0)
                .fill(color(background))
                .child(number(old))
                .child(number(new))
                .child(mono(sign.to_string(), ink).w(18.0))
                .child(
                    mono(code, ink)
                        .min_w(0.0)
                        .grow(1.0)
                        .pr(8.0)
                        .wrap_anywhere()
                        .selectable(true),
                )
        }
    }
}

fn file_view(file: File<'_>) -> El {
    let path = file.previous.map_or_else(
        || file.path.to_owned(),
        |previous| format!("{previous} → {}", file.path),
    );
    let mut header = row()
        .px(10.0)
        .py(6.0)
        .gap(8.0)
        .center_y()
        .fill(color("#252a30"))
        .child(
            label(path, 13.0, 600.0, INK)
                .grow(1.0)
                .min_w(0.0)
                .wrap_anywhere()
                .selectable(true),
        );
    if let Some(status) = file.status {
        header = header.child(label(status, 12.0, 500.0, MUTED));
    }
    let mut body = col()
        .clip_rounded(5.0)
        .border(1.0, color(RULE))
        .child(header)
        .child(divider());
    for line in file.lines {
        body = body.child(line_view(line));
    }
    body
}

pub(super) fn view(s: &Desk, k: &Kit, empty: &str) -> El {
    contents(
        if s.busy { "" } else { &s.patch },
        s.diff_pages,
        k,
        empty,
        |s| s.diff_pages += 1,
    )
}

pub(super) fn contents(
    patch: &str,
    pages: usize,
    k: &Kit,
    empty: &str,
    show_more: impl Fn(&mut Desk) + 'static,
) -> El {
    let mut content = col().gap(8.0);
    if patch.is_empty() {
        return scroll_list(content.child(label(empty, 13.0, 400.0, MUTED)));
    }
    let mut lines = patch.lines();
    for file in parse(lines.by_ref().take(PAGE_LINES * pages)) {
        content = content.child(file_view(file));
    }
    if lines.next().is_some() {
        content = content.child(button("Show more").on_press(show_more).build(k));
    }
    scroll_list(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_patch_headers_from_code_and_restarts_numbering_per_hunk() {
        let patch = concat!(
            "diff --git a/old name.txt b/new name.txt\n",
            "rename from old name.txt\n",
            "rename to new name.txt\n",
            "--- a/old name.txt\t\n",
            "+++ b/new name.txt\t\n",
            "@@ -7,2 +9,2 @@\n",
            " context\n",
            "--- removed code\n",
            "+++ added code  \n",
            "@@ -20 +25 @@ second hunk\n",
            "-before\n",
            "+after\n",
            "\\ No newline at end of file\n",
            "diff --git a/image.bin b/image.bin\n",
            "Binary files a/image.bin and b/image.bin differ\n",
        );
        let files = parse(patch.lines());
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "new name.txt");
        assert_eq!(files[0].previous, Some("old name.txt"));
        assert!(matches!(
            files[0].lines.as_slice(),
            [
                Line::Hunk(_),
                Line::Code {
                    old: Some(7),
                    new: Some(9),
                    sign: ' ',
                    ..
                },
                Line::Code {
                    old: Some(8),
                    new: None,
                    sign: '-',
                    ..
                },
                Line::Code {
                    old: None,
                    new: Some(10),
                    code: "++ added code  ",
                    ..
                },
                Line::Hunk(_),
                Line::Code {
                    old: Some(20),
                    new: None,
                    ..
                },
                Line::Code {
                    old: None,
                    new: Some(25),
                    ..
                },
                Line::Note(_),
            ]
        ));
        assert_eq!(files[1].path, "image.bin");
        assert!(matches!(files[1].lines.as_slice(), [Line::Note(_)]));
    }
}
