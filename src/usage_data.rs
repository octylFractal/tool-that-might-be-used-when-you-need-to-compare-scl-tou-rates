use bigdecimal::BigDecimal;
use jiff::civil::Time;

#[derive(Debug)]
pub struct UsageEntry {
    pub start_time: Time,
    pub end_time: Time,
    pub imported: BigDecimal,
    pub exported: BigDecimal,
}

impl UsageEntry {
    pub fn kwh_total(&self) -> BigDecimal {
        &self.imported - &self.exported
    }
}
