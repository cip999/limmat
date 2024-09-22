use std::{
    fs::{self, create_dir_all, File},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{git::Hash, test::TestResult};

// Result database similar to the design described in
// https://github.com/bjackman/git-brisect?tab=readme-ov-file#the-result-directory
// TODO: Actually we should probably separate it by the repo lol. But how?
pub struct Database {
    base_dir: PathBuf,
}

impl Database {
    pub fn create_or_open(base_dir: &Path) -> anyhow::Result<Self> {
        create_dir_all(base_dir).context(format!(
            "creating result database dir at {}",
            base_dir.display()
        ))?;
        Ok(Self {
            base_dir: base_dir.to_owned(),
        })
    }

    fn result_path(&self, hash: &Hash, test_name: impl Into<String>) -> PathBuf {
        self.base_dir.join(hash.as_ref()).join(test_name.into())
    }

    pub fn cached_result(
        &self,
        hash: &Hash,
        test_name: impl Into<String>,
    ) -> Result<Option<TestResult>> {
        let result_path = self.result_path(hash, test_name).join("result.json");
        if result_path.exists() {
            Ok(Some(
                serde_json::from_str(
                    &fs::read_to_string(result_path).context("reading result JSON")?,
                )
                .context("parsing result JSON")?,
            ))
        } else {
            Ok(None)
        }
    }

    // Prepare to create the output directory for a job output, but don't actually create it yet.
    // It's created once you use one of the methods of CommitOutput for writing data.
    pub fn create_output(
        &self,
        hash: &Hash,
        test_name: impl Into<String>,
    ) -> anyhow::Result<TestCaseOutput> {
        TestCaseOutput::new(self.result_path(hash, test_name))
    }
}

// Output for an individual commit.
pub struct TestCaseOutput {
    base_dir: PathBuf,
    stdout_opened: bool,
    stderr_opened: bool,
    status_written: bool,
}

impl TestCaseOutput {
    pub fn new(base_dir: PathBuf) -> anyhow::Result<Self> {
        Ok(Self {
            base_dir,
            stdout_opened: false,
            stderr_opened: false,
            status_written: false,
        })
    }

    // Create and return base directory
    fn get_base_dir(&self) -> Result<&Path> {
        create_dir_all(&self.base_dir).context(format!(
            "creating commit result dir at {}",
            self.base_dir.display()
        ))?;
        Ok(&self.base_dir)
    }

    // Panics if called more than once.
    pub fn stdout(&mut self) -> Result<File> {
        assert!(!self.stdout_opened);
        self.stdout_opened = true;
        Ok(File::create(self.get_base_dir()?.join("stdout.txt"))?)
    }

    // Panics if called more than once.
    pub fn stderr(&mut self) -> Result<File> {
        assert!(!self.stderr_opened);
        self.stderr_opened = true;
        Ok(File::create(self.get_base_dir()?.join("stderr.txt"))?)
    }

    // TODO: Figure out how to record errors in the more general case, probably with a JSON object.
    // Panics if called more than once.
    pub fn set_result(&mut self, result: &TestResult) -> anyhow::Result<()> {
        assert!(!self.status_written);
        self.status_written = true;
        Ok(fs::write(
            self.get_base_dir()?.join("result.json"),
            serde_json::to_vec(result).expect("failed to serialize TestStatus"),
        )?)
    }
}

// TODO:
// - Test behaviour on already-existing directories
