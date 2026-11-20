use super::{ACCENT, Desk, HOVER, INK, MUTED, SELECTED, color, label, scroll_list};
use crate::git::HistoryRow;
use rune::{El, Family, TextStyle, col, row, text};

pub(super) fn view(s: &Desk, history: &[HistoryRow]) -> El {
    if history.is_empty() {
        return scroll_list(col().child(label("No commits yet.", 14.0, 400.0, MUTED)));
    }
    let columns = history
        .iter()
        .map(|line| line.graph.len())
        .max()
        .unwrap_or(0);
    let graph_width = (columns as f32 * 8.0).clamp(24.0, 240.0);
    let mut list = col();
    for line in history {
        let strokes: String = line
            .graph
            .chars()
            .map(|c| match c {
                '*' => '●',
                '|' => '│',
                '/' => '╱',
                '\\' => '╲',
                '-' => '─',
                '_' => '─',
                other => other,
            })
            .collect();
        let graph = text(strokes)
            .font(TextStyle::new(15.0, 500.0).family(Family::SystemMono))
            .color(color(if line.commit.is_some() { ACCENT } else { MUTED }))
            .w(graph_width)
            .truncate()
            .a11y_hidden();
        let item = row().px(8.0).gap(10.0).center_y().child(graph);
        let Some(commit) = &line.commit else {
            list = list.child(item.h(20.0));
            continue;
        };
        let selected = s
            .selected_commit
            .as_ref()
            .is_some_and(|selected| selected.id == commit.id);
        let mut item = item
            .h(34.0)
            .radius(5.0)
            .role("button")
            .label(format!("{} {}", commit.short_id, commit.title))
            .description("View commit diff")
            .focusable(!s.busy)
            .disabled(s.busy)
            .focus_ring(2.0, 0.0, color(ACCENT))
            .hover(color(HOVER))
            .key(format!("commit-{}", commit.id))
            .child(
                label(&commit.short_id, 12.0, 500.0, MUTED)
                    .w(68.0)
                    .truncate(),
            )
            .child(
                label(&commit.title, 14.0, 500.0, INK)
                    .grow(1.0)
                    .min_w(0.0)
                    .truncate(),
            );
        if !commit.refs.is_empty() {
            item = item.child(
                label(&commit.refs, 12.0, 500.0, ACCENT)
                    .max_w(144.0)
                    .truncate(),
            );
        }
        if selected {
            item = item.fill(color(SELECTED));
        }
        if !s.busy {
            let commit = commit.clone();
            item = item.on_press(move |s: &mut Desk| s.select_commit(commit.clone()));
        }
        list = list.child(item);
    }
    scroll_list(list)
}
