# Dedupy

Reads Unified Transaction Reports and outputs tidy tsv imports for Compass.

## Amazon Report Sanitization.

Amazon item descriptions are not guaranteed to be valid UTF8.
A lazy conversion is done if invalid UTF8 bytes are read from the report.
You probably don't need to worry about this. `U+FFFD` is used
as the replacement character (looks like this: �).

## Hashed Transactions.

As a report is being parsed a hash is generated of each transaction. Hashes
are stored in a file called `memory`, care should be made to avoid losing this
file. Hashes (likely) guarantee that a transaction can only ever be recorded a single
time (assuming the `memory` file is never lost). In theory, you could endlessly
rerun the same report, but the result will only be one "output" of aggregation.

The hashing function used can be referenced [here](https://docs.rs/seahash/latest/seahash/reference/index.html).
It is not a perfect hash function, but it is fast and is a cheap way to reduce
the chance of the same transaction being recorded twice.

## Usage.

Double-clicking on the app will open a file browser where downloaded transactions
reports can be selected for parsing.

Finished files will be place in the same folder of the application and
be named `OUTPUT_[time].tsv`.

**Scripting usage** is partially supported by accepting a file path on the first
positional argument.

```shell
dedupy transaction_report.csv
```

The application effectively functions as if the file was selected using the
file browser. Results are **not** sent to stdout (maybe in the future).

## Debugging / Tracing

Logging filters can be set using the standard `RUST_LOG` environment variable.

```shell
# This will show input and ouput logs for each row that is parsed.
export RUST_LOG=trace
dedupy transaction_report.csv
```

## TODO

- [ ] At least a few tests.
- [ ] Proper scripting support (ie, piping from stdin to stdout).
