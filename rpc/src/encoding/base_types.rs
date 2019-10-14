use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvalidString {
    #[serde(rename = "invalid_utf8_string")]
    content: Vec<u8>,
}

impl InvalidString {
    #[inline]
    pub fn content(&self) -> &Vec<u8> {
        &self.content
    }
}

impl<T> From<T> for InvalidString
    where
        T: IntoIterator<Item=u8>
{
    fn from(value: T) -> Self {
        Self {
            content: value.into_iter().collect::<Vec<u8>>()
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum UniString {
    Valid(String),
    Invalid(InvalidString),
}

impl UniString {
    #[inline]
    pub fn is_valid(&self) -> bool {
        match *self {
            UniString::Valid(_) => true,
            _ => false,
        }
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }
}

// Timestamp
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum TimeStamp {
    Integral(i64),
    /// RFC 3339 (1996-12-19T16:39:57-08:00)
    Rfc(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    fn iter_to_string<I, T>(it: I) -> String
        where
            I: Iterator<Item=T>,
            T: std::fmt::Display,
    {
        format!("[{}]", it
            .map(|x| x.to_string())
            .fold(String::new(), |s, x| if s.is_empty() {
                x
            } else {
                format!("{},{}", s, x)
            }))
    }

    mod timestamp {
        use super::*;

        #[test]
        fn encoded_equals_decoded() -> Result<(), serde_json::Error> {
            for expected in &[TimeStamp::Integral(10), TimeStamp::Rfc("1996-12-19T16:39:57-08:00".to_string())] {
                let encoded = serde_json::to_string(expected)?;
                let decoded: TimeStamp = serde_json::from_str(&encoded)?;
                assert_eq!(expected, &decoded);
            }
            Ok(())
        }

        #[test]
        fn decode_custom_integral() -> Result<(), serde_json::Error> {
            let original = 10;
            let message = format!("{}", original);
            let decoded: TimeStamp = serde_json::from_str(&message)?;
            assert_eq!(decoded, TimeStamp::Integral(original));
            Ok(())
        }

        #[test]
        fn decode_custom_rfc() -> Result<(), serde_json::Error> {
            let original = "1996-12-19T16:39:57-08:00".to_string();
            let message = format!("\"{}\"", original);
            let decoded: TimeStamp = serde_json::from_str(&message)?;
            assert_eq!(decoded, TimeStamp::Rfc(original));
            Ok(())
        }

        #[test]
        fn encode_custom_integral() -> Result<(), serde_json::Error> {
            let value = 10;
            let original = TimeStamp::Integral(value.clone());
            let encoded = serde_json::to_string(&original)?;
            assert_eq!(encoded, format!("{}", value));
            Ok(())
        }

        #[test]
        fn encode_custom_rfc() -> Result<(), serde_json::Error> {
            let value = "1996-12-19T16:39:57-08:00".to_string();
            let original = TimeStamp::Rfc(value.clone());
            let encoded = serde_json::to_string(&original)?;
            assert_eq!(encoded, format!("\"{}\"", value));
            Ok(())
        }
    }

    mod invalid_string {
        use super::*;

        #[test]
        fn encoded_equals_decoded() -> Result<(), serde_json::Error> {
            let expected: InvalidString = "InvalidString".bytes().into();
            let encoded = serde_json::to_string(&expected)?;
            let decoded: InvalidString = serde_json::from_str(&encoded)?;
            assert_eq!(expected, decoded);
            Ok(())
        }

        #[test]
        fn decode_custom() -> Result<(), serde_json::Error> {
            let original: Vec<u8> = "InvalidString".bytes().collect();
            let message = format!("{{\"invalid_utf8_string\":{}}}", iter_to_string(original.iter()));
            let decoded: InvalidString = serde_json::from_str(&message)?;
            assert_eq!(decoded.content, original);
            Ok(())
        }

        #[test]
        fn encode_custom() -> Result<(), serde_json::Error> {
            let message: Vec<u8> = "InvalidString".bytes().collect();
            let original = InvalidString { content: message };
            let encoded = serde_json::to_string(&original)?;
            assert_eq!(format!("{{\"invalid_utf8_string\":{}}}", iter_to_string(original.content.iter())), encoded);
            Ok(())
        }
    }

    mod uni_string {
        use super::*;

        #[test]
        fn encoded_equals_decoded() -> Result<(), serde_json::Error> {
            for expected in &[UniString::Valid("String".into()), UniString::Invalid(vec![32, 32].into())] {
                let encoded = serde_json::to_string(&expected)?;
                let decoded: UniString = serde_json::from_str(&encoded)?;
                assert_eq!(expected, &decoded);
            }
            Ok(())
        }

        #[test]
        fn decode_custom_valid_string() -> Result<(), serde_json::Error> {
            let original_content = "UniString::String".to_string();
            let message = format!("\"{}\"", original_content);
            let decoded: UniString = serde_json::from_str(&message)?;
            if let UniString::Valid(decoded_content) = decoded {
                assert_eq!(original_content, decoded_content);
            } else {
                panic!("Expected UniString::String type");
            }
            Ok(())
        }

        #[test]
        fn decode_custom_invalid_string() -> Result<(), serde_json::Error> {
            let original_content: Vec<u8> = "UniString::InvalidString".bytes().collect();
            let message = format!("{{\"invalid_utf8_string\":{}}}", iter_to_string(original_content.iter()));
            let decoded: UniString = serde_json::from_str(&message)?;
            if let UniString::Invalid(decoded_content) = decoded {
                let original_content: InvalidString = original_content.into();
                assert_eq!(original_content, decoded_content);
            } else {
                panic!("Expected UniString::InvalidString type");
            }
            Ok(())
        }

        #[test]
        fn encode_custom_string() -> Result<(), serde_json::Error> {
            let message = "UniString::String".to_string();
            let original = UniString::Valid(message.clone());
            let encoded = serde_json::to_string(&original)?;
            assert_eq!(format!("\"{}\"", message), encoded);
            Ok(())
        }

        #[test]
        fn encode_custom_invalid_string() -> Result<(), serde_json::Error> {
            let message: Vec<u8> = "UniString::InvalidString".bytes().collect();
            let original = UniString::Invalid(message.clone().into());
            let encoded = serde_json::to_string(&original)?;
            assert_eq!(format!("{{\"invalid_utf8_string\":{}}}", iter_to_string(message.iter())), encoded);
            Ok(())
        }
    }
}