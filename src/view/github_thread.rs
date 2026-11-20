use super::{ACCENT, Desk, INK, MUTED, button, diff, divider, label, open_on_github, scroll_list};
use crate::{GitHubThread, github::Section};
use rune::{El, col, row};
use rune_kit::{ButtonVariant, Kit, TextArea};

pub(super) fn view(s: &Desk, thread: &GitHubThread, k: &Kit) -> El {
    let mut panel = col()
        .grow(1.0)
        .min_h(0.0)
        .gap(8.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    button(format!(
                        "Back to {}",
                        s.github_section.label().to_lowercase()
                    ))
                    .disabled(s.busy)
                    .on_press(|s: &mut Desk| {
                        s.github_thread = None;
                        s.workspace.notice.clear();
                    })
                    .build(k)
                    .key("github-back"),
                )
                .child(open_on_github(k, thread.url.clone())),
        )
        .child(
            label(
                format!("#{}  {}", thread.number, thread.title),
                17.0,
                600.0,
                INK,
            )
            .max_lines(2)
            .wrap_anywhere(),
        );
    if s.github_section == Section::PullRequests {
        let mut tabs = row().gap(8.0);
        for (show_diff, title) in [(false, "Conversation"), (true, "Files changed")] {
            tabs = tabs.child(
                button(title)
                    .variant(if thread.show_diff == show_diff {
                        ButtonVariant::Contrast
                    } else {
                        ButtonVariant::Ghost
                    })
                    .disabled(s.busy)
                    .on_press(move |s: &mut Desk| s.show_github_diff(show_diff))
                    .build(k)
                    .key(title),
            );
        }
        panel = panel.child(tabs);
    }
    panel.child(divider()).child(if thread.show_diff {
        let changed_files = if let Some(patch) = &thread.patch {
            diff::contents(patch, thread.diff_pages, k, "No changed files.", |s| {
                if let Some(thread) = &mut s.github_thread {
                    thread.diff_pages += 1;
                }
            })
        } else {
            scroll_list(col())
        };
        changed_files.key("github-changed-files")
    } else {
        conversation(s, thread, k)
    })
}

fn conversation(s: &Desk, thread: &GitHubThread, k: &Kit) -> El {
    let mut messages = col().gap(8.0);
    if let Some(conversation) = &thread.conversation {
        messages = messages.child(message(
            conversation.author.as_ref(),
            if conversation.body.trim().is_empty() {
                "No description provided."
            } else {
                &conversation.body
            },
        ));
        if conversation.comments.is_empty() {
            messages = messages.child(label("No comments yet.", 13.0, 400.0, MUTED));
        }
        for comment in &conversation.comments {
            messages = messages
                .child(divider())
                .child(message(comment.author.as_ref(), &comment.body));
        }
    }
    col()
        .grow(1.0)
        .min_h(0.0)
        .gap(8.0)
        .child(scroll_list(messages).key("github-messages"))
        .child(divider())
        .child(label("Add a comment", 13.0, 500.0, INK))
        .child(
            TextArea::new(&thread.draft)
                .label("Add a comment")
                .width(None)
                .rows(3)
                .wrap_anywhere()
                .disabled(s.busy)
                .on_edit(|s: &mut Desk| &mut s.github_thread.as_mut().unwrap().draft)
                .build(k)
                .key("github-comment-body"),
        )
        .child(
            row().justify_end().child(
                button("Post comment")
                    .variant(ButtonVariant::Contrast)
                    .disabled(s.busy || thread.draft.text.trim().is_empty())
                    .on_press(Desk::post_github_comment)
                    .build(k)
                    .key("github-post-comment"),
            ),
        )
}

fn message(author: Option<&crate::github::Author>, body: &str) -> El {
    let author = author.map_or("Unknown author", |author| author.login.as_str());
    col()
        .gap(4.0)
        .child(label(author, 13.0, 600.0, ACCENT).wrap_anywhere())
        .child(
            label(body, 14.0, 400.0, INK)
                .wrap_anywhere()
                .selectable(true),
        )
}
