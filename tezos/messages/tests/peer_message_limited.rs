use failure::Error;
use std::fmt::{self, Write};
use tezos_encoding::encoding::{Encoding, SchemaType};
use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Limit {
    Some(usize),
    Unlimited,
}

impl Limit {
    fn is_limited(&self) -> bool {
        match self {
            Limit::Some(_) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Limit::Some(size) => write!(f, "{}", size),
            Limit::Unlimited => write!(f, "unlimited"),
        }
    }
}

impl Default for Limit {
    fn default() -> Self {
        0.into()
    }
}

use std::ops;

impl ops::Add for Limit {
    type Output = Limit;
    fn add(self, other: Self) -> Self {
        match (self, other) {
            (Limit::Some(a), Limit::Some(b)) => (a + b).into(),
            _ => Limit::Unlimited,
        }
    }
}

impl ops::Add<usize> for Limit {
    type Output = Limit;
    fn add(self, other: usize) -> Self {
        match self {
            Limit::Some(a) => (a + other).into(),
            _ => Limit::Unlimited,
        }
    }
}

impl ops::AddAssign<usize> for Limit {
    fn add_assign(&mut self, other: usize) {
        match self {
            Limit::Some(ref mut a) => *a += other,
            _ => (),
        }
    }
}

impl ops::Mul for Limit {
    type Output = Limit;
    fn mul(self, other: Self) -> Self {
        match (self, other) {
            (Limit::Some(a), Limit::Some(b)) => (a * b).into(),
            _ => Limit::Unlimited,
        }
    }
}

impl ops::Mul<usize> for Limit {
    type Output = Limit;
    fn mul(self, other: usize) -> Self {
        match self {
            Limit::Some(a) => (a * other).into(),
            _ => Limit::Unlimited,
        }
    }
}

impl ops::Mul<&usize> for Limit {
    type Output = Limit;
    fn mul(self, other: &usize) -> Self {
        match self {
            Limit::Some(a) => (a * *other).into(),
            _ => Limit::Unlimited,
        }
    }
}

use std::cmp;

impl cmp::Ord for Limit {
    fn cmp(&self, other: &Limit) -> cmp::Ordering {
        match (&self, other) {
            (Limit::Some(a), Limit::Some(b)) => a.cmp(b),
            (Limit::Some(_), Limit::Unlimited) => cmp::Ordering::Less,
            (Limit::Unlimited, Limit::Some(_)) => cmp::Ordering::Greater,
            _ => cmp::Ordering::Equal,
        }
    }
}

impl cmp::PartialOrd for Limit {
    fn partial_cmp(&self, other: &Limit) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<usize> for Limit {
    fn from(source: usize) -> Self {
        Limit::Some(source)
    }
}

impl From<&usize> for Limit {
    fn from(source: &usize) -> Self {
        Limit::Some(*source)
    }
}

fn append<T: AsRef<str> + ?Sized>(path: &String, suffix: &T) -> String {
    format!("{}.{}", path, suffix.as_ref())
}

fn visit_encoding(encoding: &Encoding, path: String, out: &mut dyn Write) -> Limit {
    use Encoding::*;
    use Limit::Unlimited;
    match encoding {
        Unit => 0.into(),
        Int8 | Uint8 | Bool => 1.into(),
        Int16 | Uint16 => 2.into(),
        Int31 | Int32 | Uint32 => 4.into(),
        Int64 | RangedInt | Float | Timestamp => 8.into(),
        RangedFloat => 16.into(),
        Z | Mutez => Unlimited,
        Hash(hash) => hash.size().into(),
        String => Unlimited,
        BoundedString(max) => max.into(),
        Bytes => Unlimited,
        Tags(size, map) => {
            let mut max = 0.into();
            let mut tags = map.tags().collect::<Vec<&tezos_encoding::encoding::Tag>>();
            tags.sort_by(|a, b| a.get_id().cmp(&b.get_id()));
            for tag in tags {
                let tag_id = format!("{}({:#04X})", tag.get_variant(), tag.get_id());
                let tag_path = append(&path, &tag_id);
                let size = visit_encoding(tag.get_encoding(), tag_path, out);
                writeln!(out, "{}: variant {} size: {}", path, tag_id, size).unwrap();
                max = cmp::max(max, size);
            }
            max + *size
        }
        List(encoding) => {
            let element_size = visit_encoding(encoding, append(&path, "[]"), out);
            writeln!(
                out,
                "{}: unlimited list, max element size: {}",
                path, element_size
            )
            .unwrap();
            Unlimited
        }
        BoundedList(max, encoding) => {
            let element_size = visit_encoding(encoding, append(&path, "[]"), out);
            writeln!(
                out,
                "{}: list with max {} elements, max element size: {}",
                path, max, element_size
            )
            .unwrap();
            element_size * max
        }
        Enum => 1.into(),
        Option(encoding) => visit_encoding(encoding, path, out) + 1,
        OptionalField(encoding) => visit_encoding(encoding, path, out),
        Obj(fields) => {
            let mut sum = 0.into();
            for field in fields {
                let field_path = append(&path, field.get_name());
                let size = visit_encoding(field.get_encoding(), field_path, out);
                writeln!(out, "{}: field {}: size {}", path, field.get_name(), size).unwrap();
                sum = sum + size;
            }
            sum
        }
        Dynamic(encoding) => {
            let size = visit_encoding(encoding, path.clone(), out);
            writeln!(
                out,
                "{}: dynamic encoding, enclosing encoding size: {}",
                path, size
            )
            .unwrap();
            size + 4
        }
        BoundedDynamic(max, encoding) => {
            let size = visit_encoding(encoding, path.clone(), out);
            writeln!(
                out,
                "{}: dynamic encoding bounded by {}, enclosing encoding size: {}",
                path, max, size
            )
            .unwrap();
            cmp::min(max.into(), size) + 4
        }
        Sized(fixed_size, encoding) => {
            let size = visit_encoding(encoding, path.clone(), out);
            writeln!(
                out,
                "{}: fixed size encoding: {} enclosing encoding size: {}",
                path, fixed_size, size
            )
            .unwrap();
            fixed_size.into()
        }
        Bounded(bounded_size, encoding) => {
            let size = visit_encoding(encoding, path.clone(), out);
            writeln!(
                out,
                "{}: encoding bounded by {}, enclosing encoding size: {}",
                path, bounded_size, size
            )
            .unwrap();
            bounded_size.into()
        }
        Split(func) => visit_encoding(&(func)(SchemaType::Binary), path, out),
        Lazy(func) => visit_encoding(&(func)(), path, out),
        Custom(_) => 100.into(), // 3 hashes, three left/right tags, one op tag, 3 * (32 + 1) + 1
        _ => unimplemented!(),
    }
}

use tezos_encoding::encoding::HasEncoding;

#[test]
fn peer_message_limited() -> Result<(), Error> {
    use Encoding::*;
    let path = "PeerMessageResponse".to_string();
    let mut pre_size = Limit::default();
    if let BoundedDynamic(total_size, obj_encoding) = PeerMessageResponse::encoding() {
        pre_size += 4; // Dynamic encoding size, 4 bytes
        if let Obj(ref fields) = **obj_encoding {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].get_name(), &"message".to_string());
            if let Tags(tag_size, tag_map) = fields[0].get_encoding() {
                pre_size += *tag_size; // number of bytes used to encode a tag

                let mut details = std::string::String::new();
                let mut max = Limit::default();
                let mut tags = tag_map
                    .tags()
                    .collect::<Vec<&tezos_encoding::encoding::Tag>>();
                tags.sort_by(|a, b| a.get_id().cmp(&b.get_id()));
                for tag in tags {
                    let tag_id = format!("{}({:#04X})", tag.get_variant(), tag.get_id());
                    let tag_path = append(&path, &tag_id);
                    let size = visit_encoding(tag.get_encoding(), tag_path.clone(), &mut details);
                    assert!(size.is_limited(), "Size for {} should be limited", tag_path);
                    println!("{}: maximum size: {}", tag_path, pre_size + size);
                    max = cmp::max(max, size);
                }

                if let Limit::Some(limit) = pre_size + max {
                    assert!(limit <= *total_size, "PeerMessageResponse is limited to {} and cannot contain message of size {}", total_size, limit);
                }

                println!("\nPeerMessageResponse's content maximum size is {}", max);
                println!(
                    "PeerMessageResponse's encoding is limited to {}",
                    total_size
                );
                println!("\nDetails on inner encodings:\n\n{}", details);

                return Ok(());
            }
        }
    }
    unreachable!("Unexpected encoding for PeerMessageResponse");
}
