use super::*;
use crate::git::WorktreeMode;

pub(super) fn view(s: &Desk, data: &Snapshot, k: &Kit, layers: &mut Vec<El>) -> El {
    let form = &s.worktrees;
    let mut modes = row().gap(4.0);
    for (name, mode) in [
        ("New branch", WorktreeMode::NewBranch),
        ("Existing branch", WorktreeMode::Existing),
        ("Detached", WorktreeMode::Detached),
    ] {
        modes = modes.child(
            button(name)
                .variant(if form.mode == mode {
                    ButtonVariant::Contrast
                } else {
                    ButtonVariant::Ghost
                })
                .on_press(move |s: &mut Desk| {
                    s.worktrees.mode = mode;
                    s.worktrees.picker = false;
                })
                .disabled(s.busy)
                .build(k),
        );
    }
    let mut body = col()
        .gap(8.0)
        .child(heading("Worktrees", 20.0))
        .child(modes)
        .child(
            TextInput::new(&form.path)
                .label("Worktree folder")
                .placeholder("Folder path (absolute or relative to this repository)")
                .on_edit(|s: &mut Desk| &mut s.worktrees.path)
                .disabled(s.busy)
                .build(k)
                .key("worktree-path"),
        );
    let mut fields = row().gap(8.0);
    let mut branch_ready = true;
    if form.mode == WorktreeMode::NewBranch {
        branch_ready = !form.branch.text.trim().is_empty();
        let title = "New branch name";
        fields = fields.child(
            TextInput::new(&form.branch)
                .label(title)
                .placeholder(title)
                .on_edit(|s: &mut Desk| &mut s.worktrees.branch)
                .disabled(s.busy)
                .build(k)
                .grow(1.0)
                .key("worktree-branch"),
        );
    } else if form.mode == WorktreeMode::Existing {
        let branches: Vec<_> = data
            .branches
            .iter()
            .filter(|b| !data.worktrees.iter().any(|t| t.branch.as_ref() == Some(*b)))
            .cloned()
            .collect();
        let selected = form.existing.as_ref().filter(|b| branches.contains(b));
        branch_ready = selected.is_some();
        fields = fields.child(
            Select::new("worktree-existing", &branches)
                .current(selected.map(String::as_str))
                .placeholder(if branches.is_empty() {
                    "No available branches"
                } else {
                    "Choose local branch"
                })
                .label("Existing local branch")
                .appearance(SelectAppearance::Field)
                .width(208.0)
                .disabled(s.busy || branches.is_empty())
                .open(form.picker)
                .on_toggle(|s: &mut Desk| s.worktrees.picker = true)
                .on_close(|s: &mut Desk| s.worktrees.picker = false)
                .on_choose(|s: &mut Desk, name, _| {
                    s.worktrees.existing = Some(name.into());
                    s.worktrees.picker = false;
                })
                .build(k, layers),
        );
    }
    if form.mode != WorktreeMode::Existing {
        fields = fields.child(
            TextInput::new(&form.start)
                .label("Starting revision")
                .placeholder("Start at (HEAD)")
                .on_edit(|s: &mut Desk| &mut s.worktrees.start)
                .disabled(s.busy)
                .build(k)
                .grow(1.0)
                .key("worktree-start"),
        );
    }
    body = body
        .child(
            fields.child(
                button("Add worktree")
                    .disabled(s.busy || form.path.text.trim().is_empty() || !branch_ready)
                    .on_press(|s: &mut Desk| {
                        s.act(Action::AddWorktree {
                            path: s.worktrees.path.text.trim().into(),
                            mode: s.worktrees.mode,
                            branch: if s.worktrees.mode == WorktreeMode::Existing {
                                s.worktrees.existing.clone().unwrap_or_default()
                            } else {
                                s.worktrees.branch.text.trim().into()
                            },
                            start: s.worktrees.start.text.trim().into(),
                        })
                    })
                    .build(k)
                    .key("add-worktree"),
            ),
        )
        .child(divider());
    if let Some(path) = &form.removing {
        return col().gap(8.0).child(heading("Remove worktree", 20.0)).child(label(format!("Remove {} and its directory, including ignored files? Git will check for uncommitted changes.", path.display()), 14.0, 500.0, INK).wrap_anywhere())
            .child(row().gap(8.0).child(command(k, "Remove worktree", Action::RemoveWorktree(path.clone()), s.busy))
            .child(button("Cancel").disabled(s.busy).on_press(|s: &mut Desk| s.worktrees.removing = None).build(k)));
    }
    for tree in &data.worktrees {
        let name = tree.branch.as_deref().unwrap_or(if tree.bare {
            "Bare repository"
        } else {
            "Detached HEAD"
        });
        let mut title = row()
            .gap(8.0)
            .center_y()
            .child(label(name, 15.0, 600.0, INK).min_w(0.0).truncate());
        if tree.current {
            title = title.child(branch_badge("Current"));
        }
        if tree.main {
            title = title.child(branch_badge("Main worktree"));
        }
        if !tree.head.is_empty() {
            title = title.child(label(
                tree.head.chars().take(8).collect::<String>(),
                12.0,
                400.0,
                MUTED,
            ));
        }
        body = body.child(title).child(
            label(tree.path.display().to_string(), 13.0, 400.0, MUTED)
                .wrap_anywhere()
                .selectable(true),
        );
        for (status, reason) in [("Locked", &tree.locked), ("Missing", &tree.prunable)] {
            if let Some(reason) = reason {
                body = body.child(
                    label(format!("{status} {reason}").trim(), 12.0, 500.0, MUTED).wrap_anywhere(),
                );
            }
        }
        let mut controls = row().gap(6.0);
        if !tree.current && tree.prunable.is_none() {
            let path = tree.path.clone();
            controls = controls.child(
                button("Open")
                    .disabled(s.busy)
                    .on_press(move |s: &mut Desk| s.open(path.clone()))
                    .build(k),
            );
        }
        if !tree.main {
            let lock = tree.locked.is_none();
            controls = controls.child(command(
                k,
                if lock { "Lock" } else { "Unlock" },
                Action::LockWorktree(tree.path.clone(), lock),
                s.busy,
            ));
            if !tree.current {
                let path = tree.path.clone();
                controls = controls.child(
                    button("Remove…")
                        .disabled(s.busy || !lock)
                        .on_press(move |s: &mut Desk| s.worktrees.removing = Some(path.clone()))
                        .build(k),
                );
            }
        }
        body = body.child(controls).child(divider());
    }
    scroll_list(body)
}
