use crate::error;
use core::num::NonZeroU64;

/// The exponent `e` of the RSA public key.
#[derive(Clone, Copy, Debug)]
pub struct Exponent(NonZeroU64);

impl Exponent {
    #[cfg(test)]
    const ALL_CONSTANTS: [Self; 3] = [Self::_3, Self::_65537, Self::MAX];

    // TODO: Use `NonZeroU64::new(...).unwrap()` when `feature(const_panic)` is
    // stable.
    pub(in crate::rsa) const _3: Self = Self(unsafe { NonZeroU64::new_unchecked(3) });
    pub(in crate::rsa) const _65537: Self = Self(unsafe { NonZeroU64::new_unchecked(65537) });

    // This limit was chosen to bound the performance of the simple
    // exponentiation-by-squaring implementation in `elem_exp_vartime`. In
    // particular, it helps mitigate theoretical resource exhaustion attacks. 33
    // bits was chosen as the limit based on the recommendations in [1] and
    // [2]. Windows CryptoAPI (at least older versions) doesn't support values
    // larger than 32 bits [3], so it is unlikely that exponents larger than 32
    // bits are being used for anything Windows commonly does.
    //
    // [1] https://www.imperialviolet.org/2012/03/16/rsae.html
    // [2] https://www.imperialviolet.org/2012/03/17/rsados.html
    // [3] https://msdn.microsoft.com/en-us/library/aa387685(VS.85).aspx
    //
    // TODO: Use `NonZeroU64::new(...).unwrap()` when `feature(const_panic)` is
    // stable.
    const MAX: Self = Self(unsafe { NonZeroU64::new_unchecked((1u64 << 33) - 1) });

    pub fn from_be_bytes(
        input: untrusted::Input,
        min_value: Self,
    ) -> Result<Self, error::KeyRejected> {
        if input.len() > 5 {
            return Err(error::KeyRejected::too_large());
        }
        let value = input.read_all(error::KeyRejected::invalid_encoding(), |input| {
            // The exponent can't be zero and it can't be prefixed with
            // zero-valued bytes.
            if input.peek(0) {
                return Err(error::KeyRejected::invalid_encoding());
            }
            let mut value = 0u64;
            loop {
                let byte = input
                    .read_byte()
                    .map_err(|untrusted::EndOfInput| error::KeyRejected::invalid_encoding())?;
                value = (value << 8) | u64::from(byte);
                if input.at_end() {
                    return Ok(value);
                }
            }
        })?;

        // Step 2 / Step b. NIST SP800-89 defers to FIPS 186-3, which requires
        // `e >= 65537`. We enforce this when signing, but are more flexible in
        // verification, for compatibility. Only small public exponents are
        // supported.
        let value = NonZeroU64::new(value).ok_or_else(error::KeyRejected::too_small)?;
        if value < min_value.0 {
            return Err(error::KeyRejected::too_small());
        }
        if value.get() & 1 != 1 {
            return Err(error::KeyRejected::invalid_component());
        }
        if value > Self::MAX.0 {
            return Err(error::KeyRejected::too_large());
        }

        Ok(Self(value))
    }
}

impl From<Exponent> for NonZeroU64 {
    fn from(Exponent(value): Exponent) -> Self {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn test_public_exponent_debug() {
        let exponent =
            Exponent::from_be_bytes(untrusted::Input::from(&[0x1, 0x00, 0x01]), Exponent::_65537)
                .unwrap();
        assert_eq!("Exponent(65537)", format!("{:?}", exponent));
    }

    #[test]
    fn test_public_exponent_constants() {
        for value in Exponent::ALL_CONSTANTS.iter() {
            let value: u64 = value.0.into();
            assert_eq!(value & 1, 1);
            assert!(value >= Exponent::_3.0.into()); // The absolute minimum.
            assert!(value <= Exponent::MAX.0.into());
        }
    }
}
