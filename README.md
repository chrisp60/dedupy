# Dedupy

Reads Unified Transaction Reports and outputs tidy tsv imports for Compass.

## Report Sanitization

Amazon item descriptions are not guaranteed to be valid UTF8.
A lazy conversion is done if invalid UTF8 bytes are read from the report.
You probably don't need to worry about this. `U+FFFD` is used
as the replacement character (looks like this: �).

## Hashed Transactions

A hash is generated of each transaction new transaction that the application
encounters. Hashes are stored in a file called `memory`, care should be made
to avoid losing this file. Hashes (likely, but not entirely) guarantee that a
transaction can only ever be recorded (ie: aggregated) a single time (assuming
the `memory` file is never lost). **This includes memorizing seen transactions
previous runs**.

In theory, you could endlessly rerun the same report, but the result of
all transactions will only be aggregated once. This is a convenience for
not needing to track the "last transaction" manually.

The hashing function used can be referenced [here](https://docs.rs/seahash/latest/seahash/reference/index.html).

If the need arises to make the application "forget" the transactions it has
seen, you can delete the `memory` file and a fresh one will be created on the
next run.

## Basic Usage

Double-clicking on the app will open a file browser where transactions
reports can be selected for parsing. The file dialog will be filtered to only
show files ending in `.csv` extensions.

Finished files will be placed in the same folder of the application and
be named `OUTPUT_[time].tsv`. Time is roughly encoded to be a valid windows path.

## Scripting Usage / CLI

Scripting is partially supported by accepting a file path on the first
positional argument.

```shell
dedupy transaction_report.csv
```

The application effectively functions as if the file was selected using the
file browser. Results are **not** sent to stdout (maybe in the future).

## Debugging / Tracing

Logging filters can be set using the standard `RUST_LOG` environment variable.
**Every** function is instrumented with tracing, additional logs are provided
on logic-heavy paths.

```shell
# This will show input and ouput logs for each row that is parsed.
export RUST_LOG=dedupy=trace
# If on powershell: $RUST_LOG="dedupy=trace"
dedupy transaction_report.csv
```

## TODO

- [ ] At least a few tests.
- [ ] Proper scripting support (ie, piping from stdin to stdout).

## Benchmark

A very informal benchmark on 11 months worth of transactions.

**Input Transactions:** 232813
**Resulting Aggregation:** 22877
**Time to Complete:** 12.7637 milliseconds
