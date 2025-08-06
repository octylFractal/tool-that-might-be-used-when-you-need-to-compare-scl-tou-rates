mod rate_calculator;
mod usage_data;

use crate::rate_calculator::calculate_base_cost;
use crate::usage_data::UsageEntry;
use bigdecimal::BigDecimal;
use clap::{Args, Parser, ValueEnum};
use csv::StringRecord;
use jiff::civil::Time;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;

/// Tool that might be used when you need to compare SCL TOU rates.
/// Given your SCL usage data with its static KWH rate, and your TOU rates, calculates
/// the total cost of your usage if you switched to the TOU rates.
#[derive(Parser, Debug)]
#[command(version, long_about)]
struct Ttmbuwyntcstr {
    /// CSV file with fine-grained data, exported using the "Green Button" in SCL.
    /// It's under "View Usage" > "View Usage Details".
    #[arg(long_help)]
    usage_csv: PathBuf,
    /// Your current static KWH rate, in dollars per KWH.
    /// This can be found in your SCL bill.
    #[arg(long_help)]
    current_rate: BigDecimal,
    #[command(flatten)]
    tou_rates: TouRateInfo,
}

#[derive(Args, Debug)]
#[group(required = true)]
struct TouRateInfo {
    /// Your location, used to determine the TOU rates.
    /// You must specify this or the individual TOU rates.
    /// See https://www.seattle.gov/city-light/residential-services/billing-information/time-of-use.
    #[arg(short = 'l', long, value_enum, conflicts_with = "tou_rates")]
    tou_location: Option<TouLocation>,
    #[arg(short, long, group = "tou_rates", long_help = tou_rate_help("off-peak"))]
    off_peak_rate: Option<BigDecimal>,
    #[arg(short, long, group = "tou_rates", long_help = tou_rate_help("mid-peak"))]
    mid_peak_rate: Option<BigDecimal>,
    #[arg(short, long, group = "tou_rates", long_help = tou_rate_help("peak"))]
    peak_rate: Option<BigDecimal>,
}

fn tou_rate_help(peak: &str) -> String {
    format!(
        "Your {} TOU rates, in dollars per KWH. \
         Typically you can just give your location with `--tou-location` and the program will use \
         its built-in rates. However, if SCL has changed their rates, you need to specify them \
         manually.",
        peak
    )
}

#[derive(ValueEnum, Copy, Clone, PartialEq, Eq, Debug)]
enum TouLocation {
    Seattle,
    LakeForestPark,
    NormandyPark,
    Tukwila,
    Renton,
    /// Short for "Burien, SeaTac, Shoreline, Uninc. King County".
    Other,
}

#[derive(Debug, Clone)]
struct TouRates {
    pub off: BigDecimal,
    pub mid: BigDecimal,
    pub peak: BigDecimal,
}

impl TouRates {
    fn from_args(args: &Ttmbuwyntcstr) -> Self {
        if let Some(location) = args.tou_rates.tou_location {
            match location {
                TouLocation::Seattle => Self {
                    off: BigDecimal::from_str("0.0828").unwrap(),
                    mid: BigDecimal::from_str("0.1449").unwrap(),
                    peak: BigDecimal::from_str("0.1656").unwrap(),
                },
                TouLocation::LakeForestPark => Self {
                    off: BigDecimal::from_str("0.0895").unwrap(),
                    mid: BigDecimal::from_str("0.1565").unwrap(),
                    peak: BigDecimal::from_str("0.1789").unwrap(),
                },
                TouLocation::NormandyPark => Self {
                    off: BigDecimal::from_str("0.0881").unwrap(),
                    mid: BigDecimal::from_str("0.1541").unwrap(),
                    peak: BigDecimal::from_str("0.1762").unwrap(),
                },
                TouLocation::Tukwila => Self {
                    off: BigDecimal::from_str("0.0886").unwrap(),
                    mid: BigDecimal::from_str("0.1551").unwrap(),
                    peak: BigDecimal::from_str("0.1773").unwrap(),
                },
                TouLocation::Renton => Self {
                    off: BigDecimal::from_str("0.0828").unwrap(),
                    mid: BigDecimal::from_str("0.1449").unwrap(),
                    peak: BigDecimal::from_str("0.1656").unwrap(),
                },
                TouLocation::Other => Self {
                    off: BigDecimal::from_str("0.0894").unwrap(),
                    mid: BigDecimal::from_str("0.1565").unwrap(),
                    peak: BigDecimal::from_str("0.1788").unwrap(),
                },
            }
        } else {
            Self {
                off: args
                    .tou_rates
                    .off_peak_rate
                    .clone()
                    .expect("off-peak rate is required"),
                mid: args
                    .tou_rates
                    .mid_peak_rate
                    .clone()
                    .expect("mid-peak rate is required"),
                peak: args
                    .tou_rates
                    .peak_rate
                    .clone()
                    .expect("peak rate is required"),
            }
        }
    }
}

fn main() {
    let args = Ttmbuwyntcstr::parse();

    let tou_rates = TouRates::from_args(&args);
    let usage_data = read_usage_data(&args.usage_csv);
    eprintln!("Found {} usage entries", usage_data.len());
    let total_kwh: BigDecimal = usage_data.iter().map(|entry| entry.kwh_total()).sum();
    eprintln!("Total KWH used: {:.2}", total_kwh);
    let current_cost = calculate_base_cost(&args.current_rate, usage_data.iter());
    eprintln!("Current cost: ${:.2}", current_cost);
    let tou_cost = rate_calculator::calculate_tou_cost(&tou_rates, usage_data.iter());
    eprintln!("TOU cost: ${:.2}", tou_cost);
    if tou_cost < current_cost {
        eprintln!(
            "You would save ${:.2} by switching to TOU rates!",
            current_cost - tou_cost
        );
    } else if tou_cost > current_cost {
        eprintln!(
            "You would pay ${:.2} more by switching to TOU rates!",
            tou_cost - current_cost
        );
    } else {
        eprintln!("You would pay the same amount with TOU rates. Try another bill?");
    }
}

static EXPECTED_HEADERS: LazyLock<StringRecord> = LazyLock::new(|| {
    StringRecord::from(vec![
        "TYPE",
        "DATE",
        "START TIME",
        "END TIME",
        "IMPORT (kWh)",
        "EXPORT (kWh)",
        "NOTES",
    ])
});

fn read_usage_data(usage_csv: &Path) -> Vec<UsageEntry> {
    // Annoyingly, the usage CSV comes with extra rows at the start that don't mean anything,
    // so we need to skip them.
    let mut reader = BufReader::new(File::open(usage_csv).expect("Usage file not found"));
    let mut line_buf = String::new();
    loop {
        line_buf.clear();
        if reader
            .read_line(&mut line_buf)
            .expect("Failed to read line")
            == 0
        {
            panic!("Usage file is empty or malformed");
        }
        if line_buf.starts_with("TYPE,DATE,") {
            break; // Found the header row, stop reading
        }
    }
    let reader_with_headers = Cursor::new(line_buf).chain(reader);
    let mut csv_reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(reader_with_headers);
    let headers = csv_reader.headers().expect("CSV headers not found").clone();
    if headers != *EXPECTED_HEADERS {
        panic!(
            "Unexpected headers in usage CSV: {:?}. Expected: {:?}",
            headers, *EXPECTED_HEADERS
        );
    }
    csv_reader
        .into_records()
        .filter_map(|r| {
            let record = r.expect("Usage file could not be deserialized");
            (record[0] == *"Electric usage").then(|| UsageEntry {
                start_time: Time::from_str(&record[2]).expect("Invalid start time format"),
                end_time: Time::from_str(&record[3]).expect("Invalid end time format"),
                imported: record[4].parse().expect("Invalid imported kWh value"),
                exported: record[5].parse().expect("Invalid exported kWh value"),
            })
        })
        .collect()
}
