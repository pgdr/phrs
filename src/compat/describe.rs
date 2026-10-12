//! Polars implementation of Pandas Describe by nazq (gist):
//! https://gist.github.com/nazq/f3fc6e6b15c895fb8b474013c4932d62
//!
//! DataFrame describe() functionality for Polars in Rust
//!
//! This module provides a `describe()` method for both DataFrame and LazyFrame,
//! similar to pandas/polars describe() in Python. It computes summary statistics
//! including count, null_count, mean, std, min, percentiles, and max.
//!
//! The implementation follows the Python polars pattern closely, avoiding
//! unnecessary data collection when working with LazyFrames.

use anyhow::Result;
use polars::prelude::*;

/// Trait for types that can produce descriptive statistics
pub trait Describable {
    /// Compute descriptive statistics
    ///
    /// # Arguments
    /// * `percentiles` - Optional vector of percentiles to compute (values between 0.0 and 1.0)
    ///   Defaults to [0.25, 0.50, 0.75] if None
    ///
    /// # Returns
    /// A DataFrame containing statistics for each column:
    /// - count: number of non-null values
    /// - null_count: number of null values
    /// - mean: average value (numeric/temporal/boolean columns)
    /// - std: standard deviation (numeric columns only)
    /// - min: minimum value
    /// - percentiles: requested percentiles
    /// - max: maximum value
    ///
    /// # Example
    /// ```rust
    /// use polars::prelude::*;
    /// use phrs::Describable;
    ///
    /// fn main() -> anyhow::Result<()> {
    ///     let df = df! {
    ///         "ints" => [1, 2, 3, 4, 5],
    ///         "floats" => [1.0, 2.5, 3.0, 4.5, 5.0],
    ///         "strings" => ["a", "b", "c", "d", "e"],
    ///     }?;
    ///
    ///     let stats = df.describe(None)?;
    ///     println!("{}", stats);
    ///     Ok(())
    /// }
    /// ```
    fn describe(&self, percentiles: Option<Vec<f64>>) -> Result<DataFrame>;
}

/// Implementation for DataFrame
impl Describable for DataFrame {
    fn describe(&self, percentiles: Option<Vec<f64>>) -> Result<DataFrame> {
        // Convert to LazyFrame and use the efficient implementation
        let lf = self.clone().lazy();
        describe_lazy_impl(&lf, percentiles)
    }
}

/// Implementation for LazyFrame
impl Describable for LazyFrame {
    fn describe(&self, percentiles: Option<Vec<f64>>) -> Result<DataFrame> {
        describe_lazy_impl(self, percentiles)
    }
}

/// Internal implementation that works purely with LazyFrame
/// This follows the same pattern as the Python implementation
#[allow(clippy::too_many_lines)]
fn describe_lazy_impl(lazy_frame: &LazyFrame, percentiles: Option<Vec<f64>>) -> Result<DataFrame> {
    use polars::lazy::dsl;
    use polars::prelude::{NULL, QuantileMethod};

    // Get schema without collecting the data
    let mut lf_mut = lazy_frame.clone();
    let schema = lf_mut.collect_schema()?;

    if schema.is_empty() {
        return Err(anyhow::anyhow!(
            "cannot describe a LazyFrame that has no columns"
        ));
    }

    // Default percentiles if not provided
    let percentiles = percentiles.unwrap_or_else(|| vec![0.25, 0.50, 0.75]);

    // Build statistic row names (metrics)
    let mut metrics = vec![
        "count".to_string(),
        "null_count".to_string(),
        "mean".to_string(),
        "std".to_string(),
        "min".to_string(),
    ];
    for p in &percentiles {
        #[allow(clippy::cast_possible_truncation)]
        metrics.push(format!("{}%", (p * 100.0) as i32));
    }
    metrics.push("max".to_string());

    // Helper to check if we skip min/max
    let skip_minmax = |dtype: &DataType| -> bool {
        dtype.is_nested()
            || matches!(
                dtype,
                DataType::Categorical(..) | DataType::Null | DataType::Unknown(_)
            )
    };

    // Build all metric expressions for all columns in a single pass
    let mut metric_exprs = Vec::new();

    // Loop over columns and datatypes (like Python: for c, dtype in schema.items())
    for (col_name, dtype) in schema.iter() {
        let col_name_str = col_name.to_string();
        let col = dsl::col(&col_name_str);

        // Determine if numeric or temporal
        let is_numeric = dtype.is_numeric();
        let is_temporal = !is_numeric && dtype.is_temporal();

        // Count expressions - for all columns
        let count_expr = col.clone().count().alias(format!("count:{col_name_str}"));
        let null_count_expr = col
            .clone()
            .null_count()
            .alias(format!("null_count:{col_name_str}"));

        // Mean - for temporal, numeric, or boolean
        let mean_expr = if is_temporal || is_numeric || dtype == &DataType::Boolean {
            if dtype == &DataType::Boolean {
                col.clone().cast(DataType::Float64).mean()
            } else {
                col.clone().mean()
            }
        } else {
            dsl::lit(NULL).cast(DataType::Float64)
        };
        let mean_expr = mean_expr.alias(format!("mean:{col_name_str}"));

        // Standard deviation - only for numeric
        let std_expr = if is_numeric {
            col.clone().std(1) // ddof=1 for sample std
        } else {
            dsl::lit(NULL).cast(DataType::Float64)
        };
        let std_expr = std_expr.alias(format!("std:{col_name_str}"));

        // Min/Max - based on skip_minmax
        let min_expr = if skip_minmax(dtype) {
            dsl::lit(NULL).cast(DataType::Float64)
        } else {
            col.clone().min()
        };
        let min_expr = min_expr.alias(format!("min:{col_name_str}"));

        let max_expr = if skip_minmax(dtype) {
            dsl::lit(NULL).cast(DataType::Float64)
        } else {
            col.clone().max()
        };
        let max_expr = max_expr.alias(format!("max:{col_name_str}"));

        // Percentiles - for numeric or temporal
        let mut pct_exprs = Vec::new();
        for (i, p) in percentiles.iter().enumerate() {
            let pct_expr = if is_numeric || is_temporal {
                // For temporal types, we might need special handling
                // For now, just use quantile directly
                col.clone().quantile(dsl::lit(*p), QuantileMethod::Linear)
            } else {
                dsl::lit(NULL).cast(DataType::Float64)
            };
            pct_exprs.push(pct_expr.alias(format!("{p}:{i}:{col_name_str}")));
        }

        // Add all expressions for this column
        metric_exprs.push(count_expr);
        metric_exprs.push(null_count_expr);
        metric_exprs.push(mean_expr);
        metric_exprs.push(std_expr);
        metric_exprs.push(min_expr);
        metric_exprs.extend(pct_exprs);
        metric_exprs.push(max_expr);
    }

    // Execute all aggregations in a single pass
    let df_metrics = lazy_frame.clone().select(metric_exprs).collect()?;

    // Reshape the wide result into the final format
    let n_metrics = metrics.len();
    let mut result_series = Vec::new();

    // Add the statistic column first
    result_series.push(Series::new("statistic".into(), metrics.clone()));

    // Process each column's metrics
    for (col_name, dtype) in schema.iter() {
        let col_name_str = col_name.to_string();
        let mut col_values = Vec::new();

        // Extract values for this column from the metrics DataFrame
        // The metrics are in groups of n_metrics per column
        // let base_idx = idx * n_metrics;  // Not needed with column name lookup

        // Helper to format values based on type
        let is_numeric_result = dtype.is_numeric()
            || dtype.is_nested()
            || matches!(dtype, DataType::Null | DataType::Boolean);

        // Extract each metric for this column
        for metric_idx in 0..n_metrics {
            // let _col_idx = base_idx + metric_idx;  // Not needed
            let metric_name = match metric_idx {
                0 => format!("count:{col_name_str}"),
                1 => format!("null_count:{col_name_str}"),
                2 => format!("mean:{col_name_str}"),
                3 => format!("std:{col_name_str}"),
                4 => format!("min:{col_name_str}"),
                i if i < n_metrics - 1 => {
                    // Percentile
                    let pct_idx = i - 5;
                    let p = &percentiles[pct_idx];
                    format!("{p}:{pct_idx}:{col_name_str}")
                }
                _ => format!("max:{col_name_str}"),
            };

            // Get the value from df_metrics
            if let Ok(val) = df_metrics.column(&metric_name)?.get(0) {
                // Format based on type and metric
                let formatted = if val.is_null() {
                    "null".to_string()
                } else if metric_idx <= 1 {
                    // count and null_count - always as integer
                    format!("{val}")
                } else if is_numeric_result && (metric_idx == 2 || metric_idx == 3) {
                    // mean and std for numeric - format with decimals
                    format!("{val:.6}")
                } else if dtype == &DataType::Boolean
                    && (metric_idx == 4 || metric_idx == n_metrics - 1)
                {
                    // min/max for boolean
                    if metric_idx == 4 {
                        "false".to_string()
                    } else {
                        "true".to_string()
                    }
                } else {
                    format!("{val}")
                };

                col_values.push(formatted);
            } else {
                col_values.push("null".to_string());
            }
        }

        // Add this column's values to the result
        result_series.push(Series::new(col_name_str.into(), col_values));
    }

    DataFrame::new(
        n_metrics,
        result_series.into_iter().map(Column::from).collect(),
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_describe_numeric() -> Result<()> {
        let df = df! {
            "ints" => [1, 2, 3, 4, 5],
            "floats" => [1.0, 2.0, 3.0, 4.0, 5.0],
        }?;

        let stats = df.describe(None)?;

        // Check shape
        assert_eq!(stats.shape(), (9, 3)); // 9 stats x 3 columns (statistic + 2 data cols)

        // Check that statistic column exists
        assert!(stats.column("statistic").is_ok());

        Ok(())
    }

    #[test]
    fn test_describe_with_custom_percentiles() -> Result<()> {
        let df = df! {
            "values" => [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        }?;

        let stats = df.describe(Some(vec![0.1, 0.5, 0.9]))?;

        // Check that we have the right number of rows
        // count, null_count, mean, std, min, 10%, 50%, 90%, max = 9 rows
        assert_eq!(stats.height(), 9);

        Ok(())
    }

    #[test]
    fn test_describe_mixed_types() -> Result<()> {
        let df = df! {
            "numbers" => [1, 2, 3],
            "strings" => ["a", "b", "c"],
            "bools" => [true, false, true],
        }?;

        let stats = df.describe(None)?;

        // Should not panic and should return stats for all columns
        assert_eq!(stats.width(), 4); // statistic + 3 data columns

        Ok(())
    }

    #[test]
    fn test_describe_lazy_frame() -> Result<()> {
        let df = df! {
            "a" => [1, 2, 3, 4, 5],
            "b" => [10.0, 20.0, 30.0, 40.0, 50.0],
        }?;

        let lf = df.lazy();
        let stats = lf.describe(None)?;

        // Should work with LazyFrame without collecting first
        assert_eq!(stats.shape(), (9, 3));

        Ok(())
    }
}
