// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use anyhow::Error;
use std::{borrow::Cow, collections::HashMap, fmt, path::PathBuf, rc::Rc};
use tezos_encoding::encoding::{Encoding, Field};
use tezos_messages::p2p::encoding::{
    connection::ConnectionMessage, metadata::MetadataMessage, peer::PeerMessageResponse,
};

mod message_limit;
use message_limit::*;

fn get_contents(
    context: &str,
    encoding: &Encoding,
    infos: &mut HashMap<String, Rc<MessageInfo>>,
) -> FieldContents {
    match encoding {
        Encoding::Unit => "empty".into(),
        Encoding::Int8 => "signed byte".into(),
        Encoding::Uint8 => "unsigned byte".into(),
        Encoding::Int16 => "signed 16-bit integer".into(),
        Encoding::Uint16 => "unsigned 16-bit integer".into(),
        Encoding::Int31 => "signed 31-bit integer".into(),
        Encoding::Int32 => "signed 32-bit integer".into(),
        Encoding::Uint32 => "unsigned 32-bit integer".into(),
        Encoding::Int64 => "signed 64-bit integer".into(),
        Encoding::RangedInt => "range of integers".into(),
        Encoding::Z => "Z-encoded big integer".into(),
        Encoding::Mutez => "MuteZ-encoded big integer".into(),
        Encoding::Float => "64-bit float".into(),
        Encoding::RangedFloat => "range of 64-bit floats".into(),
        Encoding::Bool => "boolean".into(),
        Encoding::String => "UTF8-encoded string".into(),
        Encoding::BoundedString(limit) => {
            format!("UTF8-encoded string with {} elements max", limit).into()
        }
        Encoding::Bytes => "fixed-length sequence of bytes".into(),
        Encoding::List(encoding) => match encoding.as_ref() {
            Encoding::Dynamic(_) | Encoding::BoundedDynamic(_, _) => {
                let name = format!("{} items", context);
                let mut info = MessageInfo::new(name.clone());
                add_fields(&mut info, &String::from("item"), encoding, infos);
                let info = Rc::new(info);
                infos.insert(name, info.clone());
                FieldContents::list(Limit::Var, FieldContents::Reference(info))
            }
            _ => FieldContents::list(Limit::Var, get_contents(context, encoding, infos)),
        },
        Encoding::BoundedList(limit, encoding) => match encoding.as_ref() {
            Encoding::Dynamic(_) | Encoding::BoundedDynamic(_, _) => {
                let name = format!("{} items", context);
                let mut info = MessageInfo::new(name.clone());
                add_fields(&mut info, &String::from("list element"), encoding, infos);
                let info = Rc::new(info);
                infos.insert(name, info);
                FieldContents::list(Limit::UpTo(*limit), get_contents(context, encoding, infos))
            }
            _ => FieldContents::list(Limit::UpTo(*limit), get_contents(context, encoding, infos)),
        },
        Encoding::Obj(name, fields) => {
            let info = get_info(name, fields, infos);
            FieldContents::Reference(info)
        }
        Encoding::Hash(hash) => hash.as_ref().into(),
        Encoding::Sized(size, encoding) => {
            FieldContents::sized(*size, get_contents(context, encoding, infos))
        }
        Encoding::Bounded(_, encoding) => get_contents(context, encoding, infos),
        Encoding::Timestamp => "timestamp".into(),
        Encoding::Custom => "Merkle tree path encoding".into(),
        _ => todo!(
            "Getting contents description for unhandled encoding: {:?}",
            encoding
        ),
    }
}

use tezos_encoding::encoding::HasEncoding;

#[derive(Default)]
struct MessageInfo {
    name: String,
    size: Limit,
    fields: Vec<MessageField>,
}

impl MessageInfo {
    fn new<'a, T: Into<Cow<'a, str>>>(name: T) -> Self {
        let name = name.into().into_owned();
        Self {
            name,
            ..Default::default()
        }
    }

    fn add_field<'a, T: Into<Cow<'a, str>>>(
        &mut self,
        name: T,
        size: Limit,
        contents: FieldContents,
    ) {
        let contents = contents;
        let name = name.into().into_owned();
        self.fields.push(MessageField {
            name,
            size,
            contents,
        });
        self.size += size;
    }
}

impl std::fmt::Display for MessageInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "
### {}
Size: {}

| Name | Size | Contents |
|:-----|:-----|:---------|",
            self.name, self.size,
        )?;
        self.fields.iter().try_for_each(|field| {
            writeln!(
                f,
                "| {} | {} | {} |",
                field.name, field.size, field.contents
            )
        })?;
        Ok(())
    }
}

enum FieldContents {
    Comment(String),
    Reference(Rc<MessageInfo>),
    List(Limit, Box<FieldContents>),
    Sized(usize, Box<FieldContents>),
}

fn info_id(info: &MessageInfo) -> String {
    info.name
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ch.is_whitespace())
        .map(|ch| if ch.is_whitespace() { '-' } else { ch })
        .collect::<String>()
        .to_lowercase()
}

fn info_ref(info: &MessageInfo) -> String {
    let name = &info.name;
    let id = info_id(info);
    format!("[{}](#{})", name, id)
}

impl fmt::Display for FieldContents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldContents::Comment(comment) => write!(f, "{}", comment),
            FieldContents::Reference(msg) => write!(f, "{}", info_ref(msg)),
            FieldContents::List(limit, contents) => match limit {
                Limit::Fixed(limit) => write!(f, "{} of {}", limit, contents),
                Limit::UpTo(limit) => write!(f, "up to {} of {}", limit, contents),
                Limit::Var => write!(f, "list of {}", contents),
            },
            FieldContents::Sized(_, contents) => {
                write!(f, "{}", contents)
            }
        }
    }
}

impl<'a, T: Into<Cow<'a, str>>> From<T> for FieldContents {
    fn from(source: T) -> Self {
        FieldContents::Comment(source.into().into_owned())
    }
}

impl FieldContents {
    fn sized(limit: usize, contents: Self) -> Self {
        Self::Sized(limit, Box::new(contents))
    }

    fn list(limit: Limit, contents: Self) -> Self {
        Self::List(limit, Box::new(contents))
    }
}

struct MessageField {
    name: String,
    size: Limit,
    contents: FieldContents,
}

type MessageInfoList = Vec<Rc<MessageInfo>>;

fn add_fields(
    info: &mut MessageInfo,
    name: &str,
    encoding: &Encoding,
    infos: &mut HashMap<String, Rc<MessageInfo>>,
) {
    match encoding {
        Encoding::Dynamic(encoding) => {
            info.add_field(
                "# of bytes in the next field",
                4.into(),
                "unsigned 32-bit integer".into(),
            );
            add_fields(info, name, encoding, infos);
        }
        Encoding::BoundedDynamic(limit, encoding) => {
            info.add_field(
                format!("# of bytes in the next field (up to {})", limit),
                4.into(),
                "unsigned 32-bit integer".into(),
            );
            add_fields(info, name, encoding, infos);
        }
        Encoding::OptionalField(encoding) => {
            info.add_field(
                "presense of the next field",
                1.into(),
                "0xff if presend, 0x00 if absent ".into(),
            );
            add_fields(info, name, encoding, infos);
        }
        _ => {
            let size = get_max_size(encoding);
            let contents = get_contents(&format!("{}.{}", info.name, name), encoding, infos);
            info.add_field(name, size, contents);
        }
    }
}

fn get_obj_info(
    encoding: &Encoding,
    infos: &mut HashMap<String, Rc<MessageInfo>>,
) -> Rc<MessageInfo> {
    match encoding {
        Encoding::Obj(name, fields) => {
            if let Limit::Fixed(limit) | Limit::UpTo(limit) = get_max_size(encoding) {
                assert!(
                    limit
                        <= tezos_messages::p2p::binary_message::CONTENT_LENGTH_MAX
                            - crypto::crypto_box::BOX_ZERO_BYTES,
                    "Message {} may not fit in a single chunk",
                    name
                );
            } else {
                panic!("Message {} is not bounded", name);
            }
            get_single_info(name, fields, infos)
        }
        _ => unreachable!(),
    }
}

fn get_single_info(
    name: &str,
    fields: &[Field],
    infos: &mut HashMap<String, Rc<MessageInfo>>,
) -> Rc<MessageInfo> {
    if let Some(info) = infos.get(name) {
        return info.clone();
    }

    let mut info = MessageInfo::new(name);

    for field in fields {
        add_fields(&mut info, field.get_name(), field.get_encoding(), infos);
    }

    Rc::new(info)
}

fn get_info(
    name: &str,
    fields: &[Field],
    infos: &mut HashMap<String, Rc<MessageInfo>>,
) -> Rc<MessageInfo> {
    let info = get_single_info(name, fields, infos);
    infos.insert(name.to_owned(), info.clone());
    info
}

fn peer_message_response(infos: &mut HashMap<String, Rc<MessageInfo>>) -> MessageInfoList {
    use Encoding::*;
    let mut pre_size = Limit::default();
    if let BoundedDynamic(total_size, obj_encoding) = PeerMessageResponse::encoding() {
        pre_size += 4; // Dynamic encoding size, 4 bytes
        if let Obj(_, ref fields) = *obj_encoding {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].get_name(), &"message".to_string());
            if let Tags(tag_size, tag_map) = fields[0].get_encoding() {
                let mut res = vec![];
                pre_size += *tag_size; // number of bytes used to encode a tag

                let mut max = Limit::default();
                let mut tags = tag_map
                    .tags()
                    .collect::<Vec<&tezos_encoding::encoding::Tag>>();
                tags.sort_by_key(|a| a.get_id());
                for tag in tags {
                    let tag_id = format!("{:#04X}", tag.get_id());

                    let mut info = MessageInfo::new(format!("P2P {}", tag.get_variant()));
                    info.add_field(
                        "# of bytes in the message",
                        4.into(),
                        "unsigned 32-bit integer".into(),
                    );
                    info.add_field(
                        "tag",
                        (*tag_size).into(),
                        format!("tag corresponding to the message ({})", tag_id).into(),
                    );

                    let tag_path = tag.get_variant();
                    let size = get_max_size(tag.get_encoding());
                    assert!(size.is_limited(), "Size for {} should be limited", tag_path);

                    add_fields(&mut info, fields[0].get_name(), tag.get_encoding(), infos);

                    res.push(Rc::new(info));

                    max = max.union(size);
                }

                if let Limit::UpTo(limit) = pre_size + max {
                    assert!(limit <= total_size, "PeerMessageResponse is limited to {} and cannot contain message of size {}", total_size, limit);
                }

                return res;
            }
        }
    }
    unreachable!("Unexpected encoding for PeerMessageResponse");
}

#[test]
fn peer_message_limited() -> Result<(), Error> {
    use std::io::prelude::*;

    let mut infos = HashMap::new();

    let mut file =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()));
    file.push("Messages.md");
    let mut file = std::fs::File::create(file)?;

    writeln!(file, "# P2P Messages\n")?;
    writeln!(file, "## Handshaking Messages")?;
    [ConnectionMessage::encoding(), MetadataMessage::encoding()]
        .iter()
        .map(|encoding| get_obj_info(encoding, &mut infos))
        .try_for_each(|info| writeln!(file, "{}", info))?;

    writeln!(file, "## Distributed DB Messages")?;
    peer_message_response(&mut infos)
        .iter()
        .try_for_each(|info| writeln!(file, "{}", info))?;

    writeln!(file, "## Auxiliary Types")?;
    let mut infos = infos.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
    infos.sort_by_key(|info| info.name.to_string());
    infos
        .iter()
        .try_for_each(|info| writeln!(file, "{}", info))?;

    Ok(())
}
