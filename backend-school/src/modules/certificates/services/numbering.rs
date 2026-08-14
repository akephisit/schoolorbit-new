use crate::modules::certificates::models::{CertificateNumber, CertificateNumberError};

impl CertificateNumber {
    pub fn new(
        academic_year: i32,
        activity_sequence: u32,
        certificate_sequence: u32,
    ) -> Result<Self, CertificateNumberError> {
        if !(0..=9_999).contains(&academic_year) {
            return Err(CertificateNumberError::InvalidAcademicYear);
        }
        if !(1..=9_999).contains(&activity_sequence) {
            return Err(CertificateNumberError::ActivitySequenceOutOfRange);
        }
        if !(1..=999_999).contains(&certificate_sequence) {
            return Err(CertificateNumberError::CertificateSequenceOutOfRange);
        }

        let components =
            format!("{academic_year:04}{activity_sequence:04}{certificate_sequence:06}");
        let check_digit = luhn_check_digit(components.as_bytes());
        Ok(Self(format!(
            "{academic_year:04}-{activity_sequence:04}-{certificate_sequence:06}-{check_digit}"
        )))
    }

    pub fn parse(value: &str) -> Result<Self, CertificateNumberError> {
        let bytes = value.as_bytes();
        if bytes.len() != 18
            || bytes[4] != b'-'
            || bytes[9] != b'-'
            || bytes[16] != b'-'
            || bytes
                .iter()
                .enumerate()
                .any(|(index, byte)| !matches!(index, 4 | 9 | 16) && !byte.is_ascii_digit())
        {
            return Err(CertificateNumberError::InvalidFormat);
        }

        let academic_year = value[0..4]
            .parse::<i32>()
            .map_err(|_| CertificateNumberError::InvalidFormat)?;
        let activity_sequence = value[5..9]
            .parse::<u32>()
            .map_err(|_| CertificateNumberError::InvalidFormat)?;
        let certificate_sequence = value[10..16]
            .parse::<u32>()
            .map_err(|_| CertificateNumberError::InvalidFormat)?;
        let expected = Self::new(academic_year, activity_sequence, certificate_sequence)?;
        if expected.as_str() != value {
            return Err(CertificateNumberError::InvalidCheckDigit);
        }
        Ok(expected)
    }

    pub fn components(&self) -> (i32, u32, u32, u8) {
        // Construction and deserialization both validate this exact fixed-width shape.
        (
            self.0[0..4].parse().expect("validated certificate year"),
            self.0[5..9]
                .parse()
                .expect("validated certificate activity sequence"),
            self.0[10..16]
                .parse()
                .expect("validated certificate sequence"),
            self.0.as_bytes()[17] - b'0',
        )
    }
}

fn luhn_check_digit(digits: &[u8]) -> u8 {
    let sum = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(index, digit)| {
            let value = digit - b'0';
            if index % 2 == 0 {
                let doubled = value * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                value
            }
        })
        .sum::<u8>();
    (10 - sum % 10) % 10
}

#[cfg(test)]
mod tests {
    use crate::modules::certificates::models::CertificateNumber;

    #[test]
    fn formats_the_approved_number_and_validates_luhn() {
        let number = CertificateNumber::new(2569, 42, 123).unwrap();
        assert_eq!(number.as_str(), "2569-0042-000123-4");
        assert_eq!(CertificateNumber::parse(number.as_str()).unwrap(), number);
        assert!(CertificateNumber::parse("2569-0042-000123-5").is_err());
        assert!(CertificateNumber::new(2569, 10_000, 1).is_err());
        assert!(CertificateNumber::new(2569, 1, 1_000_000).is_err());
        assert_eq!(number.components(), (2569, 42, 123, 4));
        assert!(serde_json::from_str::<CertificateNumber>("\"2569-0042-000123-5\"").is_err());
        assert_eq!(
            serde_json::to_string(&number).unwrap(),
            "\"2569-0042-000123-4\""
        );
    }
}
