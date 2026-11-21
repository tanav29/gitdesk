use super::*;

impl Desk {
    fn create_tag(&mut self) {
        let name = self.tags.name.text.trim().to_owned();
        if !name.is_empty() {
            self.act(Action::CreateTag {
                name,
                target: self.tags.target.text.trim().to_owned(),
                message: self.tags.message.text.clone(),
            });
        }
    }
}

pub(super) fn view(s: &Desk, data: &Snapshot, k: &Kit) -> El {
    let form = col()
        .gap(8.0)
        .child(heading("Tags", 20.0))
        .child(
            row()
                .gap(8.0)
                .child(
                    TextInput::new(&s.tags.name)
                        .label("Tag name")
                        .placeholder("Tag name")
                        .on_edit(|s: &mut Desk| &mut s.tags.name)
                        .disabled(s.busy)
                        .build(k)
                        .grow(1.0)
                        .key("tag-name"),
                )
                .child(
                    TextInput::new(&s.tags.target)
                        .label("Target commit")
                        .placeholder("Target (HEAD)")
                        .on_edit(|s: &mut Desk| &mut s.tags.target)
                        .disabled(s.busy)
                        .build(k)
                        .grow(1.0)
                        .key("tag-target"),
                ),
        )
        .child(
            row()
                .gap(8.0)
                .child(
                    TextInput::new(&s.tags.message)
                        .label("Annotation")
                        .placeholder("Optional annotation")
                        .on_edit(|s: &mut Desk| &mut s.tags.message)
                        .disabled(s.busy)
                        .build(k)
                        .grow(1.0)
                        .key("tag-message"),
                )
                .child(
                    button("Create tag")
                        .on_press(Desk::create_tag)
                        .disabled(s.busy || s.tags.name.text.trim().is_empty())
                        .build(k)
                        .key("create-tag"),
                ),
        )
        .child(divider());
    let mut body = form;
    if let Some(tag) = &s.tags.deleting {
        body = body
            .child(
                label(
                    format!("Delete local tag {} at {}?", tag.name, tag.short_target),
                    14.0,
                    600.0,
                    INK,
                )
                .wrap_anywhere(),
            )
            .child(
                row()
                    .gap(8.0)
                    .child(
                        command(k, "Delete tag", Action::DeleteTag(tag.clone()), s.busy)
                            .key("confirm-delete-tag"),
                    )
                    .child(
                        button("Cancel")
                            .disabled(s.busy)
                            .on_press(|s: &mut Desk| s.tags.deleting = None)
                            .build(k),
                    ),
            );
    }
    for tag in &data.tags {
        let item = tag.clone();
        let kind = if tag.annotated {
            "Annotated"
        } else {
            "Lightweight"
        };
        body = body
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(
                        col()
                            .gap(2.0)
                            .grow(1.0)
                            .min_w(0.0)
                            .child(
                                label(&tag.name, 15.0, 600.0, INK)
                                    .truncate()
                                    .help(&tag.name),
                            )
                            .child(
                                label(
                                    format!("{kind} · {}  {}", tag.short_target, tag.subject),
                                    12.0,
                                    400.0,
                                    MUTED,
                                )
                                .truncate()
                                .help(&tag.subject),
                            ),
                    )
                    .child(
                        button("Delete…")
                            .variant(ButtonVariant::Ghost)
                            .disabled(s.busy)
                            .on_press(move |s: &mut Desk| s.tags.deleting = Some(item.clone()))
                            .build(k)
                            .key(format!("delete-tag-{}", tag.name)),
                    ),
            )
            .child(divider());
    }
    if data.tags.is_empty() {
        body = body.child(label("No local tags.", 14.0, 400.0, MUTED));
    }
    scroll_list(body)
}
