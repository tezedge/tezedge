use std::{fmt::format, fs::File, mem::ManuallyDrop, path::PathBuf, sync::atomic::AtomicUsize};

use memmap2::MmapRaw;

#[derive(Debug)]
pub enum MmappedError {
    Full,
}

struct MmapInner<T> {
    ptr: ManuallyDrop<Box<[T]>>,
    mmap: Option<MmapRaw>,
    file_path: PathBuf,
}

impl<T> std::ops::Deref for MmapInner<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.ptr
    }
}

impl<T> std::ops::DerefMut for MmapInner<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ptr
    }
}

impl<T> Drop for MmapInner<T> {
    fn drop(&mut self) {
        if let Some(mmap) = self.mmap.take() {
            // Call munmap before removing the file
            std::mem::drop(mmap);
        }
        if let Err(e) = std::fs::remove_file(&self.file_path) {
            eprintln!(
                "Failed to remove the Mmapped file {:?}: {:?}",
                self.file_path, e
            );
        }
    }
}

static MMAP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn create_file() -> std::io::Result<(PathBuf, File)> {
    loop {
        let file_path = format!(
            "working_tree_{}",
            MMAP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );

        match std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true) // Error if the file already exist
            .open(&file_path)
        {
            Ok(file) => return Ok((file_path.into(), file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Retry with a new name
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

impl<T> MmapInner<T> {
    fn try_new(capacity: usize) -> Result<Self, std::io::Error> {
        let nbytes = capacity * std::mem::size_of::<T>();

        let (file_path, file) = create_file()?;

        file.set_len(nbytes as u64)?;

        let mmap = memmap2::MmapOptions::new()
            .len(nbytes)
            .offset(0)
            .map_raw(&file)
            .unwrap();

        assert_eq!(mmap.len(), nbytes);

        let ptr: *mut u8 = mmap.as_mut_ptr();
        Self::check_alignment(ptr);

        let ptr: *mut T = ptr as *mut T;
        let ptr_slice: *mut [T] = std::ptr::slice_from_raw_parts_mut(ptr, capacity);
        let ptr: Box<[T]> = unsafe { Box::from_raw(ptr_slice) };

        Ok(Self {
            ptr: ManuallyDrop::new(ptr),
            mmap: Some(mmap),
            file_path: file_path.into(),
        })
    }

    fn check_alignment(ptr: *mut u8) {
        let ptr_u64 = ptr as u64;
        let align_of_t = std::mem::align_of::<T>() as u64;

        assert_eq!(ptr_u64 % align_of_t, 0);
    }
}

pub struct MmappedVec<T> {
    inner: MmapInner<T>,
    length: usize,
    capacity: usize,
}

impl<T: std::fmt::Debug> std::fmt::Debug for MmappedVec<T>
where
    T: Sized,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let slice: &[T] = &self;
        f.debug_struct("MmappedVec")
            .field("inner", &slice)
            .field("length", &self.length)
            .field("capacity", &self.capacity)
            .finish()
    }
}

impl<T> std::ops::Deref for MmappedVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.inner[..self.length]
    }
}

impl<T> std::ops::DerefMut for MmappedVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner[..self.length]
    }
}

impl<T> MmappedVec<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: MmapInner::try_new(capacity).unwrap(),
            length: 0,
            capacity,
        }
    }

    pub fn push(&mut self, element: T) -> Result<(), MmappedError> {
        if self.length == self.capacity {
            return Err(MmappedError::Full);
        }

        self.inner[self.length] = element;
        self.length += 1;

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.length = 0;
    }

    pub fn truncate(&mut self, len: usize) {
        self.length = len;
    }
}

impl<T: Clone> MmappedVec<T> {
    pub fn extend_from_slice(&mut self, slice: &[T]) -> Result<(), MmappedError> {
        let start = self.length;
        let end = start + slice.len();

        if end > self.capacity {
            return Err(MmappedError::Full);
        }

        self.inner[start..end].clone_from_slice(slice);
        self.length += slice.len();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmaped() {
        let mut vec = MmappedVec::with_capacity(10);
        for i in 0..vec.capacity() {
            vec.push(i).unwrap();
        }
        assert!(vec.push(10).is_err());
        assert_eq!(&vec[..], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        std::mem::drop(vec);

        let mut vec = MmappedVec::with_capacity(10);
        vec.extend_from_slice(&[]).unwrap();
        vec.extend_from_slice(&[1, 2]).unwrap();
        vec.extend_from_slice(&[3, 4, 5, 6, 7]).unwrap();
        assert!(vec.extend_from_slice(&[8, 9, 10, 11]).is_err());
        vec.extend_from_slice(&[8, 9, 10]).unwrap();
        assert!(vec.push(10).is_err());
        assert_eq!(&vec[..], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }
}
