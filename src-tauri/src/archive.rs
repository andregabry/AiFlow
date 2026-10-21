use crate::model::Result;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::Mutex,
};
const SHARD_SIZE: u64 = 8 * 1024 * 1024;
pub struct OutputArchive {
    path: PathBuf,
    cursors: Mutex<HashMap<String, u32>>,
}
impl OutputArchive {
    pub fn new(path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            cursors: Mutex::new(HashMap::new()),
        })
    }
    pub fn append(&self, eid: &str, payload: &Value) -> Result<Value> {
        if !eid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("Invalid archive execution ID".into());
        }
        let mut cursors = self.cursors.lock().map_err(|e| e.to_string())?;
        let shard = cursors.entry(eid.to_string()).or_insert(0);
        let (mut file, name) = loop {
            let name = format!("{eid}-{:06}.jsonl", *shard);
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .read(true)
                .open(self.path.join(&name))
                .map_err(|e| e.to_string())?;
            if file.metadata().map_err(|e| e.to_string())?.len() < SHARD_SIZE {
                break (file, name);
            }
            *shard += 1;
        };
        let offset = file.metadata().map_err(|e| e.to_string())?.len();
        let mut bytes = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_data().map_err(|e| e.to_string())?;
        Ok(json!({"archive":name,"offset":offset,"length":bytes.len()}))
    }
    pub fn hydrate(&self, payload: Value) -> Result<Value> {
        let Some(name) = payload["archive"].as_str() else {
            return Ok(payload);
        };
        if name.contains(['/', '\\']) || !name.ends_with(".jsonl") || name.contains("..") {
            return Err("Invalid output archive reference".into());
        }
        let offset = payload["offset"].as_u64().ok_or("Invalid archive offset")?;
        let length = payload["length"]
            .as_u64()
            .filter(|n| *n <= 2 * 1024 * 1024)
            .ok_or("Invalid archive record length")?;
        let mut file = File::open(self.path.join(name)).map_err(|e| {
            format!("Output archive missing: {e}. Restore database and archive together.")
        })?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![0; length as usize];
        file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }
}
