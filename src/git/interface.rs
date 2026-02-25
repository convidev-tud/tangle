use crate::git::error::{GitError, GitInterfaceError};
use crate::model::*;
use crate::util::u8_to_string;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};

#[derive(Clone, Debug)]
pub enum GitPath {
    CurrentDirectory,
    CustomDirectory(PathBuf),
}

#[derive(Clone, Debug)]
pub(super) struct GitCLI {
    path: GitPath,
}
impl GitCLI {
    pub fn in_current_directory() -> Self {
        Self::new(GitPath::CurrentDirectory)
    }
    pub fn in_custom_directory(path: PathBuf) -> Self {
        Self::new(GitPath::CustomDirectory(path))
    }
    pub fn new(path: GitPath) -> Self {
        Self { path }
    }
    pub fn run(&self, args: Vec<&str>) -> io::Result<Output> {
        let mut base = Command::new("git");
        let mut arguments: Vec<String> = vec![];
        match self.path {
            GitPath::CurrentDirectory => {}
            GitPath::CustomDirectory(ref path) => {
                arguments.push(format!("--git-dir={}/.git", path.to_str().unwrap()));
                arguments.push(format!("--work-tree={}", path.to_str().unwrap()));
            }
        }
        let mut transformed: Vec<&str> = arguments.iter().map(|s| s.as_str()).collect();
        transformed.extend(args);
        base.args(transformed).output()
    }
}

#[derive(Clone, Debug)]
pub struct GitInterface {
    model: TreeDataModel,
    raw_git_interface: GitCLI,
}
impl GitInterface {
    pub fn default() -> Self {
        Self::new(GitPath::CurrentDirectory)
    }
    pub fn in_directory(path: PathBuf) -> Self {
        Self::new(GitPath::CustomDirectory(path))
    }
    pub fn new(path: GitPath) -> Self {
        let raw_interface = GitCLI::new(path);
        let mut interface = Self {
            model: TreeDataModel::new(),
            raw_git_interface: raw_interface,
        };
        match interface.update_complete_model() {
            Ok(_) => interface,
            Err(e) => panic!("{:?}", e),
        }
    }
    fn update_complete_model(&mut self) -> Result<(), GitError> {
        let branch_output = self.raw_git_interface.run(vec!["branch"])?;
        let all_branches: Vec<String> = u8_to_string(&branch_output.stdout)
            .split("\n")
            .map(|raw_string| raw_string.to_string())
            .collect();
        for branch in all_branches {
            if !branch.is_empty() {
                let mut path = QualifiedPath::from("");
                path.push(branch);
                self.model.insert_qualified_path(path, false)?;
            }
        }
        let tag_output = self.raw_git_interface.run(vec!["tag"])?;
        let all_tags: Vec<String> = u8_to_string(&tag_output.stdout)
            .split("\n")
            .map(|raw_string| raw_string.to_string())
            .collect();
        for tag in all_tags {
            if !tag.is_empty() {
                let mut path = QualifiedPath::from("");
                path.push(tag);
                self.model.insert_qualified_path(path, true)?;
            }
        }
        Ok(())
    }
    pub fn get_model(&self) -> &TreeDataModel {
        &self.model
    }
    fn get_current_branch(&self) -> Result<String, GitError> {
        Ok(u8_to_string(
            &self
                .raw_git_interface
                .run(vec!["branch", "--show-current"])?
                .stdout,
        ))
    }
    pub fn get_current_qualified_path(&self) -> Result<QualifiedPath, GitError> {
        let mut base = QualifiedPath::from("");
        base.push(self.get_current_branch()?);
        Ok(base)
    }
    pub fn get_current_node_path(&self) -> Result<NodePath<AnyNodeType>, GitError> {
        let current_qualified_path = self.get_current_qualified_path()?;
        Ok(self.model.get_node_path(&current_qualified_path).unwrap())
    }
    pub fn get_current_area(&self) -> Result<NodePath<Area>, GitError> {
        let current_qualified_path = self.get_current_qualified_path()?;
        let qualified_path = QualifiedPath::from(&current_qualified_path[1]);
        Ok(self.model.get_area(&qualified_path).unwrap())
    }

    // all git commands
    pub fn initialize_repo(&self) -> Result<Output, GitError> {
        Ok(self
            .raw_git_interface
            .run(vec!["init", "--initial-branch=main"])?)
    }
    pub fn status(&self) -> Result<Output, GitError> {
        Ok(self.raw_git_interface.run(vec!["status"])?)
    }
    pub(super) fn checkout_raw(&self, path: &QualifiedPath) -> Result<Output, GitError> {
        Ok(self
            .raw_git_interface
            .run(vec!["checkout", path.to_git_branch().as_str()])?)
    }
    pub fn checkout(&self, path: &QualifiedPath) -> Result<Output, GitError> {
        if !self.model.has_branch(&path) {
            return Err(GitError::GitInterface(GitInterfaceError::new(
                format!("Cannot checkout branch {}: does not exist", path).as_str(),
            )));
        }
        self.checkout_raw(&path)
    }
    pub(super) fn create_branch_no_mut(&self, path: &QualifiedPath) -> Result<Output, GitError> {
        let branch = path.to_git_branch();
        let commands = vec!["branch", branch.as_str()];
        Ok(self.raw_git_interface.run(commands)?)
    }
    pub fn create_branch(&mut self, path: &QualifiedPath) -> Result<Output, GitError> {
        let output = self.create_branch_no_mut(path)?;
        if output.status.success() {
            self.model.insert_qualified_path(path.clone(), false)?;
            Ok(output)
        } else {
            Err(GitError::GitInterface(GitInterfaceError::new(
                u8_to_string(&output.stderr).as_str(),
            )))
        }
    }
    pub fn delete_branch(&self, path: &QualifiedPath) -> Result<Output, GitError> {
        let branch = path.to_git_branch();
        let commands = vec!["branch", "-D", branch.as_str()];
        Ok(self.raw_git_interface.run(commands)?)
    }
    pub fn merge(&self, paths: &Vec<QualifiedPath>) -> Result<Output, GitError> {
        let mut base = vec!["merge"];
        let new_paths: Vec<String> = paths.iter().map(|s| s.to_git_branch()).collect();
        let converted_paths: Vec<&str> = new_paths.iter().map(|p| p.as_str()).collect();
        base.extend(converted_paths);
        Ok(self.raw_git_interface.run(base)?)
    }
    pub fn abort_merge(&self) -> Result<Output, GitError> {
        Ok(self.raw_git_interface.run(vec!["merge", "--abort"])?)
    }
    pub fn create_tag(&self, tag: &QualifiedPath) -> Result<Output, GitError> {
        let current_branch = self.get_current_qualified_path()?;
        let tagged = current_branch + tag.clone();
        Ok(self
            .raw_git_interface
            .run(vec!["tag", tagged.to_git_branch().as_str()])?)
    }
    pub fn delete_tag(&self, tag: &QualifiedPath) -> Result<Output, GitError> {
        let current_branch = self.get_current_qualified_path()?;
        let tagged = current_branch + tag.clone();
        Ok(self
            .raw_git_interface
            .run(vec!["tag", "-d", tagged.to_git_branch().as_str()])?)
    }
    pub fn get_commit_history(&self, branch: &QualifiedPath) -> Result<Vec<Commit>, GitError> {
        let raw_hashes = u8_to_string(
            &self
                .raw_git_interface
                .run(vec!["log", "--format=%H", branch.to_git_branch().as_str()])?
                .stdout,
        )
        .trim()
        .to_string();
        let all_hashes = raw_hashes.split("\n").collect::<Vec<&str>>();
        let commits: Vec<Commit> = all_hashes
            .into_iter()
            .map(|hash| {
                let trimmed = hash.trim();
                let commit_message = u8_to_string(
                    &self
                        .raw_git_interface
                        .run(vec!["log", "--format=%B", "-n 1", trimmed])
                        .unwrap()
                        .stdout,
                )
                .trim()
                .to_string();
                Commit::new(trimmed, commit_message)
            })
            .collect();
        Ok(commits)
    }
    pub fn get_files_managed_by_branch(
        &self,
        branch: &QualifiedPath,
    ) -> Result<Vec<String>, GitError> {
        let out = self.raw_git_interface.run(vec![
            "ls-tree",
            "-r",
            "--name-only",
            branch.to_git_branch().as_str(),
        ])?;
        Ok(u8_to_string(&out.stdout)
            .split("\n")
            .map(|e| e.to_string())
            .collect())
    }
    pub fn get_files_changed_by_commit(&self, commit: &str) -> Result<Vec<String>, GitError> {
        let out = self.raw_git_interface.run(vec![
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            commit,
            "-r",
        ])?;
        Ok(u8_to_string(&out.stdout)
            .split("\n")
            .map(|e| e.to_string())
            .collect())
    }
    pub fn commit(&self, message: &str) -> Result<Output, GitError> {
        Ok(self.raw_git_interface.run(vec!["commit", "-m", message])?)
    }
    pub fn empty_commit(&self, message: &str) -> Result<Output, GitError> {
        Ok(self
            .raw_git_interface
            .run(vec!["commit", "--allow-empty", "-m", message])?)
    }
    pub fn cherry_pick(&self, commit: &str) -> Result<Output, GitError> {
        Ok(self.raw_git_interface.run(vec!["cherry-pick", commit])?)
    }
    pub fn reset_hard(&self, commit: &str) -> Result<Output, GitError> {
        Ok(self
            .raw_git_interface
            .run(vec!["reset", "--hard", commit])?)
    }
}

#[cfg(test)]
pub mod test_utils {
    use crate::git::error::GitError;
    use crate::git::interface::GitCLI;
    use std::fs;
    use std::path::PathBuf;

    pub fn prepare_empty_git_repo(path: PathBuf) -> Result<(), GitError> {
        let git = GitCLI::in_custom_directory(path.clone());
        git.run(vec!["init", "--initial-branch=main"])?;
        let mut file = path.clone();
        file.push("file1");
        fs::write(file.clone(), "")?;
        let out = git.run(vec!["add", file.to_str().unwrap()])?;
        let out = git.run(vec!["commit", "-m", "initial commit"])?;
        Ok(())
    }

    pub fn populate_with_features(path: PathBuf) -> Result<(), GitError> {
        let git = GitCLI::in_custom_directory(PathBuf::from(path));
        let branches = vec![
            "_main/_feature/root",
            "_main/_feature/_root/foo",
            "_main/_feature/_root/bar",
            "_main/_feature/_root/baz",
        ];
        for branch in branches {
            git.run(vec!["branch", branch])?;
        }
        Ok(())
    }

    pub fn populate_with_products(path: PathBuf) -> Result<(), GitError> {
        let git = GitCLI::in_custom_directory(PathBuf::from(path));
        let branches = vec!["_main/_product/myprod"];
        for branch in branches {
            git.run(vec!["branch", branch])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::interface::test_utils::{populate_with_features, prepare_empty_git_repo};
    use tempfile::TempDir;

    #[test]
    fn interface_populate_model() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        let interface = GitInterface::new(GitPath::CustomDirectory(path_buf));
        let paths = interface.get_model().get_qualified_paths_with_branches();
        assert_eq!(
            paths,
            &vec![
                "/main/feature/root/bar",
                "/main/feature/root/baz",
                "/main/feature/root/foo",
                "/main/feature/root",
                "/main",
            ]
        );
    }

    #[test]
    fn interface_get_current_branch_absolute() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        let interface = GitInterface::new(GitPath::CustomDirectory(path_buf));
        let current = interface.get_current_qualified_path().unwrap();
        assert_eq!(current, "/main")
    }
}
