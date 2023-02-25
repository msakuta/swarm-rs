use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

/// A virtual filesystem, which could be in-memory, in-disk or on local storage of the browser
pub trait Vfs {
    fn list_files(&self) -> Vec<String>;
    fn get_file(&self, file: &str) -> Result<String, String>;
    fn save_file(&mut self, file: &str, contents: &str) -> Result<(), String>;
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
        let mut res: Vec<String> = self.files.keys().cloned().collect();
        res.sort();
        res
    }

    fn get_file(&self, file: &str) -> Result<String, String> {
        self.files
            .get(file)
            .map(|rc| rc.clone())
            .ok_or("File not found".to_string())
    }

    fn save_file(&mut self, file: &str, contents: &str) -> Result<(), String> {
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
        let files = {
            let mut files = HashSet::new();
            match visit_dirs_root(&Path::new("./behavior_tree_config"), &mut |file| {
                files.insert(file.to_string_lossy().to_string());
            }) {
                Ok(()) => files,
                Err(_) => {
                    eprintln!("Failed to open directory \"../behavior_tree_config\", falling back to static default...");
                    let static_vfs = StaticVfs::new();
                    static_vfs.files.keys().cloned().collect()
                }
            }
        };
        Self { files }
    }
}

impl Vfs for FileVfs {
    fn list_files(&self) -> Vec<String> {
        let mut res: Vec<String> = self.files.iter().cloned().collect();
        res.sort();
        res
    }

    fn get_file(&self, file: &str) -> Result<String, String> {
        let dir = std::path::Path::new("behavior_tree_config");
        let full_path = dir.join(file);
        std::fs::read_to_string(full_path).map_err(|e| e.to_string())
    }

    fn save_file(&mut self, file: &str, contents: &str) -> Result<(), String> {
        let dir = std::path::Path::new("behavior_tree_config");
        let full_path = dir.join(file);
        let res = std::fs::write(full_path, contents).map_err(|e| e.to_string())?;
        self.files.insert(file.to_owned());
        Ok(res)
    }
}

fn visit_dirs_root(dir: &Path, cb: &mut impl FnMut(&Path)) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_dirs(&path, Path::new(path.file_name().unwrap()), cb)?;
        } else if let Some(filename) = path.file_name() {
            let relpath = Path::new(filename);
            cb(&relpath);
        }
    }
    Ok(())
}

fn visit_dirs(dir: &Path, parent: &Path, cb: &mut impl FnMut(&Path)) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(filename) = path.file_name() {
                    let parent = parent.join(filename);
                    visit_dirs(&path, &parent, cb)?;
                }
            } else if let Some(filename) = path.file_name() {
                let relpath = parent.join(filename);
                cb(&relpath);
            }
        }
    }
    Ok(())
}

/// Windows still uses CRLF
fn collapse_newlines(s: &str) -> String {
    // Can we skip replacing in *nix and newer Mac? Maybe, but it's such a fast operation
    // that we don't gain much by "optimizing" for the platform.
    s.replace("\r\n", "\n")
}
