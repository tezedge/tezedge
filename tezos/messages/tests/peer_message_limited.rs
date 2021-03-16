use failure::Error;
use std::fmt;
use tezos_encoding::encoding::{Encoding, SchemaType};
use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

#[derive(PartialEq, Eq)]
enum Limit {
    Some(usize),
    Unlimited,
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Limit::Some(size) => write!(f, "{}", size),
            Limit::Unlimited => write!(f, "unlimited"),
        }
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

fn message(msg: String) {
    println!("{}", msg)
}

fn append<T: AsRef<str> + ?Sized>(path: &String, suffix: &T) -> String {
    format!("{}.{}", path, suffix.as_ref())
}

fn visit_encoding(encoding: &Encoding, path: String) -> Limit {
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
                let tag_id = format!("{:#04X}", tag.get_id());
                let tag_path = append(&path, &tag_id);
                let size = visit_encoding(tag.get_encoding(), tag_path);
                message(format!("{}: variant {} size: {}", path, tag_id, size));
                max = cmp::max(max, size);
            }
            max + *size
        }
        List(encoding) => {
            let element_size = visit_encoding(encoding, append(&path, "[]"));
            message(format!("{}: list element size: {}", path, element_size));
            Unlimited
        }
        BoundedList(max, encoding) => {
            let element_size = visit_encoding(encoding, append(&path, "[]"));
            message(format!("{}: list element size: {}", path, element_size));
            message(format!("{}: max elements: {}", path, max));
            let size = element_size * max;
            size
        }
        Enum => 1.into(),
        Option(encoding) => visit_encoding(encoding, path) + 1,
        OptionalField(encoding) => visit_encoding(encoding, path),
        Obj(fields) => {
            let mut sum = 0.into();
            for field in fields {
                let field_path = append(&path, field.get_name());
                let size = visit_encoding(field.get_encoding(), field_path);
                message(format!(
                    "{}: field {}: size {}",
                    path,
                    field.get_name(),
                    size
                ));
                sum = sum + size;
            }
            sum
        }
        Dynamic(encoding) => {
            let size = visit_encoding(encoding, path.clone());
            message(format!(
                "{}: dynamic encoding, enclosing encoding size: {}",
                path, size
            ));
            size + 4
        }
        BoundedDynamic(max, encoding) => {
            let size = visit_encoding(encoding, path.clone());
            message(format!(
                "{}: dynamic encoding, enclosing encoding size: {}",
                path, size
            ));
            message(format!("{}: max size: {}", path, max));
            cmp::min(max.into(), size) + 4
        }
        Sized(size, encoding) => {
            {
                let size = visit_encoding(encoding, path.clone());
                message(format!(
                    "{}: fixing size for encoding of size: {}",
                    path, size
                ));
            }
            message(format!("{}: sized encoding, size: {}", path, size));
            size.into()
        }
        Bounded(size, encoding) => {
            {
                let size = visit_encoding(encoding, path.clone());
                message(format!("{}: encoding size: {}", path, size));
            }
            message(format!("{}: bounded encoding, max size: {}", path, size));
            size.into()
        }
        Split(func) => visit_encoding(&(func)(SchemaType::Binary), path),
        Lazy(func) => visit_encoding(&(func)(), path),
        Custom(_) => 100.into(),
        _ => unimplemented!(),
    }
}

use tezos_encoding::encoding::HasEncoding;

#[test]
fn peer_message_limited() -> Result<(), Error> {
    let limit = visit_encoding(
        &PeerMessageResponse::encoding(),
        String::from("PeerMessageResponse"),
    );
    message(format!("PeerMessageResponse: max size is {}", limit));
    assert!(limit != Limit::Unlimited);
    Ok(())
}
