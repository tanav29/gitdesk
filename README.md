# Gitdesk

Gitdesk is a native Git manager built with [Anomaly’s Rune UI framework](https://github.com/anomalyco/rune). Open a local repository by typing its path or choosing its folder.

## Run

With Git and the stable Rust toolchain installed, run this from the Gitdesk source directory.

```powershell
cargo run --locked
```

Pass a repository path to open it at launch, for example `cargo run --locked -- "C:\Projects\my repo"`. **Open repository…** switches to another repository or linked worktree.

## Working with a repository

Use **Changes** to stage or unstage individual files. To work on multiple files, use the checkboxes beside their names or the checkbox in a section header, then choose **Stage selected** or **Unstage selected**. Enter a commit message and choose **Commit staged** to commit the index. Gitdesk keeps a separate message draft for each repository until you close the app.

The **More** menu beside the commit button lets you add a signed-off-by trailer, sign with your configured Git signing key, amend the last commit, or allow a commit with no file changes. When amending, leave the message blank to reuse the previous message. Options apply to the next successful commit.

**Diff** shows tracked changes in the working tree or index. **History** graphs up to 40 commits reachable from local branches, remote-tracking branches, and the current HEAD. Select a commit to open its patch in **Diff**. Merge commits are compared with their first parent. **Branches** lists local branches and lets you switch between them.

In **Diff → Compare branches**, choose a base and a comparison branch to view the changes from the base tip to the comparison tip. The pickers include local and remote-tracking branches. Use **Fetch** to update remote-tracking branches.

**Fetch**, **Pull**, and **Push** use the repository’s Git configuration and credentials. Pull uses fast-forward only. Git reports errors in the window. Bare repositories support history, branch inspection, fetch, and push.

**Settings** reads and edits Git configuration at local, global, worktree, and system scope, subject to Git’s permissions and repository configuration.

**GitHub** shows open pull requests, open issues, and recent Actions runs for the repository. Choose **View details** on a pull request or issue to read its description and comments or post a comment. On a pull request, **Files changed** shows the diff with line numbers. **Open on GitHub** opens the original item in your browser. Install GitHub CLI on PATH and sign in with `gh auth login` to use these views.

**Terminal** starts a shell in the repository directory for any Git or GitHub CLI command.
