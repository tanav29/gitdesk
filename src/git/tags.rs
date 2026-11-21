use super::*;

#[derive(Clone)]
pub struct Tag {
    pub name: String,
    pub object: String,
    pub short_target: String,
    pub subject: String,
    pub annotated: bool,
}

pub(super) fn list(repo: &Path) -> Result<Vec<Tag>> {
    let format = "--format=%(refname:lstrip=2)%00%(objectname)%00%(if)%(*objectname:short)%(then)%(*objectname:short)%(else)%(objectname:short)%(end)%00%(objecttype)%00%(contents:subject)";
    output(
        repo,
        &["for-each-ref", "--sort=-creatordate", format, "refs/tags/"],
    )?
    .lines()
    .map(|line| {
        let fields: Vec<_> = line.splitn(5, '\0').collect();
        let [name, object, target, kind, subject] = fields.as_slice() else {
            return Err("Git returned an incomplete tag record.".into());
        };
        Ok(Tag {
            name: name.to_string(),
            object: object.to_string(),
            short_target: target.to_string(),
            subject: subject.to_string(),
            annotated: *kind == "tag",
        })
    })
    .collect()
}

pub(super) fn create(repo: &Path, name: &str, target: &str, message: &str) -> Result<()> {
    let target = if target.is_empty() { "HEAD" } else { target };
    let id = output(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{target}^{{commit}}"),
        ],
    )?;
    let mut args = vec!["tag", "--no-sign"];
    if !message.trim().is_empty() {
        args.extend(["-a", "-m", message]);
    }
    args.extend(["--", name, &id]);
    output(repo, &args).map(drop)
}

pub(super) fn delete(repo: &Path, tag: &Tag) -> Result<()> {
    output(
        repo,
        &[
            "update-ref",
            "--no-deref",
            "-d",
            &format!("refs/tags/{}", tag.name),
            &tag.object,
        ],
    )
    .map(drop)
}
