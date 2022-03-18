use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    T: IntoIterator<Item = u8>,
{
    fn from(value: T) -> Self {
        Self {
            content: value.into_iter().collect::<Vec<u8>>(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum UniString {
    Valid(String),
    Invalid(InvalidString),
}

impl UniString {
    #[inline]
    pub fn is_valid(&self) -> bool {
        matches!(*self, UniString::Valid(_))
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }
}

impl<T> From<T> for UniString
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        Self::Valid(value.into())
    }
}

// Timestamp
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum TimeStamp {
    Integral(i64),
    /// RFC 3339 (1996-12-19T16:39:57-08:00)
    Rfc(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::test_helpers::*;
    use serde_json;

    mod timestamp {
        use super::*;

        #[test]
        fn encoded_equals_decoded() -> Result<(), serde_json::Error> {
            for expected in &[
                TimeStamp::Integral(10),
                TimeStamp::Rfc("1996-12-19T16:39:57-08:00".to_string()),
            ] {
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
            custom_decoded(&message, TimeStamp::Integral(original))
        }

        #[test]
        fn decode_custom_rfc() -> Result<(), serde_json::Error> {
            let original = "1996-12-19T16:39:57-08:00".to_string();
            let message = format!("\"{}\"", original);
            custom_decoded(&message, TimeStamp::Rfc(original))
        }

        #[test]
        fn encode_custom_integral() -> Result<(), serde_json::Error> {
            let value = 10;
            let original = TimeStamp::Integral(value);
            custom_encoded(original, &format!("{}", value))
        }

        #[test]
        fn encode_custom_rfc() -> Result<(), serde_json::Error> {
            let value = "1996-12-19T16:39:57-08:00".to_string();
            let original = TimeStamp::Rfc(value.clone());
            custom_encoded(original, &format!("\"{}\"", value))
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
            let message = format!(
                "{{\"invalid_utf8_string\":{}}}",
                iter_to_string(original.iter())
            );
            custom_decoded(&message, InvalidString { content: original })
        }

        #[test]
        fn encode_custom() -> Result<(), serde_json::Error> {
            let message: Vec<u8> = "InvalidString".bytes().collect();
            let original = InvalidString {
                content: message.clone(),
            };
            custom_encoded(
                original,
                &format!(
                    "{{\"invalid_utf8_string\":{}}}",
                    iter_to_string(message.iter())
                ),
            )
        }
    }

    mod uni_string {
        use super::*;

        #[test]
        fn encoded_equals_decoded() -> Result<(), serde_json::Error> {
            for expected in &[
                UniString::Valid("String".into()),
                UniString::Invalid(vec![32, 32].into()),
            ] {
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
            custom_decoded(&message, UniString::Valid(original_content))
        }

        #[test]
        fn decode_custom_invalid_string() -> Result<(), serde_json::Error> {
            let original_content: Vec<u8> = "UniString::InvalidString".bytes().collect();
            let message = format!(
                "{{\"invalid_utf8_string\":{}}}",
                iter_to_string(original_content.iter())
            );
            custom_decoded(&message, UniString::Invalid(original_content.into()))
        }

        #[test]
        fn encode_custom_string() -> Result<(), serde_json::Error> {
            let message = "UniString::String".to_string();
            let original = UniString::Valid(message.clone());
            custom_encoded(original, &format!("\"{}\"", message))
        }

        #[test]
        fn encode_custom_invalid_string() -> Result<(), serde_json::Error> {
            let message: Vec<u8> = "UniString::InvalidString".bytes().collect();
            let original = UniString::Invalid(message.clone().into());
            custom_encoded(
                original,
                &format!(
                    "{{\"invalid_utf8_string\":{}}}",
                    iter_to_string(message.iter())
                ),
            )
        }
    }
}
