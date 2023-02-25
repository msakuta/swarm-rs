use std::collections::{HashMap, HashSet};

/// A virtual filesystem, which could be in-memory, in-disk or on local storage of the browser
pub trait Vfs {
    fn list_files(&self) -> Vec<String>;
    fn get_file(&self, file: &str) -> Result<String, ()>;
    fn save_file(&mut self, file: &str, contents: &str) -> Result<(), ()>;
}

/// A reference implementation of [`Vfs`]. It serves static set of files, but won't retain changes between sessions.
pub struct StaticVfs {
    pub files: HashMap<String, String>,
}

impl StaticVfs {
    pub fn new() -> Self {
        let green_agent =
            collapse_newlines(include_str!("../behavior_tree_config/green/agent.txt"));
        let green_spawner =
            collapse_newlines(include_str!("../behavior_tree_config/green/spawner.txt"));
        let red_agent = collapse_newlines(include_str!("../behavior_tree_config/red/agent.txt"));
        let red_spawner =
            collapse_newlines(include_str!("../behavior_tree_config/red/spawner.txt"));
        let agent_early =
            collapse_newlines(include_str!("../behavior_tree_config/agent_early.txt"));
        let mut files = HashMap::new();
        files.insert("green/agent.txt".to_string(), green_agent.to_string());
        files.insert("green/spawner.txt".to_string(), green_spawner.to_string());
        files.insert("red/agent.txt".to_string(), red_agent.to_string());
        files.insert("red/spawner.txt".to_string(), red_spawner.to_string());
        files.insert("agent_early.txt".to_string(), agent_early.to_string());
        Self { files }
    }
}

impl Vfs for StaticVfs {
    fn list_files(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }

    fn get_file(&self, file: &str) -> Result<String, ()> {
        self.files.get(file).map(|rc| rc.clone()).ok_or(())
    }

    fn save_file(&mut self, file: &str, contents: &str) -> Result<(), ()> {
        self.files.insert(file.to_string(), contents.to_owned());
        Ok(())
    }
}

/// A virtual file system implemented in an actual file system.
pub struct FileVfs {
    pub files: HashSet<String>,
}

impl FileVfs {
    pub fn new() -> Self {
        let static_vfs = StaticVfs::new();
        Self {
            files: static_vfs.files.keys().cloned().collect(),
        }
    }
}

impl Vfs for FileVfs {
    fn list_files(&self) -> Vec<String> {
        self.files.iter().cloned().collect()
    }

    fn get_file(&self, file: &str) -> Result<String, ()> {
        let dir = std::path::Path::new("../behavior_tree_config");
        std::fs::read_to_string(dir.join(file)).map_err(|_| ())
    }

    fn save_file(&mut self, file: &str, contents: &str) -> Result<(), ()> {
        std::fs::write(file, contents).map_err(|_| ())
    }
}

/// Windows still uses CRLF
fn collapse_newlines(s: &str) -> String {
    // Can we skip replacing in *nix and newer Mac? Maybe, but it's such a fast operation
    // that we don't gain much by "optimizing" for the platform.
    s.replace("\r\n", "\n")
}
