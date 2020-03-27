pub mod base_types;
pub mod monitor;
pub mod chain;
pub mod conversions;

#[cfg(test)]
pub mod test_helpers {
    pub fn iter_to_string<I, T>(it: I) -> String
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

    pub fn custom_encoded<T>(original: T, value: &str) -> Result<(), serde_json::Error>
        where
            T: serde::Serialize,
    {
        let encoded = serde_json::to_string(&original)?;
        assert_eq!(&encoded, value);
        Ok(())
    }

    pub fn custom_decoded<'a, T>(message: &'a str, expected: T) -> Result<(), serde_json::Error>
        where
            T: serde::Deserialize<'a>,
            T: std::fmt::Debug,
            T: Eq,
    {
        let decoded: T = serde_json::from_str(message)?;
        assert_eq!(decoded, expected);
        Ok(())
    }
}