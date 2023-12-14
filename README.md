# Dedupy

- Reads a transaction report
- Aggregates similar transactions and adjustments.
- Remembers unique transactions to avoid "double-dipping" into same lime item.
- Remembers unique SKUs and notifies when a new item exists in the aggregation.

## Usage

1. Download a transaction report from Amazon.
1. Double-click the application's icon, this will open a file browser on your
   computer. The file browser is filtered to only show `.csv` files.
1. Navigate to the downloaded transaction report using the file browser.
1. Select or double-click the report.
1. The application will process the report, skipping transactions that
   have already been aggregated from a previous run.
1. Once finished, the application will generate the following files.
   1. `AGGREGATED_[TIMESTAMP].xlsx`: Aggregation of the selected report.
   1. `NEW_SKU_FOUND_[TIMESTAMP].txt`: **Generated only if an unrecognized SKU was
      encountered**.
   1. `memory`: Encoded record of unique _transactions_ from this report, and
      all previous reports.
   1. `sku_memory`: Encoded record of unique _SKUs_ from this report, and
      all previous reports.
1. Take care to not delete the generated files with `memory` in the name.
1. The application can be forced to _forget_ previously seen items by deleting
   the memory file. These files will be replaced on the next run without
   records of any runs before that.

## Text Encoding

Text that is invalid UTF-8 is replaced with `U+FFFD` which looks like: �.

## Development

A path can be given in the first positional argument when driving the application
through the command line.

```shell
dedupy DownloadedTransactions.csv
```
