use crate::TouRates;
use crate::usage_data::UsageEntry;
use bigdecimal::BigDecimal;
use jiff::civil::Time;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfUse {
    Off,
    Mid,
    Peak,
}

impl TimeOfUse {
    pub fn from_time(time: Time) -> Self {
        let hour = time.hour();
        match hour {
            0..=5 => TimeOfUse::Off,
            6..=16 | 21..=23 => TimeOfUse::Mid,
            17..=20 => TimeOfUse::Peak,
            ..0 | 24.. => panic!("Invalid hour: {}", hour),
        }
    }
}

pub fn calculate_tou_cost<'a>(
    rate: &TouRates,
    usage_data: impl Iterator<Item = &'a UsageEntry>,
) -> BigDecimal {
    usage_data
        .map(|entry| {
            let tou_start = TimeOfUse::from_time(entry.start_time);
            let tou_end = TimeOfUse::from_time(entry.end_time);
            assert_eq!(
                tou_start, tou_end,
                "Start and end times must be in the same TOU period"
            );
            match tou_start {
                TimeOfUse::Off => &rate.off * entry.kwh_total(),
                TimeOfUse::Mid => &rate.mid * entry.kwh_total(),
                TimeOfUse::Peak => &rate.peak * entry.kwh_total(),
            }
        })
        .sum()
}

pub fn calculate_base_cost<'a>(
    rate: &BigDecimal,
    usage_data: impl Iterator<Item = &'a UsageEntry>,
) -> BigDecimal {
    usage_data.map(|entry| rate * entry.kwh_total()).sum()
}
