//! Excel (.xls, .xlsx, .xlsm, .xlsb) worksheet to CSV conversion.
//!
//! Export cell values, not Excel display formatting or formula text. The first
//! populated worksheet row becomes the CSV header for downstream phrs commands.
use calamine::{Data, ExcelDateTime, Reader, open_workbook_auto};

use crate::error::PhError;

pub fn run(path: &str, sheet: Option<&str>) -> Result<Vec<u8>, PhError> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|e| PhError::new(format!("Cannot open Excel workbook {path:?}: {e}")))?;
    let names = workbook.sheet_names();
    let name = match sheet {
        Some(requested) => {
            if !names.iter().any(|name| name == requested) {
                return Err(PhError::new(format!(
                    "Worksheet {requested:?} not found in {path:?}. Available worksheets: {}.",
                    names.join(", ")
                )));
            }
            requested
        }
        None => names
            .first()
            .map(String::as_str)
            .ok_or_else(|| PhError::new(format!("Excel workbook {path:?} has no worksheets.")))?,
    };

    let range = workbook
        .worksheet_range(name)
        .map_err(|e| PhError::new(format!("Cannot read worksheet {name:?}: {e}")))?;
    range_to_csv(&range)
}

fn range_to_csv(range: &calamine::Range<Data>) -> Result<Vec<u8>, PhError> {
    if range.is_empty() {
        return Err(PhError::new("Selected Excel worksheet is empty."));
    }

    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());

    for row in range.rows() {
        // Values are serialized directly: no Polars schema inference, no loss of
        // distinct Excel text/number types before they become CSV fields.
        let fields: Vec<String> = row.iter().map(cell_to_string).collect();
        writer
            .write_record(fields.iter().map(String::as_bytes))
            .map_err(|e| PhError::new(format!("Cannot write Excel CSV: {e}")))?;
    }
    writer
        .into_inner()
        .map_err(|e| PhError::new(format!("Cannot finish Excel CSV: {e}")))
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) => value.to_string(),
        Data::String(value) => value.clone(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => datetime_to_string(value),
        Data::DateTimeIso(value) | Data::DurationIso(value) => value.clone(),
        Data::Error(value) => value.to_string(),
    }
}

fn datetime_to_string(value: &ExcelDateTime) -> String {
    if value.is_duration() {
        // Excel elapsed-time cells such as [h]:mm:ss can exceed 24 hours.
        if let Some(duration) = value.as_duration() {
            let millis = duration.num_milliseconds() as i128;
            let negative = millis < 0;
            let abs = millis.abs();
            let hours = abs / 3_600_000;
            let minutes = (abs / 60_000) % 60;
            let seconds = (abs / 1_000) % 60;
            let fraction = abs % 1_000;
            let sign = if negative { "-" } else { "" };
            if fraction == 0 {
                return format!("{sign}{hours:02}:{minutes:02}:{seconds:02}");
            }
            return format!("{sign}{hours:02}:{minutes:02}:{seconds:02}.{fraction:03}");
        }
    }
    if let Some(datetime) = value.as_datetime() {
        let date = datetime.date();
        let time = datetime.time();
        // Dates with no time component are emitted as YYYY-MM-DD. Values in
        // the interval [0, 1) are Excel times-of-day rather than dates.
        if value.as_f64() >= 0.0 && value.as_f64() < 1.0 {
            return time.to_string();
        }
        if time == chrono::NaiveTime::default() {
            return date.to_string();
        }
        return format!("{date}T{time}");
    }
    // For Excel's invalid leap day 1900-02-29 or out-of-range serials,
    // preserve the underlying number instead of emitting a fabricated date.
    value.as_f64().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use calamine::{ExcelDateTimeType, Range};

    #[test]
    fn escaping_blanks_and_types() {
        let mut range = Range::<Data>::new((0, 0), (2, 3));
        range.set_value((0, 0), Data::String("name".into()));
        range.set_value((0, 1), Data::String("description".into()));
        range.set_value((0, 2), Data::String("quantity".into()));
        range.set_value((0, 3), Data::String("ok".into()));
        range.set_value((1, 0), Data::String("Åse".into()));
        range.set_value((1, 1), Data::String("Hello, \"world\"\nagain".into()));
        range.set_value((1, 2), Data::Int(3));
        range.set_value((1, 3), Data::Bool(true));
        range.set_value((2, 0), Data::String("Bob".into()));
        range.set_value((2, 2), Data::Float(1.25));
        range.set_value((2, 3), Data::Bool(false));
        assert_eq!(
            range_to_csv(&range).unwrap(),
            b"name,description,quantity,ok\n\xC3\x85se,\"Hello, \"\"world\"\"\nagain\",3,true\nBob,,1.25,false\n"
        );
    }

    #[test]
    fn empty_sheet_is_an_error() {
        assert!(range_to_csv(&calamine::Range::<Data>::empty()).is_err());
    }

    #[test]
    fn date_and_datetime() {
        let date = ExcelDateTime::new(45293.0, ExcelDateTimeType::DateTime, false);
        assert_eq!(datetime_to_string(&date), "2024-01-02");
        let timestamp = ExcelDateTime::new(45293.5, ExcelDateTimeType::DateTime, false);
        assert_eq!(datetime_to_string(&timestamp), "2024-01-02T12:00:00");
    }
}
