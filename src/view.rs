use super::{Desk, DiffMode, Tab};
use crate::git::{Action, Change, ConfigEdit, Scope, Snapshot};
use rune::{Color, Cx, El, FileDialogOptions, ScrollSpec, TextStyle, col, el, row, text};
use rune_kit::{Button, ButtonVariant, Kit, Select, SelectAppearance, TextInput, root};

const INK: &str = "#20283a";
const MUTED: &str = "#596579";
const ACCENT: &str = "#3856a3";
const RULE: &str = "#dce1e9";

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
    Button::new(name)
        .variant(ButtonVariant::Outline)
        .disabled(disabled)
        .on_press(move |s: &mut Desk| s.act(action.clone()))
        .build(k)
}

fn brand() -> El {
    let mark = el()
        .size(30.0, 30.0)
        .radius(8.0)
        .fill(color(ACCENT))
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
        .h(48.0)
        .gap(10.0)
        .center_y()
        .child(mark)
        .child(label("Gitdesk", 21.0, 600.0, INK))
}

fn opener(s: &Desk, k: &Kit) -> El {
    let browse = Button::new("Browse…")
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
    let mut actions = row().gap(10.0).child(
        Button::new("Open repository")
            .variant(ButtonVariant::Contrast)
            .disabled(s.busy || s.path.text.trim().is_empty())
            .on_press(|s: &mut Desk| s.open_input())
            .build(k)
            .key("open-path"),
    );
    if s.workspace.selected.is_some() {
        actions = actions.child(
            Button::new("Cancel")
                .disabled(s.busy)
                .on_press(|s: &mut Desk| {
                    s.opening = false;
                    s.workspace.notice.clear();
                })
                .build(k),
        );
    }
    let mut panel = col()
        .w(560.0)
        .padding(32.0)
        .gap(16.0)
        .fill(Color::WHITE)
        .border(1.0, color(RULE))
        .radius(10.0)
        .child(brand())
        .child(divider())
        .child(label("Open a repository", 27.0, 600.0, INK))
        .child(label("Repository path", 13.0, 500.0, MUTED))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    TextInput::new(&s.path)
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
        .fill(color("#f1f4f8"))
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
        .w(190.0)
        .h_full()
        .padding(16.0)
        .gap(8.0)
        .fill(color("#f1f4f8"))
        .child(brand())
        .child(divider());
    for (title, tab) in [
        ("Changes", Tab::Changes),
        (
            "Diff",
            Tab::Diff(if bare {
                DiffMode::Compare
            } else {
                DiffMode::WorkingTree
            }),
        ),
        ("History", Tab::History),
        ("Branches", Tab::Branches),
        ("Settings", Tab::Settings),
        ("Commands", Tab::Commands),
        ("GitHub CLI", Tab::GitHub),
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
            .h(36.0)
            .px(10.0)
            .gap(9.0)
            .center_y()
            .role("button")
            .label(title)
            .focusable(!s.busy)
            .radius(5.0)
            .focus_ring(2.0, 0.0, color(ACCENT))
            .hover(color("#e3e8f3"))
            .fill(if active {
                color("#dbe5fb")
            } else {
                Color::TRANSPARENT
            })
            .child(el().w(3.0).h(16.0).radius(1.5).fill(if active {
                color(ACCENT)
            } else {
                Color::TRANSPARENT
            }))
            .child(label(title, 14.0, if active { 600.0 } else { 400.0 }, INK));
        if !s.busy {
            item = item.on_press(move |s: &mut Desk| s.select_tab(tab));
        }
        rail = rail.child(item);
    }
    rail.child(el().grow(1.0)).child(divider()).child(
        Button::new("Open repository…")
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

fn commands(s: &Desk, k: &Kit) -> El {
    if s.shell().is_some() {
        return crate::terminal_view::view(s, k);
    }
    let github = s.tab == Tab::GitHub;
    let program = s.tab.command_program().unwrap();
    let args = s.command_input().text.trim_start();
    let query = args.split_whitespace().next().unwrap_or("");
    let mut list = col().gap(2.0);
    let shortcuts = [
        "auth",
        "pr",
        "issue",
        "repo",
        "run",
        "workflow",
        "release",
        "api",
        "alias",
        "extension",
    ];
    let command_names: Vec<&str> = if github {
        shortcuts.to_vec()
    } else {
        s.catalog.iter().map(String::as_str).collect()
    };
    for name in command_names.into_iter().filter(|n| n.contains(query)) {
        let choice = name.to_owned();
        list = list.child(
            Button::new(name)
                .variant(ButtonVariant::Ghost)
                .disabled(s.busy)
                .on_press_tx(move |s: &mut Desk, tx| {
                    let prefix = format!("{} ", shell_words::quote(&choice));
                    *s.command_input_mut() = rune::TextEdit::new(prefix);
                    tx.focus("input", false);
                })
                .build(k)
                .key(format!("choose-{name}")),
        );
    }
    let input = TextInput::new(s.command_input())
        .label(if github {
            "GitHub CLI arguments"
        } else {
            "Git arguments"
        })
        .placeholder(if github { "pr list" } else { "status --short" })
        .on_edit(|s: &mut Desk| s.command_input_mut())
        .on_submit(|s: &mut Desk, _| s.run_command())
        .disabled(s.busy)
        .build(k)
        .key("git-arguments")
        .grow(1.0);
    col()
        .grow(1.0)
        .min_h(0.0)
        .gap(12.0)
        .child(label(
            if github { "GitHub CLI" } else { "Git commands" },
            20.0,
            600.0,
            INK,
        ))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(label(program, 14.0, 600.0, INK))
                .child(input)
                .child(
                    Button::new("Run")
                        .variant(ButtonVariant::Contrast)
                        .disabled(s.busy || args.is_empty())
                        .on_press(|s: &mut Desk| s.run_command())
                        .build(k)
                        .key("run-command"),
                ),
        )
        .child(label(
            if github {
                "Common commands"
            } else {
                "Installed commands and aliases"
            },
            13.0,
            500.0,
            MUTED,
        ))
        .child(divider())
        .child(scroll_list(list))
}

fn settings(s: &Desk, k: &Kit) -> El {
    let form = &s.config;
    let mut scopes = row().gap(6.0).center_y();
    for scope in [Scope::Local, Scope::Global, Scope::Worktree, Scope::System] {
        scopes = scopes.child(
            Button::new(scope.label())
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
            .gap(12.0)
            .px(6.0)
            .center_y()
            .role("button")
            .label(format!("Edit {key}"))
            .focusable(!s.busy)
            .key(format!("setting-{i}"))
            .hover(color("#edf1f8"))
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
        .label("Setting name")
        .placeholder("user.name")
        .on_edit(|s: &mut Desk| &mut s.config.key)
        .disabled(s.busy)
        .build(k)
        .key("config-key")
        .grow(1.0);
    let value = TextInput::new(&form.value)
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
            Button::new(title)
                .disabled(s.busy || form.key.text.trim().is_empty())
                .on_press(move |s: &mut Desk| s.settings(s.config.scope, Some(mode)))
                .build(k)
                .key(format!("config-{title}")),
        );
    }
    col()
        .grow(1.0)
        .min_h(0.0)
        .gap(10.0)
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
    let mut files = col().gap(8.0);
    for staged in [false, true] {
        let section: Vec<_> = data
            .changes
            .iter()
            .filter(|c| if staged { c.staged() } else { c.unstaged() })
            .collect();
        let name = if staged { "Staged" } else { "Working tree" };
        files = files.child(
            row()
                .h(40.0)
                .center_y()
                .gap(8.0)
                .child(label(name, 16.0, 600.0, INK))
                .child(label(section.len().to_string(), 13.0, 500.0, MUTED)),
        );
        if section.is_empty() {
            let hint = if staged {
                "No changes staged."
            } else {
                "Working tree is clean."
            };
            files = files
                .child(label(hint, 13.0, 400.0, MUTED))
                .child(el().h(8.0));
        }
        for c in section {
            let kind = change_name(c, staged);
            let action = if staged {
                Action::Unstage(c.path.clone(), data.has_head)
            } else {
                Action::Stage(c.path.clone())
            };
            let ink = if staged {
                "#207057"
            } else if c.conflict() {
                "#a13936"
            } else {
                MUTED
            };
            files = files.child(
                row()
                    .h(34.0)
                    .center_y()
                    .gap(8.0)
                    .child(label(kind, 12.0, 500.0, ink).w(74.0))
                    .child(
                        label(c.path.to_string_lossy(), 13.0, 400.0, INK)
                            .grow(1.0)
                            .truncate_middle(),
                    )
                    .child(
                        command(k, action.label(), action.clone(), s.busy).key(format!(
                            "{}-{}",
                            action.label(),
                            c.path.to_string_lossy()
                        )),
                    ),
            );
        }
        files = files.child(divider());
    }
    let input = TextInput::new(&s.draft)
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
    let commit = Button::new("Commit staged")
        .variant(ButtonVariant::Contrast)
        .disabled(!available)
        .on_press(|s: &mut Desk| s.act(Action::Commit(s.draft.text.clone())))
        .build(k)
        .key("commit");
    col()
        .min_h(0.0)
        .grow(1.0)
        .gap(12.0)
        .child(scroll_list(files))
        .child(row().gap(10.0).center_y().child(input).child(commit))
}

fn diff_controls(s: &Desk, bare: bool, mode: DiffMode, k: &Kit, layers: &mut Vec<El>) -> El {
    let mut modes = row().gap(8.0);
    for (title, choice) in [
        ("Working tree", DiffMode::WorkingTree),
        ("Staged", DiffMode::Staged),
        ("Compare branches", DiffMode::Compare),
    ] {
        if bare && choice != DiffMode::Compare {
            continue;
        }
        modes = modes.child(
            Button::new(title)
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
    let controls = col().gap(10.0).child(modes);
    if mode != DiffMode::Compare {
        return controls;
    }
    let comparison = &s.comparison;
    let mut pickers = row().gap(10.0).center_y();
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
            .width(190.0)
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
            Button::new("Compare")
                .disabled(s.busy || comparison.selected.iter().any(Option::is_none))
                .on_press(|s: &mut Desk| s.compare_branches())
                .build(k)
                .key("run-branch-comparison"),
        ),
    )
}

fn detail(s: &Desk, data: &Snapshot, k: &Kit, layers: &mut Vec<El>) -> El {
    let mut body = col().gap(7.0);
    match s.tab {
        Tab::Settings => settings(s, k),
        Tab::Commands | Tab::GitHub => commands(s, k),
        Tab::Terminal => crate::terminal_view::view(s, k),
        Tab::Changes if !data.bare => changes(s, data, k),
        Tab::Branches => {
            for branch in &data.branches {
                let current = branch == &data.branch;
                let mut line = row()
                    .h(36.0)
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
            let content = if s.busy {
                ""
            } else if mode == DiffMode::Compare && !s.workspace.notice.is_empty() {
                ""
            } else if mode == DiffMode::Compare && s.comparison.branches.is_empty() {
                "No local or remote-tracking branches found."
            } else if mode == DiffMode::Compare && !s.comparison.compared {
                "Select a base branch and a compare branch."
            } else if mode == DiffMode::Compare && s.patch.is_empty() {
                "No differences between the selected branch tips."
            } else if s.patch.is_empty() {
                "No diff for tracked files in this view."
            } else {
                &s.patch
            };
            for line in content.lines().take(600) {
                let ink = if line.starts_with('+') {
                    "#21664e"
                } else if line.starts_with('-') {
                    "#9a373d"
                } else {
                    INK
                };
                body = body.child(
                    label(line, 13.0, 400.0, ink)
                        .wrap_anywhere()
                        .selectable(true),
                );
            }
            if content.lines().nth(600).is_some() {
                body = body.child(label("Showing the first 600 lines.", 12.0, 500.0, MUTED));
            }
            col()
                .grow(1.0)
                .min_h(0.0)
                .gap(10.0)
                .child(diff_controls(s, data.bare, mode, k, layers))
                .child(scroll_list(body))
        }
        _ => {
            if data.history.is_empty() {
                body = body.child(label("No commits yet.", 14.0, 400.0, MUTED));
            }
            for (id, title) in &data.history {
                body = body
                    .child(
                        row()
                            .h(32.0)
                            .gap(14.0)
                            .center_y()
                            .child(label(id, 13.0, 500.0, ACCENT).w(82.0))
                            .child(label(title, 14.0, 400.0, INK).grow(1.0).truncate()),
                    )
                    .child(divider());
            }
            scroll_list(body)
        }
    }
}

pub fn view(s: &Desk, _: &mut Cx<Desk>) -> El {
    let k = Kit::neutral(false);
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
    let refresh = Button::new("Refresh")
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
    let mut main = col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .padding(24.0)
        .gap(14.0)
        .child(
            row()
                .center_y()
                .gap(12.0)
                .child(label(title, 27.0, 600.0, INK).grow(1.0).truncate())
                .child(refresh),
        );
    if let Some(path) = &w.selected {
        main = main.child(label(path.display().to_string(), 12.0, 400.0, MUTED).truncate_head());
    }
    if let Some(data) = &w.snapshot {
        let meta = if data.bare {
            format!("Bare repository   /   {}", data.branch)
        } else {
            format!("Branch   /   {}", data.branch)
        };
        let mut toolbar = row()
            .gap(6.0)
            .center_y()
            .child(label(meta, 13.0, 500.0, ACCENT).grow(1.0).truncate())
            .child(command(&k, "Fetch", Action::Fetch, s.busy));
        if !data.bare {
            toolbar = toolbar.child(command(&k, "Pull", Action::Pull, s.busy));
        }
        main = main
            .child(toolbar.child(command(&k, "Push", Action::Push, s.busy)))
            .child(divider())
            .child(detail(s, data, &k, &mut layers));
    } else {
        let hint = "Repository could not be read.";
        if s.tab == Tab::Settings {
            main = main.child(settings(s, &k));
        } else if s.tab.command_program().is_some() {
            main = main.child(commands(s, &k));
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
        .fill(Color::WHITE)
        .child(sidebar(s, &k))
        .child(el().w(1.0).h_full().fill(color(RULE)))
        .child(main);
    root(body, layers)
}
