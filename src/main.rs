//! # Dedupy
//!
//! Reads Unified Transaction Reports and outputs tidy TSV imports for Compass.
//!
//! The output file is structured as follows (values surrounded with [] are
//! descriptive purposes only, not part of the output):
//!
//! * type [of transaction]
//! * sku
//! * description [of product]
//! * quantity [of units]
//! * total [flow of quantty * unit price]
//!
//! # Amazon Report Footguns
//!
//! Amazon item descriptions are not enforced to a standardized text encoding.
//! This app takes special care to handle non-utf8 text by performing a lossy
//! conversion when encountering invalid utf8 sequences. This is something
//! that no one cares about until you have to deal with it either by choice
//! or chance.
//!
//! # Logging Date/Times
//!
//! A boundary is created while parsing an individule report that is defined
//! by the oldest and newest date/time values. This data is saved in a report
//! log so on subsequent runs the parser can skip over records that have dates
//! that fall within the previously parsed boundary.
//!
//! # Backups
//!
//! As an extreme measure of caution, a machine-readable representation of the
//! **raw** unaggregated transactions is kept of each run. Thes backup files
//! can be used to sanity check output, automate testing, and a safety net
//! if things go wrong.
//!
//! Backups are stored in `.json` format but
#![allow(dead_code)]

use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// A reference to a transaction from the input Csv.
#[derive(Deserialize, Serialize, Debug)]
struct RefSale<'a> {
    #[serde(alias = "date/time")]
    date_time: &'a str,
    #[serde(alias = "type")]
    kind: &'a str,
    sku: &'a str,
    total: &'a str,
    quantity: &'a str,
    description: &'a str,
}

impl RefSale<'_> {
    fn total_cents(&self) -> Result<i64, eyre::Error> {
        // To avoid floating point errors we parse the total price into cents.
        // This is undone when writing the final output.
        Ok(self.total.replace(['.', ','], "").parse::<i64>()?)
    }

    fn quantity(&self) -> Result<i64, eyre::Error> {
        if !self.quantity.is_empty() {
            Ok(self.quantity.parse::<i64>()?)
        } else {
            Ok(0)
        }
    }

    fn unit_cents(&self) -> Result<i64, eyre::Error> {
        if self.quantity()? == 0 {
            Ok(self.total_cents()?)
        } else {
            Ok(self.total_cents()? / self.quantity()?)
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Sale {
    sku: String,
    #[serde(serialize_with = "cents_to_dollars")]
    unit_cents: i64,
    quantity: i64,
    description: String,
    kind: String,
}

fn cents_to_dollars<S>(cents: &i64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let v = (*cents as f64) / 100.0;
    let mut buffer = ryu::Buffer::new();
    serializer.serialize_str(buffer.format(v))
}

#[derive(Debug, Default)]
struct Bucket {
    /// buffer to aggregate sales before writing to disk
    sales: Vec<Sale>,
    /// The oldest date in the report.
    ///
    /// To be clear, this is assuming the report is sorted so that the
    /// last record is the oldest, and the first record is the newest.
    oldest: Option<String>,
    /// The flip side of `oldest`.
    newest: Option<String>,
    // Have we seen a record yet?
    seen: bool,
    original_path: PathBuf,
}

impl Bucket {
    fn write(self) -> eyre::Result<()> {
        let Bucket {
            mut sales,
            oldest,
            newest,
            original_path,
            ..
        } = self;
        let og_name = original_path
            .file_stem()
            .ok_or_else(|| eyre::eyre!("No file stem"))?
            .to_str()
            .ok_or_else(|| eyre::eyre!("Invalid Utf-8 path"))?;

        let newest = newest.unwrap();
        let oldest = oldest.unwrap();

        let new = format!("dedupy-{og_name}.tsv");
        println!("Writing to {new}");
        let mut writer = csv::WriterBuilder::new().delimiter(b'\t').from_path(new)?;

        sales.drain(..).for_each(|mut sale| {
            if sale.quantity != 0 {
                sale.unit_cents *= sale.quantity;
            }
            writer.serialize(sale).unwrap();
        });
        writer.flush()?;

        write_log(og_name, oldest, newest)?;
        Ok(())
    }

    fn set_oldest(&mut self, date: &str) {
        self.oldest = Some(date.to_string());
    }

    fn set_newest(&mut self, date: &str) {
        if !self.seen {
            self.newest = Some(date.to_string());
            self.seen = true;
        }
    }

    fn add(&mut self, sales_ref: &RefSale<'_>) -> eyre::Result<()> {
        self.set_oldest(sales_ref.date_time);

        let mut found = false;
        for sale in self.sales.iter_mut() {
            // check if sku description, unit_cents and kind matches
            // if they do then increase the quantity and return
            if sale.sku == sales_ref.sku
                && sale.kind == sales_ref.kind
                && sale.description == sales_ref.description
                && sale.unit_cents == sales_ref.unit_cents()?
            {
                sale.quantity += sales_ref.quantity()?;
                found = true;
                break;
            }
        }
        // If we didn't find a match then add a new sale.
        if !found {
            self.sales.push(Sale {
                sku: sales_ref.sku.to_string(),
                unit_cents: sales_ref.unit_cents()?,
                quantity: sales_ref.quantity()?,
                description: sales_ref.description.to_string(),
                kind: sales_ref.kind.to_string(),
            });
        }
        Ok(())
    }
}

fn write_log(og_name: &str, oldest: String, newest: String) -> Result<(), eyre::Error> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open("memory.csv")
        .unwrap();
    let mut log_writer = csv::WriterBuilder::new().delimiter(b'\t').from_writer(file);
    log_writer.write_record([og_name, &oldest, &newest])?;
    log_writer.flush()?;
    Ok(())
}

/// Parse a Transaction report and writes results to the current directory.
fn parse_report<P: AsRef<Path>>(path: P) -> eyre::Result<()> {
    let mut bucket = Bucket {
        sales: vec![],
        oldest: None,
        newest: None,
        seen: false,
        original_path: path.as_ref().to_path_buf(),
    };
    let read = std::fs::read(path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(read.as_slice());

    let mut header = None;
    let mut peekable = rdr.byte_records().skip(7).peekable();
    loop {
        if header.is_none() {
            match peekable.next() {
                Some(headers) => header = Some(headers?),
                None => eyre::bail!("No header found"),
            }
            continue;
        }
        if let Some(record) = peekable.next() {
            let record = record?;
            let sale = record.deserialize::<RefSale>(header.as_ref())?;
            bucket.add(&sale)?;
            bucket.set_newest(sale.date_time);
            if peekable.peek().is_none() {
                bucket.set_oldest(sale.date_time)
            }
        } else {
            // Sanity checks that the bucket is in a valid state.
            assert!(bucket.seen);
            assert!(bucket.oldest.is_some());
            assert!(bucket.newest.is_some());
            println!("Reached end of file and passed validation");
            bucket.write()?;
            break;
        }
    }
    Ok(())
}

fn main() -> eyre::Result<()> {
    let picks = rfd::FileDialog::new().pick_files();
    let Some(files) = picks else {
        println!("No files selected");
        return Ok(());
    };
    for file in files {
        println!("Parsing {:?}", file.display());
        parse_report(file)?;
    }
    Ok(())
}
