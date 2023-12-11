#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    io::Write,
    path::Path,
};

use seahash::hash;
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// A set of hashes of transactions that have already been written to disk.
#[derive(Debug)]
struct Memory(HashSet<u64>);

impl Memory {
    fn has_hash(&self, hash: &u64) -> bool {
        self.0.contains(hash)
    }

    /// Returns a new [`Memory`] instance.
    #[instrument]
    fn new() -> eyre::Result<Self> {
        match csv::Reader::from_path("memory") {
            Ok(mut rdr) => {
                let mut records = HashSet::new();
                for record in rdr.records() {
                    let rcd = record?;
                    records.insert(rcd.deserialize(None)?);
                }
                Ok(Self(records))
            }
            Err(_) => Ok(Self(HashSet::new())),
        }
    }
}

/// A reference to a transaction from the input CSV.
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
    /// Returns the total cents of the transaction.
    #[instrument]
    fn total_cents(&self) -> Result<i64, eyre::Error> {
        // To avoid floating point errors we parse the total price into cents.
        // This is undone when writing the final output.
        Ok(self.total.replace(['.', ','], "").parse::<i64>()?)
    }

    /// Returns the quantity of the transaction.
    #[instrument]
    fn quantity(&self) -> Result<i64, eyre::Error> {
        if !self.quantity.is_empty() {
            Ok(self.quantity.parse::<i64>()?)
        } else {
            Ok(0)
        }
    }

    /// Returns the unit value of the transaction.
    ///
    /// This is the total cents divided by the quantity unless the quantity
    /// is zero, in which case the total cents is returned.
    #[instrument]
    fn unit_cents(&self) -> Result<i64, eyre::Error> {
        if self.quantity()? == 0 {
            Ok(self.total_cents()?)
        } else {
            Ok(self.total_cents()? / self.quantity()?)
        }
    }
}

/// An owned transaction.
#[derive(Debug, Deserialize, Serialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
struct Sale {
    sku: String,
    #[serde(serialize_with = "cents_to_dollars")]
    unit_cents: i64,
    quantity: i64,
    description: String,
    kind: String,
}

/// Helper function to serialize cents to dollars.
fn cents_to_dollars<S>(cents: &i64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // TODO: Would this ever panic?;
    let v = (*cents as f64) / 100.0;
    let mut buffer = ryu::Buffer::new();
    serializer.serialize_str(buffer.format(v))
}

/// A bucket to aggregate sales before writing to disk.
///
/// Additionally, holds hashes of transactions from the current run.
#[derive(Debug, Default)]
struct Bucket {
    // buffer to aggregate sales before writing to disk
    sales: HashMap<Sale, i64>,
    hashes: Vec<u64>,
}

impl Bucket {
    /// Returns a new [`Bucket`] instance.
    #[instrument]
    fn new() -> Self {
        Self {
            sales: HashMap::new(),
            hashes: Vec::new(),
        }
    }

    /// Flushes the bucket to disk.
    #[instrument]
    fn flush(self) -> eyre::Result<()> {
        let Bucket {
            mut sales, hashes, ..
        } = self;

        let time = chrono::Local::now().to_rfc3339().replace(":", "_");
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_path(format!("OUTPUT-{time}.tsv"))?;

        // Can write all sales with a sku immediately.
        // Make a second pass to write sales without a sku after further
        // consolidation.
        let mut without_sku = HashMap::<Sale, i64>::new();

        // Drain the hashmap of sales with skus into the writer.
        // Branching on empty skus into a second pass.
        for (mut sale, quantity) in sales.drain() {
            if sale.sku.is_empty() {
                // Set the quantity to 0 so it can be used as a key.
                // Extract the cents and add them to the hashmap's value.
                let scents = sale.unit_cents;
                sale.unit_cents = 0;
                match without_sku.get_mut(&sale) {
                    Some(cents) => *cents += scents,
                    None => {
                        without_sku.insert(sale, scents);
                    }
                }
            } else {
                sale.quantity = quantity;
                writer.serialize(sale)?;
            }
        }

        // Handle writing the second pass of transactions without a sku.
        for (mut sale, cents) in without_sku.drain() {
            if sale.unit_cents.is_negative() {
                sale.quantity = -1
            } else {
                sale.quantity = 1;
            }
            // Per request.
            sale.sku = "FBATF".to_string();
            sale.unit_cents = cents;
            writer.serialize(sale)?;
        }

        writer.flush()?;

        // Only after producing aggregated output do we write the hashes.
        let mut write = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open("memory")?;
        // TODO: is this buffered? Does it need to be?
        // Doesnt this technically write the hash as a string?
        // I think I want to write the bytes but how would a newline be
        // written?
        for hash in hashes {
            writeln!(write, "{}", hash)?;
        }
        Ok(())
    }

    /// Adds a transaction to the bucket.
    #[instrument]
    fn add(&mut self, sales_ref: &RefSale<'_>, hash: u64) -> eyre::Result<()> {
        let qt_value = sales_ref.quantity()?;
        self.hashes.push(hash);
        let sale = Sale {
            sku: sales_ref.sku.to_string(),
            unit_cents: sales_ref.unit_cents()?,
            // This is a placeholder value that will be overwritten
            // before writing to disk. The value is being summed as the
            // hashmap's value.
            quantity: 0,
            description: sales_ref.description.to_string(),
            kind: sales_ref.kind.to_string(),
        };
        // If the sale is already in the hashmap, add the quantity.
        // Otherwise insert the sale into the hashmap.
        match self.sales.get_mut(&sale) {
            Some(quantity) => *quantity += qt_value,
            None => {
                self.sales.insert(sale, qt_value);
            }
        };
        Ok(())
    }
}

/// Entry point for the library.
pub struct Report;

impl Report {
    /// Parse the report at the given path and write output to disk.
    pub fn parse<P>(path: P) -> eyre::Result<()>
    where
        P: AsRef<Path> + std::fmt::Debug,
    {
        let mut bucket = Bucket::new();
        let memory = Memory::new()?;

        let read_file = std::fs::read(&path)?;

        // Cannot guarantee the file is utf8, if anything we know it's not.
        let read = String::from_utf8_lossy(&read_file);

        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(read.as_bytes());

        // The first 7 records of the report are trash.
        let mut records = rdr.records().skip(7);

        // Yank out the headers for deserialization.
        let header = records.next().transpose()?;

        for record in records {
            let record = record?;
            let hash = hash(record.as_slice().as_bytes());
            if !memory.has_hash(&hash) {
                let sale = record.deserialize(header.as_ref())?;
                // Do not log the current hash to disk since we could crash
                // prior to actually producing output of the transaction.
                bucket.add(&sale, hash)?;
            }
        }

        bucket.flush()?;
        Ok(())
    }
}
