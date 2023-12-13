#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use seahash::hash;
use serde::{Deserialize, Serialize};

/// A set of hashes of transactions that have already been written to disk.
#[derive(Debug)]
struct Memory {
    set: HashSet<u64>,
    path: &'static str,
}

impl Memory {
    fn memorize<S>(&mut self, s: S) -> bool
    where
        S: AsRef<str>,
    {
        let hash = hash(s.as_ref().as_bytes());
        self.set.insert(hash)
    }

    /// Returns a new [`Memory`] instance.
    fn new(path: &'static str) -> eyre::Result<Self> {
        let set = csv::Reader::from_path(path)
            .and_then(|mut val| {
                val.deserialize::<u64>()
                    .collect::<Result<HashSet<u64>, _>>()
            })
            .unwrap_or_default();
        Ok(Self { path, set })
    }

    fn write(self) -> eyre::Result<()> {
        let mut wtr = csv::Writer::from_path(self.path)?;
        for v in &self.set {
            wtr.serialize(v)?;
        }
        wtr.flush()?;
        Ok(())
    }
}

/// A reference to a transaction from the input CSV.
#[derive(Deserialize, Serialize, Debug)]
struct RefSale<'a> {
    #[serde(alias = "type")]
    kind: String,
    sku: Option<String>,
    total: &'a str,
    #[serde(default)]
    quantity: i64,
    description: String,
}

/// An owned transaction.
// These fields are in the order that they were specified in the original
// email. I do not know if they are read by index or by header. I guess
// this is the safest way to do it.
#[derive(Debug, Deserialize, Serialize, Hash, Eq, PartialEq, PartialOrd, Ord, Default)]
struct Sale {
    #[serde(rename = "Type")]
    kind: String,
    #[serde(rename = "SKU")]
    sku: String,
    #[serde(rename = "Description")]
    description: String,
    #[serde(rename = "Quantity", default)]
    quantity: i64,
    // Originally canoverted all dollars to cents, so now we reverse
    #[serde(serialize_with = "to_dollars", rename = "Total")]
    cents: i64,
}

impl Sale {
    fn new(t: Trx, i: i64) -> Self {
        match t {
            Trx::Adjustment(a) => Self {
                kind: a.kind,
                sku: "FBATF".to_string(),
                description: a.description,
                quantity: if i < 0 { -1 } else { 1 },
                cents: i,
            },
            Trx::WithSku(WithSku {
                kind,
                sku,
                cents,
                description,
            }) => Self {
                kind,
                sku,
                description,
                quantity: i,
                cents: cents * i,
            },
        }
    }
}

/// Helper function to serialize cents to dollars.
fn to_dollars<S>(cents: &i64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // TODO: Would this ever panic?;
    let v = (*cents as f64) / 100.0;
    let mut buffer = ryu::Buffer::new();
    serializer.serialize_str(buffer.format(v))
}

/// Entry point for the library.
pub struct Report;

impl Report {
    /// Parse the report at the given path and write output to disk.
    pub fn parse<P>(path: P) -> eyre::Result<()>
    where
        P: AsRef<Path> + std::fmt::Debug,
    {
        let read_file = std::fs::read(&path)?;

        // Cannot guarantee the file is utf8, if anything we know it's not.
        let read = String::from_utf8_lossy(&read_file);

        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(read.as_bytes());

        // The first 7 records of the report are trash.
        let mut iter = rdr.records().skip(7);

        let mut adjustmut_map = HashMap::<Adjustment, Occurences>::new();
        let mut with_sku_map = HashMap::<WithSku, Cents>::new();

        let mut recmem = Memory::new("memory")?;
        let mut skumem = Memory::new("sku_memory")?;

        let hdr = iter.next().transpose()?;
        for record in iter {
            let r = &record?;
            if !recmem.memorize(r.as_slice()) {
                let sale = r.deserialize::<RefSale>(hdr.as_ref())?;
                let qt = sale.quantity;
                match Trx::try_from(sale)? {
                    Trx::Adjustment(a) => adjustmut_map
                        .entry(a)
                        .and_modify(|v| *v += qt)
                        .or_insert(qt),
                    Trx::WithSku(s) => {
                        skumem.memorize(&s.sku);
                        with_sku_map.entry(s).and_modify(|v| *v += qt).or_insert(qt)
                    }
                };
            }
        }

        recmem.write()?;
        skumem.write()?;

        let mut buffer = adjustmut_map
            .into_iter()
            .map(|(k, v)| Sale::new(Trx::Adjustment(k), v))
            .collect::<Vec<_>>();
        buffer.extend(
            with_sku_map
                .into_iter()
                .map(|(k, v)| Sale::new(Trx::WithSku(k), v)),
        );

        buffer.sort_unstable_by_key(|s| (s.kind.clone(), s.description.clone()));
        eprintln!("{buffer:#?}");
        let mut wb = rust_xlsxwriter::Workbook::new();
        let worksheet = wb.add_worksheet();
        worksheet.serialize_headers(0, 0, &Sale::default())?;
        for sale in buffer {
            worksheet.serialize(&sale)?;
        }

        wb.save("output.xlsx")?;
        Ok(())
    }
}

enum Trx {
    Adjustment(Adjustment),
    WithSku(WithSku),
}

impl TryFrom<RefSale<'_>> for Trx {
    type Error = eyre::Error;
    fn try_from(value: RefSale<'_>) -> Result<Self, Self::Error> {
        match value.sku {
            Some(_) => WithSku::try_from(value).map(Trx::WithSku),
            None => Adjustment::try_from(value).map(Trx::Adjustment),
        }
    }
}

type Cents = i64;
type Occurences = i64;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
struct Adjustment {
    kind: String,
    description: String,
}

impl TryFrom<RefSale<'_>> for Adjustment {
    type Error = eyre::Error;
    fn try_from(value: RefSale<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: value.kind.to_string(),
            description: value.description.to_string(),
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
struct WithSku {
    kind: String,
    sku: String,
    cents: Cents,
    description: String,
}

impl TryFrom<RefSale<'_>> for WithSku {
    type Error = eyre::Error;
    fn try_from(value: RefSale<'_>) -> Result<Self, Self::Error> {
        let RefSale {
            kind,
            sku,
            total,
            quantity,
            description,
        } = value;

        let total = total.replace(['.', ','], "").parse::<i64>()?;
        // Div by 0 is None => cents
        let cents = total.checked_div(quantity).unwrap_or(total);

        Ok(Self {
            kind,
            sku: sku.expect("sku is some"),
            cents,
            description,
        })
    }
}
