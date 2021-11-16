# Database Schema
## Hint entry file
**Name**|**Size**|**Rust Type**|**Contents**
:-----:|:-----:|:-----:|-----
timestamp|8| `i64`| Timestamp in utc encoded to big-endian bytes
key_size| 8| `u64`| Size of the key encoded to big-endian bytes
value_size|8| `u64`| Size of the value encoded to big-endian bytes
data_entry_position| 8| `u64`| Position of the beginning of data in `Data file`
key|variable| `Vec<u8>`| Key


Hint entry is appended to the `{file_id}.hint` with this encoding structure

```
+-----------+----------+------------+---------------------+-----+
| timestamp | key_size | value_size | data_entry_position | key |
+-----------+----------+------------+---------------------+-----+
```


## Data entry file
| **Name**    | **Size**   | **Rust Type**  | **Contents**            |
|:-----------:|:----------:|:--------------:|-------------------------|
| crc         | 4          | `u32`          | cyclic redundancy check |
| timestamp   | 8          | `i64`          | Timestamp               |
| key_size    | 8          | `u64`          | Key size                |
| value_size  | 8          | `u64`          | Value size              |
| key         | variable   | `Vec<u8>`      | Key in bytes            |
| value       | variable   | `Vec<u8>`      | Value in bytes          |

Data entry is appended to the `{file_id}.data` file with this encoding structure
```
+-----+-----------+----------+------------+-----+-------+
| crc | timestamp | key_size | value_size | key | value |
+-----+-----------+----------+------------+-----+-------+
```

## Keys Dir
Hint files are store in memory using `BtreeMap<K,V>`
where `K : Vec<u8>` read from the hint file and V is a rust struct `KeyDirEntry` 

**KeyDirEntry**

| Name                | Rust Type | Contents                    |
|:-------------------:|:---------:|-----------------------------|
| file_id             | `String`  | File name                   |
| key_size            | `u64`     | Key size                    |
| value_size          | `u64`     | Value size                  |
| data_entry_position | `u64`     | Data entry position in File |


# Usage

```rust
    let db = EdgeKV::open("db").unwrap();

    let k = b"k1";

    db.put(k.to_vec(), vec![0]).unwrap();
    db.merge(concatenate_merge, k.to_vec(), vec![1]).unwrap();
    db.merge(concatenate_merge, k.to_vec(), vec![2]).unwrap();
    assert_eq!(db.get(&k.to_vec()).unwrap().unwrap(), vec![0, 1, 2]);

    db.put(k.to_vec(), vec![3]).unwrap();
    assert_eq!(db.get(&k.to_vec()).unwrap().unwrap(), vec![3]);
```