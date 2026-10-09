use crate::{cli::Invocation, error::PhError, io};
use polars::prelude::*;

#[derive(Debug, Clone, Copy)]
struct SliceSpec {
    start: Option<i128>,
    stop: Option<i128>,
    step: i128,
}

impl SliceSpec {
    fn parse(text: &str) -> Result<Self, PhError> {
        let parts: Vec<_> = text.split(':').collect();
        if !(2..=3).contains(&parts.len()) {
            return Err(PhError::new(format!(
                "Invalid slice {text:?}: expected start:end[:step]."
            )));
        }

        fn bound(part: &str) -> Result<Option<i128>, PhError> {
            let part = part.trim();
            if part.is_empty() {
                return Ok(None);
            }
            // Python's integers are unbounded. Large slice indices can simply be
            // clamped, so saturate values outside i128's range instead of failing.
            match part.parse::<i128>() {
                Ok(value) => Ok(Some(value)),
                Err(_) => {
                    let digits = part
                        .strip_prefix('+')
                        .or_else(|| part.strip_prefix('-'))
                        .unwrap_or(part);
                    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
                        Err(PhError::new(format!("Invalid slice index: {part}.")))
                    } else if part.starts_with('-') {
                        Ok(Some(i128::MIN))
                    } else {
                        Ok(Some(i128::MAX))
                    }
                }
            }
        }

        let start = bound(parts[0])?;
        let stop = bound(parts[1])?;
        let step = if parts.len() == 3 {
            bound(parts[2])?.unwrap_or(1)
        } else {
            1
        };
        if step == 0 {
            return Err(PhError::new("Slice step cannot be zero."));
        }
        Ok(Self { start, stop, step })
    }

    /// Equivalent to Python's `slice(start, stop, step).indices(length)`.
    fn normalize(self, length: usize) -> (i128, i128, i128) {
        let n = length as i128;
        let adjust = |index: i128, low: i128, high: i128| {
            let index = if index < 0 {
                index.saturating_add(n)
            } else {
                index
            };
            index.clamp(low, high)
        };

        if self.step > 0 {
            (
                self.start.map_or(0, |v| adjust(v, 0, n)),
                self.stop.map_or(n, |v| adjust(v, 0, n)),
                self.step,
            )
        } else {
            (
                self.start.map_or(n - 1, |v| adjust(v, -1, n - 1)),
                // An omitted stop is the sentinel -1, *not* the same as
                // explicitly specifying -1 (which means the final row).
                self.stop.map_or(-1, |v| adjust(v, -1, n - 1)),
                self.step,
            )
        }
    }
}

pub fn run(df: DataFrame, inv: &Invocation) -> Result<Vec<u8>, PhError> {
    let spec = SliceSpec::parse(&inv.args[0])?;
    let (start, stop, step) = spec.normalize(df.height());

    let sliced = if step == 1 {
        // Contiguous slices are zero-copy and avoid building a row-index array.
        let offset = i64::try_from(start)
            .map_err(|_| PhError::new("Slice start exceeds supported row range."))?;
        let length = usize::try_from((stop - start).max(0))
            .map_err(|_| PhError::new("Slice length exceeds supported row range."))?;
        df.slice(offset, length)
    } else {
        let mut indices: Vec<IdxSize> = Vec::new();
        let mut index = start;
        while (step > 0 && index < stop) || (step < 0 && index > stop) {
            indices.push(
                IdxSize::try_from(index)
                    .map_err(|_| PhError::new("Slice index exceeds supported row range."))?,
            );
            // Saturation also covers arbitrarily large steps safely.
            index = index.saturating_add(step);
        }
        df.take(&IdxCa::new("index".into(), indices))?
    };
    io::write_csv(sliced)
}

#[cfg(test)]
mod tests {
    use super::SliceSpec;

    fn selected(slice: &str, len: usize) -> Vec<i128> {
        let (mut index, stop, step) = SliceSpec::parse(slice).unwrap().normalize(len);
        let mut result = Vec::new();
        while (step > 0 && index < stop) || (step < 0 && index > stop) {
            result.push(index);
            index = index.saturating_add(step);
        }
        result
    }

    #[test]
    fn python_slice_indices() {
        assert_eq!(selected("1:9:2", 6), vec![1, 3, 5]);
        assert_eq!(selected("::-1", 6), vec![5, 4, 3, 2, 1, 0]);
        assert_eq!(selected(":3", 6), vec![0, 1, 2]);
        assert_eq!(selected("3:", 6), vec![3, 4, 5]);
        assert_eq!(selected("-3:", 6), vec![3, 4, 5]);
        assert_eq!(selected(":-2", 6), vec![0, 1, 2, 3]);
        assert_eq!(selected("5:0:-2", 6), vec![5, 3, 1]);
        assert_eq!(selected("-1::-2", 6), vec![5, 3, 1]);
        assert_eq!(selected("::-2", 6), vec![5, 3, 1]);
        assert_eq!(selected("::-1", 0), Vec::<i128>::new());
        assert_eq!(selected("2:2", 6), Vec::<i128>::new());
        assert_eq!(selected("::", 6), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(selected("3:0:-1", 6), vec![3, 2, 1]);
        assert_eq!(selected("3:-1:-1", 6), Vec::<i128>::new());
        assert_eq!(selected("100:-100:-1", 6), vec![5, 4, 3, 2, 1, 0]);
        assert_eq!(selected("-100:100:2", 6), vec![0, 2, 4]);
        assert_eq!(
            selected("::999999999999999999999999999999999999999999", 6),
            vec![0]
        );
    }

    #[test]
    fn invalid_slices() {
        for invalid in ["", "2", "1:2:0", "1:2:3:4", "a:b", "1:2:no"] {
            assert!(SliceSpec::parse(invalid).is_err(), "{invalid:?}");
        }
    }
}
