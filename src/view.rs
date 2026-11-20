use super::{Desk, DiffMode, Tab};
use crate::git::{Action, Change, ConfigEdit, Scope, Snapshot};
use rune::{Color, Cx, El, FileDialogOptions, ScrollSpec, TextStyle, col, el, row, text};
use rune_kit::{
    Button, ButtonVariant, Checkbox, ControlSize, Kit, Select, SelectAppearance, TextInput, root,
};

mod diff;
mod github_thread;
mod history;

const PAGE: &str = "#171a1e";
const RAIL: &str = "#121417";
const HOVER: &str = "#252a30";
const SELECTED: &str = "#223438";
const INK: &str = "#e5eaee";
const MUTED: &str = "#a0abb5";
const ACCENT: &str = "#8fd8cf";
const RULE: &str = "#30363d";
const ADDED: &str = "#8cd4a5";
const REMOVED: &str = "#f2a5a3";

pub(crate) fn button(title: impl Into<String>) -> Button {
    Button::new(title).control_size(ControlSize::Regular)
}

fn color(hex: &str) -> Color {
    rune::color::hex(hex).expect("valid palette color")
}
fn label(value: impl Into<String>, size: f32, weight: f32, ink: &str) -> El {
    text(value.into())
        .font(TextStyle::new(size, weight))
        .color(color(ink))
}
fn divider() -> El {
    el().w_full().h(1.0).fill(color(RULE))
}
fn scroll_list(body: El) -> El {
    col()
        .min_h(0.0)
        .grow(1.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(body)
}
fn command(k: &Kit, name: &str, action: Action, disabled: bool) -> El {
    button(name)
        .variant(ButtonVariant::Outline)
        .disabled(disabled)
        .on_press(move |s: &mut Desk| s.act(action.clone()))
        .build(k)
}

fn brand() -> El {
    let mark = el()
        .size(30.0, 30.0)
        .radius(6.0)
        .fill(color("#315652"))
        .child(el().size(2.0, 15.0).absolute(9.0, 7.0).fill(Color::WHITE))
        .child(el().size(10.0, 2.0).absolute(10.0, 14.0).fill(Color::WHITE))
        .child(el().size(2.0, 7.0).absolute(19.0, 9.0).fill(Color::WHITE))
        .child(
            el().size(6.0, 6.0)
                .absolute(7.0, 5.0)
                .radius(3.0)
                .fill(Color::WHITE),
        )
        .child(
            el().size(6.0, 6.0)
                .absolute(7.0, 20.0)
                .radius(3.0)
                .fill(Color::WHITE),
        )
        .child(
            el().size(6.0, 6.0)
                .absolute(17.0, 6.0)
                .radius(3.0)
                .fill(Color::WHITE),
        );
    row()
        .h(30.0)
        .gap(8.0)
        .center_y()
        .child(mark)
        .child(label("Gitdesk", 16.0, 600.0, INK))
}

fn opener(s: &Desk, k: &Kit) -> El {
    let browse = button("Browse…")
        .disabled(s.busy)
        .on_press_tx(|s: &mut Desk, tx| {
            tx.open_folder(
                FileDialogOptions {
                    title: "Open a Git repository".into(),
                    directory: s.workspace.selected.clone(),
                    ..Default::default()
                },
                |s: &mut Desk, result, _| match result {
                    Ok(Some(path)) => s.open(path),
                    Ok(None) => {}
                    Err(e) => s.workspace.notice = e.to_string(),
                },
            );
        })
        .build(k)
        .key("browse");
    let mut actions = row().gap(8.0).child(
        button("Open repository")
            .variant(ButtonVariant::Contrast)
            .disabled(s.busy || s.path.text.trim().is_empty())
            .on_press(|s: &mut Desk| s.open_input())
            .build(k)
            .key("open-path"),
    );
    if s.workspace.selected.is_some() {
        actions = actions.child(
            button("Cancel")
                .disabled(s.busy)
                .on_press(|s: &mut Desk| {
                    s.opening = false;
                    s.workspace.notice.clear();
                })
                .build(k),
        );
    }
    let mut panel = col()
        .w(470.0)
        .padding(18.0)
        .gap(8.0)
        .fill(color(PAGE))
        .border(1.0, color(RULE))
        .radius(8.0)
        .child(brand())
        .child(divider())
        .child(label("Open a repository", 19.0, 600.0, INK))
        .child(label("Repository path", 13.0, 500.0, MUTED))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    TextInput::new(&s.path)
                        .height(28.0)
                        .label("Repository path")
                        .placeholder("Enter a local repository path")
                        .on_edit(|s: &mut Desk| &mut s.path)
                        .on_submit(|s: &mut Desk, _| s.open_input())
                        .disabled(s.busy)
                        .build(k)
                        .grow(1.0)
                        .key("repo-path"),
                )
                .child(browse),
        )
        .child(actions);
    if !s.workspace.notice.is_empty() {
        panel = panel.child(label(&s.workspace.notice, 13.0, 400.0, MUTED).wrap_anywhere());
    }
    col()
        .w_full()
        .h_full()
        .center()
        .fill(color(RAIL))
        .child(panel)
}

fn sidebar(s: &Desk, k: &Kit) -> El {
    let bare = s.workspace.snapshot.as_ref().is_some_and(|data| data.bare);
    let current = if bare && s.tab == Tab::Changes {
        Tab::History
    } else {
        s.tab
    };
    let mut rail = col()
        .w(156.0)
        .h_full()
        .padding(10.0)
        .gap(4.0)
        .fill(color(RAIL))
        .child(brand())
        .child(divider());
    for (title, tab) in [
        ("Changes", Tab::Changes),
        (
            "Diff",
            Tab::Diff(if s.selected_commit.is_some() {
                DiffMode::Commit
            } else if bare {
                DiffMode::Compare
            } else {
                DiffMode::WorkingTree
            }),
        ),
        ("History", Tab::History),
        ("Branches", Tab::Branches),
        ("Settings", Tab::Settings),
        ("GitHub", Tab::GitHub),
        ("Terminal", Tab::Terminal),
    ] {
        if bare && tab == Tab::Changes {
            continue;
        }
        if tab == Tab::Settings {
            rail = rail.child(divider());
        }
        let active = tab == current || matches!((tab, current), (Tab::Diff(_), Tab::Diff(_)));
        let mut item = row()
            .key(format!("tab-{title}"))
            .h(30.0)
            .px(6.0)
            .gap(7.0)
            .center_y()
            .role("button")
            .label(title)
            .focusable(!s.busy)
            .radius(5.0)
            .focus_ring(2.0, 0.0, color(ACCENT))
            .hover(color(HOVER))
            .fill(if active {
                color(SELECTED)
            } else {
                Color::TRANSPARENT
            })
            .child(el().w(3.0).h(16.0).radius(1.5).fill(if active {
                color(ACCENT)
            } else {
                Color::TRANSPARENT
            }))
            .child(label(
                title,
                13.0,
                if active { 600.0 } else { 400.0 },
                if active { INK } else { MUTED },
            ));
        if !s.busy {
            item = item.on_press(move |s: &mut Desk| s.select_tab(tab));
        }
        rail = rail.child(item);
    }
    rail.child(el().grow(1.0)).child(divider()).child(
        button("Open repository…")
            .disabled(s.busy)
            .on_press(|s: &mut Desk| {
                s.opening = true;
                s.workspace.notice.clear();
                s.path = rune::TextEdit::new("");
            })
            .build(k)
            .key("change-repository"),
    )
}

fn github(s: &Desk, k: &Kit) -> El {
    if let Some(thread) = &s.github_thread {
        return github_thread::view(s, thread, k);
    }
    let mut sections = row().gap(8.0);
    for section in crate::github::Section::ALL {
        sections = sections.child(
            button(section.label())
                .variant(if section == s.github_section {
                    ButtonVariant::Contrast
                } else {
                    ButtonVariant::Ghost
                })
                .disabled(s.busy)
                .on_press(move |s: &mut Desk| s.load_github(section))
                .build(k)
                .key(format!("github-{}", section.label())),
        );
    }

    let mut list = col().gap(8.0);
    if s.github_entries.is_empty() && !s.busy && s.workspace.notice.is_empty() {
        list = list.child(label(s.github_section.empty(), 14.0, 400.0, MUTED));
    }
    for entry in &s.github_entries {
        let title = entry.number.map_or_else(
            || entry.title.clone(),
            |number| format!("#{number}  {}", entry.title),
        );
        let mut text = col()
            .grow(1.0)
            .min_w(0.0)
            .gap(3.0)
            .child(label(title, 14.0, 500.0, INK).wrap_anywhere());
        let status = entry.status();
        let metadata = [
            entry.workflow_name.as_deref(),
            entry.head_branch.as_deref(),
            status.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        if !metadata.is_empty() {
            text = text.child(label(metadata, 12.0, 400.0, MUTED).wrap_anywhere());
        }
        let mut actions = row().gap(6.0);
        if let Some(number) = entry.number {
            actions = actions.child(
                button("View details")
                    .disabled(s.busy)
                    .on_press(move |s: &mut Desk| s.open_github_thread(number))
                    .build(k)
                    .key(format!("github-details-{number}")),
            );
        }
        list = list
            .child(
                row()
                    .key(format!("github-entry-{}", entry.url))
                    .gap(8.0)
                    .center_y()
                    .child(text)
                    .child(actions.child(open_on_github(k, entry.url.clone()))),
            )
            .child(divider());
    }

    col()
        .grow(1.0)
        .min_h(0.0)
        .gap(8.0)
        .child(sections)
        .child(divider())
        .child(scroll_list(list))
}

fn open_on_github(k: &Kit, url: String) -> El {
    button("Open on GitHub")
        .on_press_tx(move |s: &mut Desk, tx| {
            if let Err(error) = tx.open_url(&url) {
                s.workspace.notice = error.to_string();
            }
        })
        .build(k)
}

fn settings(s: &Desk, k: &Kit) -> El {
    let form = &s.config;
    let mut scopes = row().gap(6.0).center_y();
    for scope in [Scope::Local, Scope::Global, Scope::Worktree, Scope::System] {
        scopes = scopes.child(
            button(scope.label())
                .variant(if form.scope == scope {
                    ButtonVariant::Contrast
                } else {
                    ButtonVariant::Ghost
                })
                .disabled(s.busy)
                .on_press(move |s: &mut Desk| s.settings(scope, None))
                .build(k)
                .key(format!("scope-{}", scope.label())),
        );
    }
    let mut list = col().gap(4.0);
    for (i, (key, value)) in form.entries.iter().enumerate() {
        let (name, val) = (key.clone(), value.clone());
        let mut entry = row()
            .h(30.0)
            .gap(8.0)
            .px(6.0)
            .center_y()
            .role("button")
            .label(format!("Edit {key}"))
            .focusable(!s.busy)
            .key(format!("setting-{i}"))
            .hover(color(HOVER))
            .focus_ring(2.0, 0.0, color(ACCENT))
            .child(label(key, 13.0, 500.0, INK).grow(1.0).truncate_middle())
            .child(label(value, 13.0, 400.0, MUTED).grow(1.0).truncate());
        if !s.busy {
            entry = entry.on_press(move |s: &mut Desk| {
                s.config.key = rune::TextEdit::new(name.clone());
                s.config.value = rune::TextEdit::new(val.clone());
            });
        }
        list = list.child(entry);
    }
    if form.entries.is_empty() && !s.busy {
        list = list.child(label("No entries in this scope.", 13.0, 400.0, MUTED));
    }
    let key = TextInput::new(&form.key)
        .height(28.0)
        .label("Setting name")
        .placeholder("user.name")
        .on_edit(|s: &mut Desk| &mut s.config.key)
        .disabled(s.busy)
        .build(k)
        .key("config-key")
        .grow(1.0);
    let value = TextInput::new(&form.value)
        .height(28.0)
        .label("Setting value")
        .placeholder("Value")
        .on_edit(|s: &mut Desk| &mut s.config.value)
        .disabled(s.busy)
        .build(k)
        .key("config-value")
        .grow(1.0);
    let mut buttons = row().gap(8.0);
    for (title, mode) in [
        ("Save", ConfigEdit::Set),
        ("Add value", ConfigEdit::Add),
        ("Remove value", ConfigEdit::Remove),
    ] {
        buttons = buttons.child(
            button(title)
                .disabled(s.busy || form.key.text.trim().is_empty())
                .on_press(move |s: &mut Desk| s.settings(s.config.scope, Some(mode)))
                .build(k)
                .key(format!("config-{title}")),
        );
    }
    col()
        .grow(1.0)
        .min_h(0.0)
        .gap(8.0)
        .child(label("Git settings", 20.0, 600.0, INK))
        .child(scopes)
        .child(divider())
        .child(scroll_list(list))
        .child(divider())
        .child(row().gap(8.0).child(key).child(value))
        .child(buttons)
}

fn change_name(c: &Change, staged: bool) -> &'static str {
    if c.conflict() {
        return "Conflict";
    }
    match if staged { c.index } else { c.work } {
        b'?' => "Untracked",
        b'A' => "Added",
        b'D' => "Deleted",
        b'T' => "Type change",
        _ => "Modified",
    }
}

fn changes(s: &Desk, data: &Snapshot, k: &Kit) -> El {
    let mut checkbox_kit = k.clone();
    checkbox_kit.c.border_weak = color("#707d88");
    checkbox_kit.c.border_hover = color(MUTED);
    checkbox_kit.c.std_border = color(ACCENT);
    let mut files = col().gap(12.0);
    for staged in [false, true] {
        let selection = &s.selected_changes[usize::from(staged)];
        let section: Vec<_> = data
            .changes
            .iter()
            .filter(|c| if staged { c.staged() } else { c.unstaged() })
            .collect();
        let name = if staged { "Staged" } else { "Working tree" };
        let paths: Vec<_> = section.iter().map(|c| c.path.clone()).collect();
        let selected: Vec<_> = paths
            .iter()
            .filter(|path| selection.contains(*path))
            .cloned()
            .collect();
        let count = selected.len();
        let all = count > 0 && count == paths.len();
        let mixed = count > 0 && !all;
        let action = if staged { "Unstage" } else { "Stage" };
        let mut group = col().border(1.0, color(RULE)).clip_rounded(6.0).child(
            row()
                .h(42.0)
                .px(10.0)
                .center_y()
                .gap(10.0)
                .fill(color(HOVER))
                .child(
                    Checkbox::new(all)
                        .label(format!("Select all {} files", name.to_lowercase()))
                        .hide_label()
                        .indeterminate(mixed)
                        .icon(if mixed { k.glyph.dash } else { k.glyph.check })
                        .disabled(s.busy || paths.is_empty())
                        .on_toggle(move |s: &mut Desk| {
                            let selection = &mut s.selected_changes[usize::from(staged)];
                            selection.clear();
                            if !all {
                                selection.extend(paths.iter().cloned());
                            }
                        })
                        .build(&checkbox_kit)
                        .w(24.0)
                        .h(32.0)
                        .key(format!("select-all-{action}")),
                )
                .child(label(name, 15.0, 600.0, INK))
                .child(label(section.len().to_string(), 12.0, 500.0, MUTED))
                .child(el().grow(1.0))
                .child(label(
                    if count == 0 {
                        String::new()
                    } else {
                        format!("{count} selected")
                    },
                    12.0,
                    500.0,
                    MUTED,
                ))
                .child(
                    command(
                        k,
                        &format!("{action} selected"),
                        if staged {
                            Action::Unstage(selected, data.has_head)
                        } else {
                            Action::Stage(selected)
                        },
                        s.busy || count == 0,
                    )
                    .w(140.0)
                    .key(format!("{action}-selected")),
                ),
        );
        if section.is_empty() {
            let hint = if staged {
                "No changes staged."
            } else {
                "Working tree is clean."
            };
            group = group
                .child(divider())
                .child(col().padding(10.0).child(label(hint, 13.0, 400.0, MUTED)));
        }
        for c in section {
            let kind = change_name(c, staged);
            let action = if staged {
                Action::Unstage(vec![c.path.clone()], data.has_head)
            } else {
                Action::Stage(vec![c.path.clone()])
            };
            let ink = if staged {
                ADDED
            } else if c.conflict() {
                REMOVED
            } else {
                MUTED
            };
            let checked = selection.contains(&c.path);
            let path = c.path.clone();
            group = group.child(divider()).child(
                row()
                    .h(36.0)
                    .px(10.0)
                    .center_y()
                    .gap(10.0)
                    .fill(if checked {
                        color(SELECTED)
                    } else {
                        Color::TRANSPARENT
                    })
                    .child(
                        Checkbox::new(checked)
                            .label(format!(
                                "Select {} to {}",
                                c.path.to_string_lossy(),
                                action.label().to_lowercase()
                            ))
                            .hide_label()
                            .disabled(s.busy)
                            .on_toggle(move |s: &mut Desk| {
                                let selection = &mut s.selected_changes[usize::from(staged)];
                                if !selection.remove(&path) {
                                    selection.insert(path.clone());
                                }
                            })
                            .build(&checkbox_kit)
                            .w(24.0)
                            .h(32.0)
                            .key(format!(
                                "select-{}-{}",
                                action.label(),
                                c.path.to_string_lossy()
                            )),
                    )
                    .child(
                        label(c.path.to_string_lossy(), 13.0, 400.0, INK)
                            .grow(1.0)
                            .min_w(0.0)
                            .truncate_middle(),
                    )
                    .child(label(kind, 12.0, 500.0, ink).w(74.0))
                    .child(
                        command(k, action.label(), action.clone(), s.busy)
                            .w(84.0)
                            .key(format!("{}-{}", action.label(), c.path.to_string_lossy())),
                    ),
            );
        }
        files = files.child(group);
    }
    let input = TextInput::new(&s.draft)
        .height(32.0)
        .label("Commit message")
        .placeholder("Describe the staged changes")
        .on_edit(|s: &mut Desk| &mut s.draft)
        .disabled(s.busy)
        .build(k)
        .key("commit-message")
        .grow(1.0);
    let available = !s.busy
        && !s.draft.text.trim().is_empty()
        && data.changes.iter().any(Change::staged)
        && !data.changes.iter().any(Change::conflict);
    let commit = button("Commit staged")
        .variant(ButtonVariant::Contrast)
        .disabled(!available)
        .on_press(|s: &mut Desk| s.act(Action::Commit(s.draft.text.clone())))
        .build(k)
        .h(32.0)
        .key("commit");
    col()
        .min_h(0.0)
        .grow(1.0)
        .gap(8.0)
        .child(scroll_list(files))
        .child(divider())
        .child(row().gap(8.0).center_y().child(input).child(commit))
}

fn diff_controls(s: &Desk, bare: bool, mode: DiffMode, k: &Kit, layers: &mut Vec<El>) -> El {
    let mut modes = row().gap(8.0);
    for (title, choice) in [
        ("Working tree", DiffMode::WorkingTree),
        ("Staged", DiffMode::Staged),
        ("Compare branches", DiffMode::Compare),
        ("Commit", DiffMode::Commit),
    ] {
        if choice == DiffMode::Commit && s.selected_commit.is_none()
            || bare && matches!(choice, DiffMode::WorkingTree | DiffMode::Staged)
        {
            continue;
        }
        modes = modes.child(
            button(title)
                .variant(if mode == choice {
                    ButtonVariant::Contrast
                } else {
                    ButtonVariant::Ghost
                })
                .disabled(s.busy)
                .on_press(move |s: &mut Desk| s.select_tab(Tab::Diff(choice)))
                .build(k)
                .key(title),
        );
    }
    let controls = col().gap(8.0).child(modes);
    if mode == DiffMode::Commit
        && let Some(commit) = &s.selected_commit
    {
        return controls.child(
            row()
                .center_y()
                .gap(8.0)
                .child(label(&commit.short_id, 13.0, 600.0, ACCENT))
                .child(
                    label(&commit.title, 14.0, 500.0, INK)
                        .grow(1.0)
                        .min_w(0.0)
                        .truncate(),
                )
                .child(
                    button("Back to History")
                        .disabled(s.busy)
                        .on_press(|s: &mut Desk| s.select_tab(Tab::History))
                        .build(k)
                        .key("commit-back-to-history"),
                ),
        );
    }
    if mode != DiffMode::Compare {
        return controls;
    }
    let comparison = &s.comparison;
    let mut pickers = row().gap(8.0).center_y();
    for (index, (key, title)) in [
        ("base-branch", "Base branch"),
        ("compare-branch", "Compare branch"),
    ]
    .into_iter()
    .enumerate()
    {
        let picker = Select::new(key, &comparison.branches)
            .current(comparison.selected[index].as_deref())
            .placeholder(title)
            .label(title)
            .appearance(SelectAppearance::Field)
            .width(176.0)
            .disabled(s.busy || comparison.branches.is_empty())
            .open(comparison.picker == Some(index))
            .on_toggle(move |s: &mut Desk| s.comparison.picker = Some(index))
            .on_close(|s: &mut Desk| s.comparison.picker = None)
            .on_choose(move |s: &mut Desk, value, _| {
                s.comparison.selected[index] = Some(value.to_owned());
                s.comparison.picker = None;
                s.comparison.compared = false;
                s.workspace.notice.clear();
                s.patch.clear();
            })
            .build(k, layers);
        pickers = pickers.child(
            col()
                .gap(4.0)
                .child(label(title, 12.0, 500.0, MUTED))
                .child(picker),
        );
    }
    controls.child(
        pickers.child(
            button("Compare")
                .disabled(s.busy || comparison.selected.iter().any(Option::is_none))
                .on_press(|s: &mut Desk| s.compare_branches())
                .build(k)
                .key("run-branch-comparison"),
        ),
    )
}

fn detail(s: &Desk, data: &Snapshot, k: &Kit, layers: &mut Vec<El>) -> El {
    let mut body = col().gap(4.0);
    match s.tab {
        Tab::Settings => settings(s, k),
        Tab::GitHub => github(s, k),
        Tab::Terminal => crate::terminal_view::view(s, k),
        Tab::Changes if !data.bare => changes(s, data, k),
        Tab::Branches => {
            for branch in &data.branches {
                let current = branch == &data.branch;
                let mut line = row()
                    .h(30.0)
                    .center_y()
                    .child(label(branch, 14.0, 500.0, INK).grow(1.0).truncate());
                if current {
                    line = line.child(label("Current", 12.0, 500.0, ACCENT));
                } else if !data.bare {
                    line = line.child(command(k, "Switch", Action::Switch(branch.clone()), s.busy));
                }
                body = body.child(line).child(divider());
            }
            if data.branches.is_empty() {
                body = body.child(label(
                    "Local branches appear after the first commit.",
                    14.0,
                    400.0,
                    MUTED,
                ));
            }
            scroll_list(body)
        }
        Tab::Diff(mode) => {
            let empty = if s.busy || !s.workspace.notice.is_empty() {
                ""
            } else if mode == DiffMode::Compare && s.comparison.branches.is_empty() {
                "No local or remote-tracking branches found."
            } else if mode == DiffMode::Compare && !s.comparison.compared {
                "Select a base branch and a compare branch."
            } else if mode == DiffMode::Compare && s.patch.is_empty() {
                "No differences between the selected branch tips."
            } else if mode == DiffMode::Commit {
                "This commit changes no files."
            } else {
                "No diff for tracked files in this view."
            };
            col()
                .grow(1.0)
                .min_h(0.0)
                .gap(8.0)
                .child(diff_controls(s, data.bare, mode, k, layers))
                .child(diff::view(s, k, empty))
        }
        Tab::History | Tab::Changes => history::view(s, &data.history),
    }
}

pub fn view(s: &Desk, _: &mut Cx<Desk>) -> El {
    let mut k = Kit::neutral(true);
    k.c.border_focus = color(ACCENT);
    if s.opening {
        return opener(s, &k);
    }
    let w = &s.workspace;
    let mut layers = Vec::new();
    let title = w
        .selected
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Workspace".into());
    let refresh = button("Refresh")
        .disabled(s.busy)
        .on_press(|s: &mut Desk| {
            if s.tab == Tab::Diff(DiffMode::Compare) && s.comparison.compared {
                s.compare_branches();
            } else {
                s.select_tab(s.tab);
            }
        })
        .build(&k)
        .key("refresh");
    let mut toolbar = row().center_y().gap(6.0).child(
        label(title, 20.0, 600.0, INK)
            .grow(1.0)
            .min_w(0.0)
            .truncate(),
    );
    if let Some(data) = &w.snapshot {
        toolbar = toolbar.child(command(&k, "Fetch", Action::Fetch, s.busy));
        if !data.bare {
            toolbar = toolbar.child(command(&k, "Pull", Action::Pull, s.busy));
        }
        toolbar = toolbar.child(command(&k, "Push", Action::Push, s.busy));
    }
    let mut header = col().gap(4.0).child(toolbar.child(refresh));
    if let Some(path) = &w.selected {
        let mut metadata = row().center_y().gap(8.0).child(
            label(path.display().to_string(), 12.0, 400.0, MUTED)
                .grow(1.0)
                .min_w(0.0)
                .truncate_head(),
        );
        if let Some(data) = &w.snapshot {
            let branch = if data.bare {
                format!("Bare · {}", data.branch)
            } else {
                data.branch.clone()
            };
            metadata = metadata.child(label(branch, 12.0, 500.0, ACCENT).max_w(180.0).truncate());
        }
        header = header.child(metadata);
    }
    let mut main = col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .padding(12.0)
        .gap(8.0)
        .child(header)
        .child(divider());
    if let Some(data) = &w.snapshot {
        main = main.child(detail(s, data, &k, &mut layers));
    } else {
        let hint = "Repository could not be read.";
        if s.tab == Tab::Settings {
            main = main.child(settings(s, &k));
        } else if s.tab == Tab::GitHub {
            main = main.child(github(s, &k));
        } else if s.tab == Tab::Terminal {
            main = main.child(crate::terminal_view::view(s, &k));
        } else {
            main = main
                .child(label(hint, 15.0, 400.0, MUTED))
                .child(el().grow(1.0));
        }
    }
    if !w.notice.is_empty() {
        main = main.child(divider()).child(
            col()
                .max_h(68.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(
                    label(&w.notice, 12.0, 500.0, INK)
                        .wrap_anywhere()
                        .selectable(true),
                ),
        );
    }
    let body = row()
        .w_full()
        .h_full()
        .fill(color(PAGE))
        .child(sidebar(s, &k))
        .child(el().w(1.0).h_full().fill(color(RULE)))
        .child(main);
    root(body, layers)
}
