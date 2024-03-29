use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

/// A virtual filesystem, which could be in-memory, in-disk or on local storage of the browser
pub trait Vfs {
    fn list_files(&self) -> Vec<String>;
    fn get_file(&self, file: &str) -> Result<String, String>;
    fn save_file(&mut self, file: &str, contents: &str) -> Result<(), String>;
    fn delete_file(&mut self, file: &str) -> Result<(), String>;
    /// Dangerous - it resets the whole filesystem!
    fn reset(&mut self) -> Result<(), String>;
}

/// A reference implementation of [`Vfs`]. It serves static set of files, but won't retain changes between sessions.
pub struct StaticVfs {
    pub files: HashMap<String, String>,
}

impl StaticVfs {
    pub fn new() -> Self {
        let green_agent =
            collapse_newlines(include_str!("../behavior_tree_config/green/agent.btc"));
        let green_spawner =
            collapse_newlines(include_str!("../behavior_tree_config/green/spawner.btc"));
        let red_agent = collapse_newlines(include_str!("../behavior_tree_config/red/agent.btc"));
        let red_spawner =
            collapse_newlines(include_str!("../behavior_tree_config/red/spawner.btc"));
        let agent_early =
            collapse_newlines(include_str!("../behavior_tree_config/agent_early.btc"));
        let mut files = HashMap::new();
        files.insert("green/agent.btc".to_string(), green_agent.to_string());
        files.insert("green/spawner.btc".to_string(), green_spawner.to_string());
        files.insert("red/agent.btc".to_string(), red_agent.to_string());
        files.insert("red/spawner.btc".to_string(), red_spawner.to_string());
        files.insert("agent_early.btc".to_string(), agent_early.to_string());
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

    fn delete_file(&mut self, file: &str) -> Result<(), String> {
        self.files
            .remove(file)
            .map(|_| ())
            .ok_or_else(|| "File not found".to_string())
    }

    fn reset(&mut self) -> Result<(), String> {
        *self = StaticVfs::new();
        Ok(())
    }
}

const BTC_DIR: &str = "./behavior_tree_config";

/// A virtual file system implemented in an actual file system.
pub struct FileVfs {
    pub files: HashSet<String>,
}

impl FileVfs {
    pub fn new() -> Self {
        let files = {
            let mut files = HashSet::new();
            match visit_dirs_root(&Path::new(BTC_DIR), &mut |file| {
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
        let dir = Path::new(BTC_DIR);
        let full_path = dir.join(file);
        std::fs::read_to_string(full_path)
            .map(|s| collapse_newlines(&s))
            .map_err(|e| e.to_string())
    }

    fn save_file(&mut self, file: &str, contents: &str) -> Result<(), String> {
        let dir = Path::new(BTC_DIR);
        let full_path = dir.join(file);
        let res =
            std::fs::write(full_path, expand_newlines(contents)).map_err(|e| e.to_string())?;
        self.files.insert(file.to_owned());
        Ok(res)
    }

    fn delete_file(&mut self, file: &str) -> Result<(), String> {
        if self.files.remove(file) {
            let full_path = Path::new(BTC_DIR).join(file);
            std::fs::remove_file(&full_path).map_err(|err| err.to_string())?;
            Ok(())
        } else {
            Err("File not found".to_string())
        }
    }

    fn reset(&mut self) -> Result<(), String> {
        for (file, contents) in StaticVfs::new().files {
            let full_path = Path::new(BTC_DIR).join(&file);
            std::fs::write(&full_path, expand_newlines(&contents))
                .map_err(|e| format!("Error on writing {file}: {e}"))?;
        }
        *self = Self::new();
        Ok(())
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

/// Oh Windows
fn expand_newlines(s: &str) -> String {
    // Is it worth replacing newlines to Windows format? Most editors support LF newlines nowadays,
    // even the infamous notepad. However, git usually configures `core.autocrlf = true` on Windows,
    // so not replacing LF with CRLF may lead to annoying diffs.
    #[cfg(target_os = "windows")]
    let s = {
        // Regex would be re.replace_all("[^\r]\n", "\r\n"), but I don't want to depend on regex library just for this!
        let mut last_char = None;
        let mut res = "".to_string();
        for char in s.chars() {
            if last_char != Some('\r') && char == '\n' {
                res.push('\r');
            }
            res.push(char);
            last_char = Some(char);
        }
        res
    };
    s.to_string()
}
