// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use byte_unit::Byte;

use tezos_context::{working_tree::working_tree::WorkingTreeStatistics, ObjectHash};

pub struct DebugWorkingTreeStatistics(pub WorkingTreeStatistics);

enum Numbers {
    TotalInlined {
        total: usize,
        inlined: usize,
        not_inlined: usize,
    },
    TotalUnique {
        total: usize,
        unique: usize,
    },
    Unique {
        unique: usize,
    },
}

impl std::fmt::Debug for Numbers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Numbers::TotalInlined {
                total,
                inlined,
                not_inlined,
            } => f.write_fmt(format_args!(
                "{{ total: {:>8}, inlined: {:>8}, not inlined: {:>8} }}",
                total, inlined, not_inlined,
            )),
            Numbers::TotalUnique { total, unique } => {
                let total_bytes = total * std::mem::size_of::<ObjectHash>();
                let total_str = Byte::from_bytes(total_bytes as u64)
                    .get_appropriate_unit(false)
                    .to_string();

                let unique_bytes = unique * std::mem::size_of::<ObjectHash>();
                let unique_str = Byte::from_bytes(unique_bytes as u64)
                    .get_appropriate_unit(false)
                    .to_string();

                let duplicated = total - unique;
                let duplicated_bytes = duplicated * std::mem::size_of::<ObjectHash>();
                let duplicated_str = Byte::from_bytes(duplicated_bytes as u64)
                    .get_appropriate_unit(false)
                    .to_string();

                f.write_fmt(format_args!(
                    "{{ total: {:>8} ({}), unique: {:>8} ({}), duplicate: {:>8} ({}) }}",
                    total, total_str, unique, unique_str, duplicated, duplicated_str,
                ))
            }
            Numbers::Unique { unique } => f.write_fmt(format_args!("{{ unique: {:>8} }}", unique,)),
        }
    }
}

struct BytesDisplay {
    bytes: usize,
}

impl std::fmt::Debug for BytesDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes_str = Byte::from_bytes(self.bytes as u64)
            .get_appropriate_unit(false)
            .to_string();

        f.write_fmt(format_args!("{:>8} ({:>8})", self.bytes, bytes_str,))
    }
}

impl std::fmt::Debug for DebugWorkingTreeStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut blobs_stats = Vec::with_capacity(self.0.blobs_by_length.len());
        for stats in self.0.blobs_by_length.values() {
            blobs_stats.push(stats);
        }
        blobs_stats.sort_by_key(|stats| stats.size);

        let mut dir_stats = Vec::with_capacity(self.0.directories_by_length.len());
        for stats in self.0.directories_by_length.values() {
            dir_stats.push(stats);
        }
        dir_stats.sort_by_key(|stats| stats.size);

        let total_bytes = self.0.nhashes * std::mem::size_of::<ObjectHash>()
            + self.0.strings_total_bytes
            + self.0.objects_total_bytes
            + self.0.shapes_total_bytes;

        f.debug_struct("WorkingTreeStatistics")
            .field("blobs_by_length", &blobs_stats)
            .field("directories_by_length", &dir_stats)
            .field("max_depth", &self.0.max_depth)
            .field(
                "oldest_reference (absolute offset in data.db file) ",
                &self.0.lowest_offset,
            )
            .field(
                "number_of_objects (directories + blobs)",
                &Numbers::TotalInlined {
                    total: self.0.nobjects,
                    inlined: self.0.nobjects_inlined,
                    not_inlined: self.0.nobjects - self.0.nobjects_inlined,
                },
            )
            .field(
                "number_of_hashes ",
                &Numbers::TotalUnique {
                    total: self.0.nhashes,
                    unique: self.0.unique_hash.len(),
                },
            )
            .field(
                "number_of_shapes ",
                &Numbers::Unique {
                    unique: self.0.nshapes,
                },
            )
            .field("number_of_directories ", &self.0.ndirectories)
            .field(
                "hashes_total_bytes (hashes.db file)",
                &BytesDisplay {
                    bytes: self.0.nhashes * std::mem::size_of::<ObjectHash>(),
                },
            )
            .field(
                "strings_total_bytes (small & big strings)",
                &BytesDisplay {
                    bytes: self.0.strings_total_bytes,
                },
            )
            .field(
                "objects_total_bytes (data.db file)",
                &BytesDisplay {
                    bytes: self.0.objects_total_bytes,
                },
            )
            .field(
                "shapes_total_bytes (shape_directories.db file)",
                &BytesDisplay {
                    bytes: self.0.shapes_total_bytes,
                },
            )
            .field("total_bytes", &BytesDisplay { bytes: total_bytes })
            .finish()
    }
}
