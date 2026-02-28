use chrono::{Datelike, NaiveDate};

pub struct DndData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonthKey {
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

impl MonthKey {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "jan" | "january" => Ok(Self::January),
            "feb" | "february" => Ok(Self::February),
            "mar" | "march" => Ok(Self::March),
            "apr" | "april" => Ok(Self::April),
            "may" => Ok(Self::May),
            "jun" | "june" => Ok(Self::June),
            "jul" | "july" => Ok(Self::July),
            "aug" | "august" => Ok(Self::August),
            "sep" | "sept" | "september" => Ok(Self::September),
            "oct" | "october" => Ok(Self::October),
            "nov" | "november" => Ok(Self::November),
            "dec" | "december" => Ok(Self::December),
            _ => Err("invalid month name".to_string()),
        }
    }

    pub fn number(self) -> u32 {
        match self {
            Self::January => 1,
            Self::February => 2,
            Self::March => 3,
            Self::April => 4,
            Self::May => 5,
            Self::June => 6,
            Self::July => 7,
            Self::August => 8,
            Self::September => 9,
            Self::October => 10,
            Self::November => 11,
            Self::December => 12,
        }
    }
}

pub fn is_read_only(month: MonthKey, now: NaiveDate) -> bool {
    month.number() < now.month()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{is_read_only, MonthKey};

    #[test]
    fn month_parses_case_insensitive() {
        assert_eq!(MonthKey::parse("January").unwrap(), MonthKey::January);
        assert_eq!(MonthKey::parse("sep").unwrap(), MonthKey::September);
    }

    #[test]
    fn month_rejects_invalid_value() {
        assert!(MonthKey::parse("foo").is_err());
    }

    #[test]
    fn month_readonly_cutoff_works() {
        let now = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        assert!(is_read_only(MonthKey::March, now));
        assert!(!is_read_only(MonthKey::April, now));
        assert!(!is_read_only(MonthKey::December, now));
    }
}
