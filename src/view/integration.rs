use super::*;
use crate::Update;
use crate::git::{Integration, IntegrationMode, Operation, Recovery};

impl Desk {
    pub(super) fn preview_integration(&mut self, source: String, mode: IntegrationMode) {
        let Some(repo) = self.workspace.selected.clone() else {
            return;
        };
        self.branch_prompt = None;
        self.start("Comparing branch tips…", move || {
            Update::IntegrationPreview(Integration::preview(&repo, source, mode))
        });
    }
}

pub(super) fn preview(s: &Desk, data: &Snapshot, plan: &Integration, k: &Kit) -> El {
    use IntegrationMode::*;
    let source = plan
        .source
        .strip_prefix("refs/heads/")
        .or_else(|| plan.source.strip_prefix("refs/remotes/"))
        .unwrap_or(&plan.source);
    let rebase = plan.mode.is_rebase();
    let title = if rebase {
        format!("Rebase {} onto {source}", data.branch)
    } else {
        format!("Merge {source} into {}", data.branch)
    };
    let mut body = col()
        .gap(10.0)
        .child(heading(&title, 20.0).wrap_anywhere())
        .child(
            label(
                format!(
                    "{} commits unique to this checkout · {} unique to {source}",
                    plan.ours, plan.theirs
                ),
                14.0,
                500.0,
                MUTED,
            )
            .wrap_anywhere(),
        );
    let options: &[_] = if rebase {
        &[("Linear", Rebase), ("Preserve merges", RebaseMerges)]
    } else {
        &[
            ("Git default", Merge),
            ("Fast-forward only", FastForward),
            ("Merge commit", MergeCommit),
        ]
    };
    let mut choices = row().gap(6.0);
    for &(name, mode) in options {
        choices = choices.child(
            button(name)
                .variant(if plan.mode == mode {
                    ButtonVariant::Contrast
                } else {
                    ButtonVariant::Neutral
                })
                .disabled(s.busy)
                .on_press(move |s: &mut Desk| {
                    if let Some(plan) = &mut s.integration {
                        plan.mode = mode;
                    }
                })
                .build(k),
        );
    }
    let description = match plan.mode {
        Merge => "Use this repository’s merge configuration.",
        FastForward if plan.ours > 0 && plan.theirs > 0 => {
            "The branches have diverged. A fast-forward is unavailable."
        }
        FastForward => "Advance the current branch only when its history can fast-forward.",
        MergeCommit => "Create a merge commit even when the current branch could fast-forward.",
        Rebase => {
            "Replay the current branch’s commits onto the source tip. Replayed commits receive new IDs."
        }
        RebaseMerges => {
            "Replay the current branch and recreate its merge topology. Replayed commits receive new IDs."
        }
    };
    body = body
        .child(choices)
        .child(label(description, 14.0, 400.0, INK).wrap_anywhere());
    let disabled = s.busy
        || plan.checkout != data.checkout
        || data.operation.is_some()
        || (plan.mode == FastForward && plan.ours > 0 && plan.theirs > 0);
    scroll_list(
        body.child(
            row()
                .gap(8.0)
                .child(
                    command(
                        k,
                        plan.mode.label(),
                        Action::Integrate(plan.clone()),
                        disabled,
                    )
                    .key("confirm-integration"),
                )
                .child(
                    button("Cancel")
                        .disabled(s.busy)
                        .on_press(|s: &mut Desk| s.integration = None)
                        .build(k),
                ),
        ),
    )
}

pub(super) fn recovery(s: &Desk, data: &Snapshot, op: Operation, k: &Kit) -> El {
    let conflicts = data.changes.iter().filter(|c| c.conflict()).count();
    let mut pane = col()
        .gap(5.0)
        .px(10.0)
        .py(6.0)
        .radius(6.0)
        .fill(color(RAIL))
        .child(label(
            format!("Git {} in progress", op.command()),
            15.0,
            600.0,
            INK,
        ));
    if s.tab != Tab::Changes {
        return pane.child(
            button("Review in Changes")
                .disabled(s.busy)
                .on_press(|s: &mut Desk| s.select_tab(Tab::Changes))
                .build(k),
        );
    }
    if let Some((expected, recovery)) = s.recovery
        && op == expected
    {
        let message = if recovery == Recovery::Abort {
            format!(
                "Abort this {} and discard its in-progress resolution?",
                op.command()
            )
        } else {
            "Discard this commit’s changes and advance to the next step?".to_owned()
        };
        return pane
            .child(label(message, 13.0, 500.0, INK).wrap_anywhere())
            .child(
                row()
                    .gap(6.0)
                    .child(
                        command(k, recovery.label(), Action::Recover(op, recovery), s.busy)
                            .key("confirm-recovery"),
                    )
                    .child(
                        button("Cancel")
                            .disabled(s.busy)
                            .on_press(|s: &mut Desk| s.recovery = None)
                            .build(k),
                    ),
            );
    }
    let message = if conflicts == 0 {
        "Review your staged changes, then continue.".to_owned()
    } else {
        format!("{conflicts} conflicted paths. Edit the files, then stage each resolution below.")
    };
    pane = pane.child(label(message, 13.0, 400.0, MUTED).wrap_anywhere());
    let mut controls = row().gap(6.0).child(
        command(
            k,
            "Continue",
            Action::Recover(op, Recovery::Continue),
            s.busy || conflicts > 0,
        )
        .key("continue-operation"),
    );
    for recovery in [Recovery::Skip, Recovery::Abort] {
        if recovery == Recovery::Skip && op == Operation::Merge {
            continue;
        }
        controls = controls.child(
            button(recovery.label())
                .variant(ButtonVariant::Ghost)
                .disabled(s.busy)
                .on_press(move |s: &mut Desk| s.recovery = Some((op, recovery)))
                .build(k)
                .key(recovery.label()),
        );
    }
    pane.child(controls)
}
