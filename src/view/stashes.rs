use super::{Desk, INK, MUTED, button, command, divider, label, scroll_list};
use crate::git::{Action, Change, Snapshot};
use rune::{El, col, row};
use rune_kit::{ButtonVariant, Checkbox, Kit, TextInput};

pub(super) fn view(s: &Desk, data: &Snapshot, k: &Kit) -> El {
    let include_untracked = s.stash.include_untracked;
    let can_stash = data.has_head
        && !data.changes.iter().any(Change::conflict)
        && data
            .changes
            .iter()
            .any(|change| include_untracked || change.index != b'?');
    let create = button("Stash changes")
        .variant(ButtonVariant::Contrast)
        .disabled(s.busy || !can_stash)
        .on_press(|s: &mut Desk| {
            s.act(Action::Stash(
                s.stash.message.text.clone(),
                s.stash.include_untracked,
            ));
        })
        .build(k)
        .h(32.0)
        .key("create-stash");
    let message = TextInput::new(&s.stash.message)
        .height(32.0)
        .label("Stash message")
        .placeholder("Stash message (optional)")
        .disabled(s.busy || !data.has_head)
        .on_edit(|s: &mut Desk| &mut s.stash.message)
        .build(k)
        .grow(1.0)
        .min_w(0.0)
        .key("stash-message");
    let mut form = col()
        .gap(8.0)
        .child(row().gap(8.0).center_y().child(message).child(create))
        .child(
            Checkbox::new(include_untracked)
                .label("Include untracked files")
                .disabled(s.busy || !data.has_head)
                .on_toggle(|s: &mut Desk| s.stash.include_untracked = !s.stash.include_untracked)
                .build(k)
                .key("stash-include-untracked"),
        );
    if !data.has_head {
        form = form.child(label(
            "Make an initial commit before stashing changes.",
            13.0,
            400.0,
            MUTED,
        ));
    } else if data.changes.iter().any(Change::conflict) {
        form = form.child(label(
            "Resolve conflicts before stashing changes.",
            13.0,
            400.0,
            MUTED,
        ));
    }

    let mut list = col().gap(4.0);
    if data.stashes.is_empty() {
        list = list.child(label("No stashes yet.", 14.0, 400.0, MUTED));
    }
    for stash in &data.stashes {
        let action = |pop| Action::RestoreStash {
            stash: stash.clone(),
            pop,
            restore_index: s.stash.restore_index,
        };
        list = list
            .child(
                row()
                    .h(36.0)
                    .gap(8.0)
                    .center_y()
                    .child(
                        label(&stash.reference, 12.0, 500.0, MUTED)
                            .w(86.0)
                            .truncate(),
                    )
                    .child(
                        label(&stash.description, 14.0, 500.0, INK)
                            .grow(1.0)
                            .min_w(0.0)
                            .truncate(),
                    )
                    .child(
                        command(k, "Apply", action(false), s.busy)
                            .w(64.0)
                            .label(format!("Apply {}", stash.reference))
                            .key(format!("apply-{}", stash.reference)),
                    )
                    .child(
                        command(k, "Pop", action(true), s.busy)
                            .w(64.0)
                            .label(format!("Pop {}", stash.reference))
                            .key(format!("pop-{}", stash.reference)),
                    ),
            )
            .child(divider());
    }

    col()
        .grow(1.0)
        .min_h(0.0)
        .gap(8.0)
        .child(form)
        .child(divider())
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(label("Saved stashes", 15.0, 600.0, INK).grow(1.0))
                .child(
                    Checkbox::new(s.stash.restore_index)
                        .label("Restore staged state")
                        .disabled(s.busy || data.stashes.is_empty())
                        .on_toggle(|s: &mut Desk| s.stash.restore_index = !s.stash.restore_index)
                        .build(k)
                        .key("stash-restore-index"),
                ),
        )
        .child(label(
            "Apply keeps the stash. Pop removes it after a successful restore.",
            12.0,
            400.0,
            MUTED,
        ))
        .child(scroll_list(list))
}
