use super::*;
use crate::git::{Checkout, Commit, HistoryAction, ResetMode};
use rune::TextEdit;

pub(crate) struct HistoryForm {
    checkout: Checkout,
    commit: Commit,
    action: HistoryAction,
    message: TextEdit,
}

pub(super) fn menu(s: &Desk, bare: bool, k: &Kit, layers: &mut Vec<El>) -> El {
    let trigger = button("Actions")
        .icon(k.glyph.chevron_down)
        .disabled(s.busy)
        .on_press(|s: &mut Desk| s.history_menu = !s.history_menu)
        .build(k)
        .key("commit-actions");
    let mut menu = Menu::new("history-actions")
        .trigger(trigger)
        .open(s.history_menu)
        .on_close(|s: &mut Desk| s.history_menu = false)
        .placement(Placement::BottomEnd)
        .item(MenuItem::new("Tag this commit…").on_select(|s: &mut Desk| {
            if let Some(commit) = &s.selected_commit {
                s.tags.target = TextEdit::new(&commit.id);
                s.select_tab(Tab::Tags);
            }
        }));
    if !bare
        && s.workspace
            .snapshot
            .as_ref()
            .is_some_and(|d| d.operation.is_none() && d.has_head)
    {
        for action in [
            HistoryAction::Revert { mainline: None },
            HistoryAction::Reset(ResetMode::Soft),
            HistoryAction::Squash(String::new()),
        ] {
            menu = menu.item(MenuItem::new(format!("{}…", action.label())).on_select(
                move |s: &mut Desk| {
                    if let (Some(data), Some(commit)) = (&s.workspace.snapshot, &s.selected_commit)
                    {
                        s.history_form = Some(HistoryForm {
                            checkout: data.checkout.clone(),
                            commit: commit.clone(),
                            action: action.clone(),
                            message: TextEdit::new(&commit.title),
                        });
                        s.history_menu = false;
                        s.workspace.notice.clear();
                    }
                },
            ));
        }
    }
    menu.build(k, layers)
}

pub(super) fn form(s: &Desk, data: &Snapshot, form: &HistoryForm, k: &Kit) -> El {
    let commit = &form.commit;
    let title = form.action.label();
    let mut body = col().gap(10.0).child(heading(title, 20.0)).child(
        label(
            format!("{} · {}  {}", data.branch, commit.short_id, commit.title),
            14.0,
            600.0,
            INK,
        )
        .wrap_anywhere(),
    );
    let mut ready = !s.busy && data.checkout == form.checkout && data.operation.is_none();
    let mut action = form.action.clone();
    match &form.action {
        HistoryAction::Revert { mainline } => {
            body = body.child(
                label(
                    "Create a new commit that reverses this commit’s changes.",
                    14.0,
                    400.0,
                    MUTED,
                )
                .wrap_anywhere(),
            );
            if commit.parents.len() > 1 {
                ready &= mainline.is_some();
                body = body.child(
                    label(
                        "Choose the mainline parent whose side of the merge to keep.",
                        14.0,
                        500.0,
                        INK,
                    )
                    .wrap_anywhere(),
                );
                for (i, parent) in commit.parents.iter().enumerate() {
                    let n = i + 1;
                    body = body.child(
                        button(format!("Parent {n} · {}", &parent[..8]))
                            .variant(if *mainline == Some(n) {
                                ButtonVariant::Contrast
                            } else {
                                ButtonVariant::Neutral
                            })
                            .disabled(s.busy)
                            .on_press(move |s: &mut Desk| {
                                if let Some(f) = &mut s.history_form {
                                    f.action = HistoryAction::Revert { mainline: Some(n) };
                                }
                            })
                            .build(k),
                    );
                }
            }
        }
        HistoryAction::Reset(selected) => {
            let mut choices = row().gap(6.0);
            for (name, mode) in [
                ("Soft", ResetMode::Soft),
                ("Mixed", ResetMode::Mixed),
                ("Hard", ResetMode::Hard),
            ] {
                choices = choices.child(
                    button(name)
                        .variant(if *selected == mode {
                            ButtonVariant::Contrast
                        } else {
                            ButtonVariant::Neutral
                        })
                        .disabled(s.busy)
                        .on_press(move |s: &mut Desk| {
                            if let Some(f) = &mut s.history_form {
                                f.action = HistoryAction::Reset(mode);
                            }
                        })
                        .build(k),
                );
            }
            let detail = match selected {
                ResetMode::Soft => {
                    "Move HEAD to this commit. Keep the index and working files as they are."
                }
                ResetMode::Mixed => {
                    "Move HEAD and reset the index. Working files stay in place and staged changes become unstaged."
                }
                ResetMode::Hard => {
                    "Move HEAD and replace the index and tracked files. Uncommitted tracked changes and untracked files obstructing checkout can be lost."
                }
            };
            body = body.child(choices).child(
                label(
                    detail,
                    14.0,
                    500.0,
                    if *selected == ResetMode::Hard {
                        REMOVED
                    } else {
                        MUTED
                    },
                )
                .wrap_anywhere(),
            );
        }
        HistoryAction::Squash(_) => {
            ready &= !form.message.text.trim().is_empty()
                && form.checkout.id.as_deref() != Some(&commit.id);
            body = body.child(label(format!("Replace the linear range from {} through HEAD with one commit. This rewrites commit IDs and preserves your staged and unstaged changes.", commit.short_id), 14.0, 400.0, MUTED).wrap_anywhere())
                .child(TextInput::new(&form.message).label("Squash commit message").placeholder("Squash commit message")
                    .disabled(s.busy).on_edit(|s: &mut Desk| &mut s.history_form.as_mut().unwrap().message)
                    .build(k).key("squash-message"));
            action = HistoryAction::Squash(form.message.text.clone());
        }
    }
    scroll_list(
        body.child(
            row()
                .gap(8.0)
                .child(
                    command(
                        k,
                        title,
                        Action::History {
                            checkout: form.checkout.clone(),
                            target: commit.id.clone(),
                            action,
                        },
                        !ready,
                    )
                    .key("confirm-history-action"),
                )
                .child(
                    button("Cancel")
                        .disabled(s.busy)
                        .on_press(|s: &mut Desk| s.history_form = None)
                        .build(k),
                ),
        ),
    )
}
