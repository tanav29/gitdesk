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

pub(super) fn view(s: &Desk, data: &Snapshot, k: &Kit, layers: &mut Vec<El>) -> El {
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
        return col()
            .gap(8.0)
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
    if let Some(tag) = &s.tags.publishing {
        let remote = s
            .tags
            .remote
            .as_deref()
            .filter(|name| data.remotes.iter().any(|r| r == name));
        let picker = Select::new("tag-push-remote", &data.remotes)
            .current(remote)
            .label("Remote")
            .placeholder("Choose a remote")
            .appearance(SelectAppearance::Field)
            .width(200.0)
            .open(s.tags.remote_picker)
            .disabled(s.busy)
            .on_toggle(|s: &mut Desk| s.tags.remote_picker = true)
            .on_close(|s: &mut Desk| s.tags.remote_picker = false)
            .on_choose(|s: &mut Desk, name, _| {
                s.tags.remote = Some(name.into());
                s.tags.remote_picker = false;
            })
            .build(k, layers);
        return col()
            .gap(8.0)
            .child(heading("Push tag", 20.0))
            .child(
                label(
                    format!(
                        "Publish {} at {} to the selected remote.",
                        tag.name, tag.short_target
                    ),
                    14.0,
                    500.0,
                    INK,
                )
                .wrap_anywhere(),
            )
            .child(picker)
            .child(
                row()
                    .gap(8.0)
                    .child(
                        command(
                            k,
                            "Push tag",
                            Action::PushTag(tag.clone(), remote.unwrap_or_default().into()),
                            s.busy || remote.is_none(),
                        )
                        .key("confirm-push-tag"),
                    )
                    .child(
                        button("Cancel")
                            .disabled(s.busy)
                            .on_press(|s: &mut Desk| s.tags.publishing = None)
                            .build(k),
                    ),
            );
    }
    for tag in &data.tags {
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
                    .child(tag_menu(s, tag, data, k, layers)),
            )
            .child(divider());
    }
    if data.tags.is_empty() {
        body = body.child(label("No local tags.", 14.0, 400.0, MUTED));
    }
    scroll_list(body)
}

fn tag_menu(s: &Desk, tag: &crate::git::Tag, data: &Snapshot, k: &Kit, layers: &mut Vec<El>) -> El {
    let name = tag.name.clone();
    let object = tag.object.clone();
    let mut menu = Menu::new(format!("tag-menu-{name}"))
        .trigger(
            button("Actions")
                .icon(k.glyph.chevron_down)
                .disabled(s.busy)
                .on_press(move |s: &mut Desk| {
                    s.tags.menu = (s.tags.menu.as_ref() != Some(&name)).then(|| name.clone())
                })
                .build(k)
                .key(format!("tag-actions-{}", tag.name)),
        )
        .open(s.tags.menu.as_ref() == Some(&tag.name))
        .on_close(|s: &mut Desk| s.tags.menu = None)
        .placement(Placement::BottomEnd)
        .item(
            MenuItem::new("View target commit").on_select(move |s: &mut Desk| {
                if let Some(repo) = s.workspace.selected.clone() {
                    let object = object.clone();
                    s.tags.menu = None;
                    s.start("Reading tagged commit…", move || {
                        crate::Update::TaggedCommit(crate::git::tagged_commit(&repo, &object))
                    });
                }
            }),
        );
    if !data.remotes.is_empty() {
        let item = tag.clone();
        menu = menu.item(MenuItem::new("Push tag…").on_select(move |s: &mut Desk| {
            s.tags.menu = None;
            s.tags.remote_picker = false;
            if let Some(data) = &s.workspace.snapshot
                && s.tags
                    .remote
                    .as_ref()
                    .is_none_or(|r| !data.remotes.contains(r))
            {
                s.tags.remote = data
                    .remotes
                    .iter()
                    .find(|r| r.as_str() == "origin")
                    .or(data.remotes.first())
                    .cloned();
            }
            s.tags.publishing = Some(item.clone());
        }));
    }
    let item = tag.clone();
    menu.item(
        MenuItem::new("Delete local tag…").on_select(move |s: &mut Desk| {
            s.tags.menu = None;
            s.tags.deleting = Some(item.clone());
        }),
    )
    .build(k, layers)
}
