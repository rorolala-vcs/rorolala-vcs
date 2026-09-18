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
