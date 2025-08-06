tool-that-might-be-used-when-you-need-to-compare-scl-tou-rates
==============================================================

Also known as "TTMBUWYNTCSTR".

This tool is designed to help you compare your current usage rates with the new SCL TOU rates.
Usage is in the `--help`, e.g. `cargo run -- --help`.

# Known issues

- The amount of KWH in the SCL CSV file is different from the amount of KWH in the bill from the same period.
  I assume this is because the bill period is an hour or so off from the exact start and end of the day.
  This does mean that the cost reported by the tool will not match the cost on the bill.
