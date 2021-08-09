// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::datastore::{KeyDirEntry, KeysDir};
use crate::errors::EdgeKVError;
use crate::schema::{DataEntry, Decoder, Encoder, HintEntry};
use crate::Result;
use chrono::Utc;
use fs2::FileExt;
use fs_extra::dir::DirOptions;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use bloomfilter::Bloom;

const DATA_FILE_EXTENSION: &str = "data";
const HINT_FILE_EXTENSION: &str = "hint";
const BUFFER_FILE_EXTENSION: &str = "buff";
const BLOOM_FILTER_FILE_EXTENSION: &str = "blmf";

#[derive(Debug, Clone)]
pub struct FilePair {
    file_id: String,
    data_file_path: PathBuf,
    hint_file_path: PathBuf,
    bloom_filter_file_path: PathBuf,
}

impl FilePair {
    fn new(file_id: &str) -> Self {
        Self {
            file_id: file_id.to_string(),
            data_file_path: Default::default(),
            hint_file_path: Default::default(),
            bloom_filter_file_path: Default::default(),
        }
    }

    pub fn data_file_path(&self) -> String {
        String::from(self.data_file_path.to_string_lossy())
    }

    pub fn hint_file_path(&self) -> String {
        String::from(self.hint_file_path.to_string_lossy())
    }

    pub fn bloom_filter_file_path(&self) -> String {
        String::from(self.bloom_filter_file_path.to_string_lossy())
    }
}
#[derive(Debug)]
pub struct Index {
    file_id: String,
    data_file_path: PathBuf,
    hint_file_path: PathBuf,
}

impl Index {
    pub fn read(&self, entry_position: u64, _size: usize) -> Result<DataEntry> {
        let data = File::open(self.data_file_path.as_path())?;
        let mut reader = BufReader::new(data);

        // TODO - TE-721: handle this error
        reader.seek(SeekFrom::Start(entry_position));

        let data_entry = DataEntry::decode(&mut reader)?;

        if !data_entry.check_crc() {
            return Err(EdgeKVError::CorruptData);
        }

        Ok(data_entry)
    }

    pub fn get_hints(&self) -> Result<Vec<HintEntry>> {
        let hint = File::open(self.hint_file_path.as_path())?;
        let mut rdr = BufReader::new(hint);
        let mut hints = Vec::new();
        while let Ok(hint_entry) = HintEntry::decode(&mut rdr) {
            hints.push(hint_entry)
        }
        Ok(hints)
    }

    pub fn get_hint_entries(&self) -> Result<BTreeMap<Vec<u8>, KeyDirEntry>> {
        let mut entries = BTreeMap::new();
        let hint = File::open(self.hint_file_path.as_path())?;
        let mut rdr = BufReader::new(hint);
        while let Ok(hint_entry) = HintEntry::decode(&mut rdr) {
            if hint_entry.is_deleted() {
                entries.remove(&hint_entry.key());
            } else {
                let key_dir_entry = KeyDirEntry::new(
                    self.file_id.to_string(),
                    hint_entry.key_size(),
                    hint_entry.value_size(),
                    hint_entry.data_entry_position(),
                );
                entries.insert(hint_entry.key(), key_dir_entry);
            }
        }
        Ok(entries)
    }

    pub fn data_file_path(&self) -> PathBuf {
        self.data_file_path.clone()
    }

    pub fn hint_file_path(&self) -> PathBuf {
        self.hint_file_path.to_owned()
    }

    pub fn file_id(&self) -> String {
        self.file_id.to_owned()
    }
}

impl FilePair {
    pub fn fetch_bloom_filters(&self, keys_dir: &KeysDir) -> Result<()> {
        let mut raw_bloom_filter = Vec::new();
        let bloomfilter_file = File::open(&self.bloom_filter_file_path.as_path())?;
        let mut rdr = BufReader::new(bloomfilter_file);

        // TODO - TE-721: handle this error
        rdr.read_to_end(&mut raw_bloom_filter);

        if raw_bloom_filter.is_empty() {
            return Ok(());
        }

        //println!("[fetch_bloom_filters] Raw Bloom: {:?}", raw_bloom_filter);
        let bloomfilter: Bloom<Vec<u8>> = match bincode::deserialize(&raw_bloom_filter) {
            Ok(bf) => bf,
            Err(_) => return Ok(()),
        };

        // TODO - TE-721: handle this error
        keys_dir.insert_bloom(self.file_id.to_string(), bloomfilter);

        Ok(())
    }
    pub fn file_id(&self) -> String {
        self.file_id.to_owned()
    }

    pub fn fetch_hint_entries(&self, keys_dir: &KeysDir) -> Result<()> {
        let hint_file = File::open(&self.hint_file_path.as_path())?;
        let mut rdr = BufReader::new(hint_file);
        while let Ok(hint_entry) = HintEntry::decode(&mut rdr) {
            if !hint_entry.check_crc() {
                return Err(EdgeKVError::CorruptData);
            }
            if hint_entry.is_deleted() {
                // TODO - TE-721: handle this error
                keys_dir.remove(&hint_entry.key());
            } else {
                let key_dir_entry = KeyDirEntry::new(
                    self.file_id.to_string(),
                    hint_entry.key_size(),
                    hint_entry.value_size(),
                    hint_entry.data_entry_position(),
                );
                // TODO - TE-721: handle this error
                keys_dir.insert(hint_entry.key(), key_dir_entry);
            }
        }
        Ok(())
    }

    pub fn to_index(&self) -> Result<Index> {
        let data_file_path = self.data_file_path.clone();
        let hint_file_path = self.hint_file_path.clone();

        Ok(Index {
            file_id: self.file_id.clone(),
            data_file_path,
            hint_file_path,
        })
    }
}

pub struct ActiveFilePair {
    hint_file: File,
    data_file: File,
    file_pair: FilePair,
}

impl ActiveFilePair {
    pub fn from(file_pair: FilePair) -> Result<Self> {
        let data_file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&file_pair.data_file_path.as_path())?;
        let hint_file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&file_pair.hint_file_path.as_path())?;
        Ok(Self {
            hint_file,
            data_file,
            file_pair,
        })
    }

    pub fn get_file_pair(&self) -> FilePair {
        self.file_pair.clone()
    }

    pub fn sync(&self) -> Result<()> {
        self.hint_file.sync_all()?;
        self.data_file.sync_all()?;
        Ok(())
    }

    pub fn file_id(&self) -> String {
        self.file_pair.file_id.to_owned()
    }

    pub fn hint_file_size(&self) -> Result<u64> {
        let metadata = self.hint_file.metadata()?;
        Ok(metadata.len())
    }

    pub fn data_file_size(&self) -> Result<u64> {
        let metadata = self.data_file.metadata()?;
        Ok(metadata.len())
    }

    pub fn as_file_pair(&self) -> &FilePair {
        &self.file_pair
    }
}

impl Drop for ActiveFilePair {
    fn drop(&mut self) {
        match self.sync() {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Sync Error: {:#?}", e)
            }
        }
    }
}

impl ActiveFilePair {
    pub fn write(&self, entry: &DataEntry, _keys_dir: &KeysDir) -> Result<KeyDirEntry> {
        self.data_file.try_lock_exclusive()?;
        self.hint_file.try_lock_exclusive()?;

        //Appends entry to data file
        let mut dfw = BufWriter::new(&self.data_file);
        let data_entry_position = dfw.seek(SeekFrom::End(0))?;
        dfw.write_all(&entry.encode())?;
        // TODO - TE-721: handle this error
        dfw.flush();

        //Append hint to hint file
        let hint_entry = HintEntry::from(entry, data_entry_position);
        let mut hfw = BufWriter::new(&self.hint_file);
        hfw.seek(SeekFrom::End(0))?;
        hfw.write_all(&hint_entry.encode())?;
        // TODO - TE-721: handle htis error
        hfw.flush();

        self.data_file.unlock()?;
        self.hint_file.unlock()?;
        Ok(KeyDirEntry::new(
            self.file_pair.file_id.to_string(),
            hint_entry.key_size(),
            hint_entry.value_size(),
            data_entry_position,
        ))
    }

    pub fn remove(&self, key: Vec<u8>) -> Result<()> {
        self.hint_file.try_lock_exclusive()?;
        //Append hint to hint file
        let hint_entry = HintEntry::tombstone(key);
        let mut hfw = BufWriter::new(&self.hint_file);
        hfw.seek(SeekFrom::End(0))?;
        hfw.write_all(&hint_entry.encode())?;
        // TODO - TE-721: handle this error
        hfw.flush();
        self.hint_file.unlock()?;
        Ok(())
    }
}

pub fn create_new_file_pair<P: AsRef<Path>>(dir: P) -> Result<FilePair> {
    fs_extra::dir::create_all(dir.as_ref(), false)?;

    let file_name = Utc::now().timestamp_nanos().to_string();
    let mut data_file_path = PathBuf::new();
    data_file_path.push(dir.as_ref());
    data_file_path.push(format!("{}.{}", file_name, DATA_FILE_EXTENSION));
    data_file_path.set_extension(DATA_FILE_EXTENSION);

    let mut hint_file_path = PathBuf::new();
    hint_file_path.push(dir.as_ref());
    hint_file_path.push(format!("{}.{}", file_name, HINT_FILE_EXTENSION));
    hint_file_path.set_extension(HINT_FILE_EXTENSION);

    let mut bloom_filter_file_path = PathBuf::new();
    bloom_filter_file_path.push(dir.as_ref());
    bloom_filter_file_path.push(format!("{}.{}", file_name, BLOOM_FILTER_FILE_EXTENSION));
    bloom_filter_file_path.set_extension(BLOOM_FILTER_FILE_EXTENSION);

    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(data_file_path.as_path())?;
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(hint_file_path.as_path())?;

    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(bloom_filter_file_path.as_path())?;

    Ok(FilePair {
        data_file_path,
        hint_file_path,
        file_id: file_name,
        bloom_filter_file_path,
    })
}

pub fn get_lock_file<P: AsRef<Path>>(dir: P) -> Result<File> {
    let mut lock_file_path = PathBuf::new();
    lock_file_path.push(dir.as_ref());
    lock_file_path.push("edgekv.lock");
    fs_extra::dir::create_all(dir.as_ref(), false)?;
    let file = OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .open(lock_file_path.as_path())?;
    Ok(file)
}

pub fn fetch_file_pairs<P: AsRef<Path>>(
    dir: P,
) -> Result<(BTreeMap<String, FilePair>, BTreeMap<i64, PathBuf>)> {
    let mut file_pairs = BTreeMap::new();
    let mut buffer_files: BTreeMap<i64, PathBuf> = BTreeMap::new();
    let mut option = DirOptions::new();
    option.depth = 1;

    let dir_content = fs_extra::dir::get_dir_content2(dir, &option)?;
    for file in dir_content.files.iter() {
        let file_path = Path::new(file);
        let file_extension =
            String::from(file_path.extension().unwrap_or_default().to_string_lossy());
        match file_extension.as_str() {
            DATA_FILE_EXTENSION => {}
            HINT_FILE_EXTENSION => {}
            BUFFER_FILE_EXTENSION => {}
            BLOOM_FILTER_FILE_EXTENSION => {}
            _ => {
                continue;
            }
        };

        let file_name = String::from(file_path.file_name().unwrap().to_string_lossy());
        let file_name = &file_name[..file_name.len() - 5];

        match file_extension.as_str() {
            DATA_FILE_EXTENSION => {
                let file_pair = file_pairs
                    .entry(file_name.to_owned())
                    .or_insert(FilePair::new(file_name));
                file_pair.data_file_path = file_path.to_path_buf();
            }
            HINT_FILE_EXTENSION => {
                let file_pair = file_pairs
                    .entry(file_name.to_owned())
                    .or_insert(FilePair::new(file_name));
                file_pair.hint_file_path = file_path.to_path_buf();
            }
            BLOOM_FILTER_FILE_EXTENSION => {
                let file_pair = file_pairs
                    .entry(file_name.to_owned())
                    .or_insert(FilePair::new(file_name));
                file_pair.bloom_filter_file_path = file_path.to_path_buf();
            }
            BUFFER_FILE_EXTENSION => {
                buffer_files.insert(
                    file_name
                        .parse()
                        .map_err(|_e| EdgeKVError::StringToIntegerParseError)?,
                    file_path.to_path_buf(),
                );
            }
            _ => {}
        };
    }
    Ok((file_pairs, buffer_files))
}

#[cfg(test)]
mod tests {
    use crate::file_ops::{create_new_file_pair, fetch_file_pairs};

    #[test]
    fn test_create_file_pairs() {
        create_new_file_pair("./testdir").unwrap();
        create_new_file_pair("./testdir").unwrap();
        create_new_file_pair("./testdir").unwrap();

        let (b, _) = fetch_file_pairs("./testdir").unwrap();
        println!("{:#?}", b);
        clean_up()
    }

    fn clean_up() {
        fs_extra::dir::remove("./testdir").ok();
    }
}
