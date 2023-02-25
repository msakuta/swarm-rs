use ::swarm_rs::vfs::{StaticVfs, Vfs};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub(crate) fn log(s: &str);
}

pub(crate) struct LocalStorageVfs {
    files: HashMap<String, String>,
}

impl LocalStorageVfs {
    pub fn new() -> Self {
        let local_storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
        let files_encoded = local_storage.get("swarm-rs-btc").ok();

        if let Some(files_encoded) = files_encoded.flatten() {
            if let Some(files) = ron::from_str(&files_encoded).ok() {
                log(&format!("Loaded {} bytes of VFS", files_encoded.len()));
                return Self { files };
            }
        }

        let static_vfs = StaticVfs::new();
        Self {
            files: static_vfs.files,
        }
    }
}

impl Vfs for LocalStorageVfs {
    fn list_files(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }

    fn get_file(&self, path: &str) -> Result<String, String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| "File not found!".to_owned())
    }

    fn save_file(&mut self, path: &str, content: &str) -> Result<(), String> {
        let entry = self
            .files
            .entry(path.to_owned())
            .or_insert_with(|| "".to_owned());
        *entry = content.to_owned();
        let local_storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
        match ron::to_string(&self.files) {
            Ok(files) => {
                local_storage
                    .set("swarm-rs-btc", &files)
                    .map_err(|e| format!("localStorage: {e:?}"))?;
                log(&format!("Saved {} bytes of VFS", files.len()));
                Ok(())
            }
            Err(e) => Err(format!("Ron format error {e}")),
        }
    }
}
