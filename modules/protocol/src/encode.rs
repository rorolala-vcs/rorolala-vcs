/// A trait for types that can be encoded into a byte vector and decoded back from one.
pub trait Encodable {
    /// Encodes `self` into a byte vector.
    ///
    /// # Errors
    ///
    /// Returns an error if `self` fails to serialize into `bincode2`.
    fn encode(&self) -> Result<Vec<u8>, bincode2::Error>;

    /// Decodes a value of this type from the given raw byte vector.
    ///
    /// # Errors
    ///
    /// Returns an error if `raw` fails to deserialize into `Self` via `bincode2`.
    fn decode(raw: Vec<u8>) -> Result<Self, bincode2::Error>
    where
        Self: Sized;
}

/// Blanket implementation of [`Encodable`] for any type that implements
/// [`serde::Serialize`] and [`serde::de::DeserializeOwned`], using `bincode2`
/// with its standard configuration for the conversion.
impl<T> Encodable for T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    /// Encodes `self` into a byte vector using `bincode2`.
    fn encode(&self) -> Result<Vec<u8>, bincode2::Error> {
        bincode2::serialize(self)
    }

    /// Decodes a value of this type from the given raw byte vector using `bincode2`.
    fn decode(raw: Vec<u8>) -> Result<Self, bincode2::Error> {
        bincode2::deserialize(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::Encodable;

    #[test]
    fn a_value_round_trips_through_its_encoding() {
        let encoded = 42_u32.encode().unwrap();
        assert_eq!(u32::decode(encoded).unwrap(), 42);
    }

    #[test]
    fn decoding_bytes_that_are_too_short_reports_a_codec_failure() {
        assert!(u32::decode(Vec::new()).is_err());
        assert!(u32::decode(vec![0, 1, 2]).is_err());
    }
}
