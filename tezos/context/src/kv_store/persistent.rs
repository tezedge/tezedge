use std::{
    borrow::Cow,
    cell::Cell,
    collections::{hash_map::DefaultHasher, VecDeque},
    convert::{TryFrom, TryInto},
    hash::Hasher,
    io::Write,
    sync::Arc,
};

use crypto::hash::ContextHash;
use tezos_timing::RepositoryMemoryUsage;

use crate::{
    gc::{worker::PRESERVE_CYCLE_COUNT, GarbageCollectionError, GarbageCollector},
    persistent::{
        get_persistent_base_path, DBError, File, FileOffset, FileType, Flushable,
        KeyValueStoreBackend, Persistable,
    },
    working_tree::{
        serializer::{read_object_length, ObjectHeader, ObjectLength},
        shape::{DirectoryShapeId, DirectoryShapes, ShapeStrings},
        storage::DirEntryId,
        string_interner::{StringId, StringInterner},
    },
    Map, ObjectHash,
};

use super::{HashId, VacantObjectHash};

pub struct Persistent {
    data_file: File,
    shape_file: File,
    shape_index_file: File,
    commit_index_file: File,
    strings_file: File,
    big_strings_file: File,
    big_strings_offsets_file: File,

    hashes: Hashes,
    // hashes_file: File,

    // hashes_file_index: usize,
    shapes: DirectoryShapes,
    string_interner: StringInterner,

    // hashes: Hashes,
    pub context_hashes: Map<u64, (HashId, u64)>,
    context_hashes_cycles: VecDeque<Vec<u64>>,
    // data: Vec<u8>,
}

impl GarbageCollector for Persistent {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError> {
        // self.new_cycle_started();
        Ok(())
    }

    fn block_applied(
        &mut self,
        referenced_older_objects: Vec<HashId>,
    ) -> Result<(), GarbageCollectionError> {
        // self.block_applied(referenced_older_objects);
        Ok(())
    }
}

impl Flushable for Persistent {
    fn flush(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

impl Persistable for Persistent {
    fn is_persistent(&self) -> bool {
        false
    }
}

struct Hashes {
    list: Vec<ObjectHash>,
    list_first_index: usize,
    hashes_file: File,

    bytes: Vec<u8>,
    // hashes_file_index: usize,
}

impl Hashes {
    fn try_new(base_path: &str) -> Self {
        let hashes_file = File::new(base_path, FileType::Hashes);

        Self {
            list: Vec::with_capacity(1000),
            hashes_file,
            // hashes_file_index: 0,
            list_first_index: 0,
            bytes: Vec::with_capacity(1000),
        }
    }

    fn get_hash(&self, hash_id: HashId) -> Result<Option<Cow<ObjectHash>>, DBError> {
        let hash_id_index: usize = hash_id.try_into().unwrap();

        let is_in_file = hash_id_index < self.list_first_index;

        if is_in_file {
            let offset = hash_id_index * std::mem::size_of::<ObjectHash>();

            let mut hash: ObjectHash = Default::default();

            self.hashes_file
                .read_exact_at(&mut hash, FileOffset(offset as u64));

            Ok(Some(Cow::Owned(hash)))
        } else {
            let index = hash_id_index - self.list_first_index;

            match self.list.get(index) {
                Some(hash) => Ok(Some(Cow::Borrowed(hash))),
                None => Ok(None),
            }
        }
    }

    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        let list_length = self.list.len();
        let index = self.list_first_index + list_length;
        self.list.push(Default::default());

        Ok(VacantObjectHash {
            entry: Some(&mut self.list[list_length]),
            // entry: Some(&mut self.hashes_file),
            hash_id: HashId::try_from(index).unwrap(),
            // data: Default::default(),
        })
    }

    fn contains(&self, hash_id: HashId) -> bool {
        let hash_id: usize = hash_id.try_into().unwrap();

        hash_id < self.list_first_index + self.list.len()
    }

    fn commit(&mut self) {
        if self.list.is_empty() {
            return;
        }

        self.bytes.clear();
        for h in &self.list {
            self.bytes.extend_from_slice(h);
        }
        self.list_first_index += self.list.len();

        self.hashes_file.append(&self.bytes);
        self.list.clear();
    }
}

impl Persistent {
    pub fn try_new() -> Result<Persistent, std::io::Error> {
        let base_path = get_persistent_base_path();

        let data_file = File::new(&base_path, FileType::Data);
        let shape_file = File::new(&base_path, FileType::ShapeDirectories);
        let shape_index_file = File::new(&base_path, FileType::ShapeDirectoriesIndex);
        let commit_index_file = File::new(&base_path, FileType::CommitIndex);
        let strings_file = File::new(&base_path, FileType::Strings);
        let big_strings_file = File::new(&base_path, FileType::BigStrings);
        let big_strings_offsets_file = File::new(&base_path, FileType::BigStringsOffsets);

        let hashes = Hashes::try_new(&base_path);
        // let hashes_file = File::new(&base_path, FileType::Hashes);

        let mut context_hashes_cycles = VecDeque::with_capacity(PRESERVE_CYCLE_COUNT);
        for _ in 0..PRESERVE_CYCLE_COUNT {
            context_hashes_cycles.push_back(Default::default())
        }

        Ok(Self {
            data_file,
            shape_file,
            shape_index_file,
            commit_index_file,
            strings_file,
            hashes,
            big_strings_file,
            big_strings_offsets_file,
            // hashes_file,
            // hashes_file_index: 0,
            shapes: DirectoryShapes::default(),
            string_interner: StringInterner::default(),
            // hashes: Default::default(),
            context_hashes: Default::default(),
            context_hashes_cycles,
            // data: Vec::with_capacity(100_000),
        })
    }

    #[cfg(test)]
    pub(crate) fn put_object_hash(&mut self, entry_hash: ObjectHash) -> HashId {
        let vacant = self.get_vacant_object_hash().unwrap();
        vacant.write_with(|entry| *entry = entry_hash)
    }
}

fn serialize_context_hash(hash_id: HashId, offset: u64, hash: &[u8]) -> Vec<u8> {
    let mut output = Vec::<u8>::with_capacity(100);

    let hash_id: u32 = hash_id.as_u32();

    output.write_all(&hash_id.to_ne_bytes()).unwrap();
    output.write_all(&offset.to_ne_bytes()).unwrap();
    output.write_all(hash).unwrap();

    output
}

impl KeyValueStoreBackend for Persistent {
    fn write_batch(&mut self, batch: Vec<(HashId, Arc<[u8]>)>) -> Result<(), DBError> {
        Ok(())
    }

    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        Ok(self.hashes.contains(hash_id))
    }

    fn put_context_hash(&mut self, hash_id: HashId, offset: u64) -> Result<(), DBError> {
        let commit_hash = self.get_hash(hash_id).unwrap().unwrap();
        // let commit_hash = self
        //     .hashes
        //     .get_hash(commit_hash_id)?
        //     .ok_or(DBError::MissingObject {
        //         hash_id: commit_hash_id,
        //     })?;

        let mut hasher = DefaultHasher::new();
        hasher.write(&commit_hash[..]);
        let hashed = hasher.finish();

        let output = serialize_context_hash(hash_id, offset, commit_hash.as_ref());
        self.commit_index_file.append(&output);

        self.context_hashes.insert(hashed, (hash_id, offset));
        if let Some(back) = self.context_hashes_cycles.back_mut() {
            back.push(hashed);
        };

        Ok(())
    }

    fn get_context_hash(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<(HashId, u64)>, DBError> {
        let mut hasher = DefaultHasher::new();
        hasher.write(context_hash.as_ref());
        let hashed = hasher.finish();

        Ok(self.context_hashes.get(&hashed).cloned())
    }

    fn get_hash(&self, hash_id: HashId) -> Result<Option<Cow<ObjectHash>>, DBError> {
        self.hashes.get_hash(hash_id)
    }

    fn get_value(&self, hash_id: HashId) -> Result<Option<Cow<[u8]>>, DBError> {
        todo!()
    }

    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        self.hashes.get_vacant_object_hash()
    }

    fn clear_objects(&mut self) -> Result<(), DBError> {
        Ok(())
    }

    fn memory_usage(&self) -> RepositoryMemoryUsage {
        RepositoryMemoryUsage::default()
    }

    fn get_shape(&self, shape_id: DirectoryShapeId) -> Result<ShapeStrings, DBError> {
        self.shapes
            .get_shape(shape_id)
            .map(ShapeStrings::SliceIds)
            .map_err(Into::into)
    }

    fn make_shape(
        &mut self,
        dir: &[(StringId, DirEntryId)],
    ) -> Result<Option<DirectoryShapeId>, DBError> {
        self.shapes.make_shape(dir).map_err(Into::into)
    }

    fn get_str(&self, string_id: StringId) -> Option<&str> {
        self.string_interner.get(string_id)
    }

    fn synchronize_strings(&mut self, string_interner: &StringInterner) -> Result<(), DBError> {
        self.string_interner.extend_from(string_interner);

        Ok(())
    }

    fn get_current_offset(&self) -> Result<u64, DBError> {
        Ok(self.data_file.offset())
    }

    fn append_serialized_data(&mut self, data: &[u8]) -> Result<(), DBError> {
        self.data_file.append(data);

        let strings = self.string_interner.serialize();

        self.strings_file.append(&strings.strings);
        self.big_strings_file.append(&strings.big_strings);
        self.big_strings_offsets_file
            .append(&strings.big_strings_offsets);

        let shapes = self.shapes.serialize();
        self.shape_file.append(shapes.shapes);
        self.shape_index_file.append(shapes.index);

        self.hashes.commit();

        self.data_file.sync();
        self.strings_file.sync();
        self.big_strings_file.sync();
        self.big_strings_offsets_file.sync();
        self.hashes.hashes_file.sync();
        self.commit_index_file.sync();

        Ok(())
    }

    fn synchronize_full(&mut self) -> Result<(), DBError> {
        Ok(())
    }

    fn get_value_from_offset(&self, buffer: &mut Vec<u8>, offset: u64) -> Result<(), DBError> {
        let mut header: [u8; 5] = Default::default();
        self.data_file
            .read_exact_at(&mut header[..1], FileOffset(offset));

        let object_header: ObjectHeader = ObjectHeader::from_bytes([header[0]]);

        let header = match object_header.get_length() {
            ObjectLength::OneByte => &mut header[..2],
            ObjectLength::TwoBytes => &mut header[..3],
            ObjectLength::FourBytes => &mut header[..5],
        };

        self.data_file.read_exact_at(header, FileOffset(offset));
        let (_, length) = read_object_length(header, &object_header);

        println!("LENGTH={:?}", length);

        // let length = u32::from_ne_bytes(header[1..].try_into().unwrap());
        // let total_length = length as usize;
        //let total_length = 5 + length as usize;

        buffer.resize(length, 0);
        // self.data.clear();
        // self.data.reserve(total_length);

        self.data_file.read_exact_at(buffer, FileOffset(offset));

        Ok(())
    }

    // fn get_value_from_offset(&self, buffer: &mut Vec<u8>, offset: u64) -> Result<(), DBError> {
    //     let mut header: [u8; 5] = Default::default();
    //     self.data_file.read_exact_at(&mut header, FileOffset(offset));

    //     let length = u32::from_ne_bytes(header[1..].try_into().unwrap());
    //     let total_length = length as usize;
    //     //let total_length = 5 + length as usize;

    //     buffer.resize(total_length, 0);
    //     // self.data.clear();
    //     // self.data.reserve(total_length);

    //     self.data_file.read_exact_at(buffer, FileOffset(offset));

    //     Ok(())
    // }
}
