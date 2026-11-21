use super::{ACCENT, Desk, HOVER, INK, MUTED, SELECTED, color, label, scroll_list};
use crate::git::Commit;
use rune::{Color, El, Path, Shape, canvas, col, el, row, shape::LineCap};

const ROW_HEIGHT: f32 = 34.0;
const LANE_COLORS: [&str; 6] = [
    ACCENT, "#7844a1", "#15776c", "#a65321", "#b04463", "#526176",
];

struct GraphRow {
    lane: usize,
    incoming: Vec<usize>,
    passing: Vec<usize>,
    outgoing: Vec<usize>,
}

fn free_lane(lanes: &mut Vec<Option<&str>>) -> usize {
    if let Some(index) = lanes.iter().position(Option::is_none) {
        index
    } else {
        lanes.push(None);
        lanes.len() - 1
    }
}

fn graph_rows(history: &[Commit]) -> (Vec<GraphRow>, usize) {
    let mut lanes: Vec<Option<&str>> = Vec::new();
    let mut rows = Vec::with_capacity(history.len());
    for commit in history {
        let mut incoming = Vec::new();
        let mut passing = Vec::new();
        for (index, target) in lanes.iter_mut().enumerate() {
            match target {
                Some(id) if *id == commit.id => {
                    incoming.push(index);
                    *target = None;
                }
                Some(_) => passing.push(index),
                None => {}
            }
        }
        let lane = incoming
            .first()
            .copied()
            .unwrap_or_else(|| free_lane(&mut lanes));

        // Keep each link to a parent in its own lane until it reaches that commit.
        let mut outgoing = Vec::with_capacity(commit.parents.len());
        for (index, parent) in commit.parents.iter().enumerate() {
            let parent_lane = if index == 0 {
                lane
            } else {
                free_lane(&mut lanes)
            };
            lanes[parent_lane] = Some(parent);
            outgoing.push(parent_lane);
        }
        rows.push(GraphRow {
            lane,
            incoming,
            passing,
            outgoing,
        });
    }
    (rows, lanes.len())
}

fn connect(shape: Shape, from: [f32; 2], to: [f32; 2], width: f32, ink: Color) -> Shape {
    if from[0] == to[0] {
        return shape.line(from, to, width, LineCap::Round, ink);
    }
    let mid = (from[1] + to[1]) * 0.5;
    let path = Path::new()
        .move_to(from)
        .cubic_to([from[0], mid], [to[0], mid], to);
    shape.stroke_path(&path, width, LineCap::Round, ink)
}

fn draw_graph(graph: GraphRow, pitch: f32, width: f32) -> El {
    let x = |lane: usize| 4.0 + (lane as f32 + 0.5) * pitch;
    let ink = |lane: usize| color(LANE_COLORS[lane % LANE_COLORS.len()]);
    let stroke = (pitch * 0.24).min(2.0);
    let center = [x(graph.lane), ROW_HEIGHT * 0.5];
    let mut shape = Shape::new();
    for lane in graph.passing {
        shape = shape.line(
            [x(lane), 0.0],
            [x(lane), ROW_HEIGHT],
            stroke,
            LineCap::Round,
            ink(lane),
        );
    }
    for lane in graph.incoming {
        shape = connect(shape, [x(lane), 0.0], center, stroke, ink(lane));
    }
    for lane in graph.outgoing {
        shape = connect(shape, center, [x(lane), ROW_HEIGHT], stroke, ink(lane));
    }

    let radius = (pitch * 0.35).min(4.5);
    canvas(shape)
        .size(width, ROW_HEIGHT)
        .min_w(width)
        .a11y_hidden()
        .child(
            el().absolute(center[0] - radius, center[1] - radius)
                .size(radius * 2.0, radius * 2.0)
                .radius(radius)
                .fill(ink(graph.lane)),
        )
}

pub(super) fn view(s: &Desk, history: &[Commit]) -> El {
    if history.is_empty() {
        return scroll_list(col().child(label("No commits yet.", 14.0, 400.0, MUTED)));
    }
    let (graph, lanes) = graph_rows(history);
    let pitch = (232.0 / lanes as f32).min(18.0);
    let graph_width = 8.0 + lanes as f32 * pitch;
    let mut list = col();
    for (commit, graph) in history.iter().zip(graph) {
        let selected = s
            .selected_commit
            .as_ref()
            .is_some_and(|selected| selected.id == commit.id);
        let mut item = row()
            .px(8.0)
            .gap(10.0)
            .center_y()
            .h(ROW_HEIGHT)
            .radius(5.0)
            .role("button")
            .label(format!("{} {}", commit.short_id, commit.title))
            .description("View commit diff")
            .focusable(!s.busy)
            .disabled(s.busy)
            .focus_ring(2.0, 0.0, color(ACCENT))
            .hover(color(HOVER))
            .key(format!("commit-{}", commit.id))
            .child(draw_graph(graph, pitch, graph_width))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_lanes_fan_out_and_meet_at_the_shared_commit() {
        let history = [
            ("merge", &["main", "topic", "docs"][..]),
            ("topic", &["root"]),
            ("docs", &["root"]),
            ("main", &["root"]),
            ("root", &[]),
            ("unrelated", &[]),
        ]
        .map(|(id, parents)| Commit {
            id: id.to_owned(),
            short_id: id.to_owned(),
            parents: parents.iter().map(|parent| (*parent).to_owned()).collect(),
            refs: String::new(),
            title: String::new(),
        });

        let (rows, lanes) = graph_rows(&history);
        assert_eq!(lanes, 3);
        assert!(rows[0].incoming.is_empty());
        assert_eq!(rows[0].outgoing, [0, 1, 2]);
        assert_eq!(rows[1].lane, 1);
        assert_eq!(rows[1].passing, [0, 2]);
        assert_eq!(rows[2].lane, 2);
        assert_eq!(rows[2].passing, [0, 1]);
        assert_eq!(rows[3].lane, 0);
        assert_eq!(rows[3].passing, [1, 2]);
        assert_eq!(rows[4].incoming, [0, 1, 2]);
        assert!(rows[4].outgoing.is_empty());
        assert!(rows[5].incoming.is_empty() && rows[5].passing.is_empty());
    }
}
