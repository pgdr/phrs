# phrs — the tabular data shell tool

`phrs` is a Rust command-line tool for manipulating CSV data in Unix pipelines. It is inspired by [ph](https://github.com/pgdr/ph) and uses [Polars](https://pola.rs/) for tabular operations.

## Install

```bash
cargo install phrs
```

## Getting started

Suppose `a.csv` contains:

```csv
x,y
3,8
4,9
5,10
6,11
7,12
8,13
```

Commands usually read CSV from standard input and write CSV to standard output, so they can be chained:

```bash
cat a.csv | phrs columns y x | phrs head 3
```

```csv
y,x
8,3
9,4
10,5
```

To display a human-readable table, use `show`:

```bash
cat a.csv | phrs show
```

```text
      x    y
--  ---  ---
 0    3    8
 1    4    9
 2    5   10
 3    6   11
 4    7   12
 5    8   13
```

`show` produces terminal output rather than CSV; it is normally the last command in a pipeline.

## Commands

### Inspect and select columns

```bash
cat a.csv | phrs columns          # List column names
cat a.csv | phrs columns y x      # Select and reorder columns
cat a.csv | phrs shape            # Output row and column counts as CSV
```

### Select and order rows

```bash
cat a.csv | phrs head             # First 10 rows (default)
cat a.csv | phrs head 3
cat a.csv | phrs tail             # Last 10 rows (default)
cat a.csv | phrs tail 3
cat a.csv | phrs sort x           # Sort by column x
cat a.csv | phrs slice :3         # First three rows
cat a.csv | phrs slice 1:9:2      # Every second row from index 1
cat a.csv | phrs slice ::-1       # Reverse the rows
```

### Rename and clean columns

```bash
cat a.csv | phrs rename x number  # Rename column x to number
cat a.csv | phrs slugify          # Normalize column names
cat a.csv | phrs strip            # Trim whitespace in string columns
cat a.csv | phrs strip x          # Trim a specified column
```

### Filter rows with `query`

`query` supports simple column-to-value comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`):

```bash
cat a.csv | phrs query 'x>5'
cat a.csv | phrs query 'y <= 10'
```

### Compute columns with `eval`

`eval` supports arithmetic expressions, assignments, numeric constants, parentheses, and exponentiation:

```bash
cat a.csv | phrs eval 'z = x + y'
cat a.csv | phrs eval 'z = x**2 + y'
cat a.csv | phrs eval 'z = (x + y) / 2'
cat a.csv | phrs eval 'x**2'       # Output only the computed column
```

### Compute differences with `diff`

`diff` subtracts the previous row from the current row. By default it processes all columns; specify column names to limit the operation. Use `--periods` to change the offset or `--axis=1` to compute differences across columns.

```bash
cat a.csv | phrs diff x
cat a.csv | phrs diff x --periods=2
cat a.csv | phrs diff --axis=1
```

### Aggregate columns with `sum`, `mean`, `median`, `std`, `min`, `max`

Each command reduces the input CSV to a **single-row CSV**, preserving the
original column headers. By default reductions are column-wise (`--axis=0`).
Select particular columns by name, or pass `--axis=1` to aggregate across
columns and produce a single row indexed by the original row numbers.

```bash
cat a.csv | phrs sum                  # x,y / 33,63
cat a.csv | phrs mean                 # x,y / 5.5,10.5
cat a.csv | phrs median
cat a.csv | phrs std                  # Sample standard deviation (ddof=1)
cat a.csv | phrs min
cat a.csv | phrs max
cat a.csv | phrs sum x                # Reduce only column x
cat a.csv | phrs mean --axis=1        # Mean of each input row
cat a.csv | phrs std --ddof=0         # Population standard deviation
cat a.csv | phrs mean --numeric_only=True
cat a.csv | phrs sum --min_count=2
cat a.csv | phrs mean --skipna=False
```

Missing numeric values are excluded by default; `--skipna=False` propagates
missing values. `sum` defaults to zero for an entirely missing column (unless
`--min_count` requests more observations). For mixed numeric/text data,
`sum` concatenates text and `min` / `max` compare strings. `mean`, `median`
and `std` require numeric columns unless `--numeric_only=True` is supplied.

### Handle missing values with `dropna` and `fillna`

`dropna` removes rows containing missing cells (`--axis=0`, the default),
whereas `--axis=1` removes columns. By default, any missing cell causes the
row/column to be removed; `--how=all` removes only entirely missing rows/columns.
`--thresh=N` instead retains rows/columns containing at least N nonmissing
cells. Missing values include CSV blanks, NaNs, and pandas' standard NA strings.

```bash
cat a.csv | phrs dropna
cat a.csv | phrs dropna --how=all
cat a.csv | phrs dropna --thresh=5
cat a.csv | phrs dropna --axis=1
```

`fillna` replaces missing cells either with a scalar or using the closest
preceding/following nonmissing value in the same column. Use `--method=pad` or
`--method=ffill` for forward filling, and `--method=bfill` or
`--method=backfill` for backward filling. A method and a scalar value are
mutually exclusive.

```bash
cat a.csv | phrs fillna 0
cat a.csv | phrs fillna 999.75
cat a.csv | phrs fillna missing
cat a.csv | phrs fillna --method=pad
cat a.csv | phrs fillna --method=bfill --limit=7
cat a.csv | phrs fillna 0 --limit=2
```

For a **method**, `--limit` caps the number of consecutive gaps filled per
run. For a **scalar**, it caps the total number of missing values filled per
column, following pandas' behavior.

### Compute rolling statistics with `rolling`

`rolling WINDOW [COLUMN ...]` computes a trailing window reduction. Its
`--how` option accepts `sum` (the default), `mean`, `median`, `std`, `min`, and
`max`. It returns a CSV with the **same number of rows**. Results are
floating-point, and incomplete windows are blank by default.

```bash
cat a.csv | phrs rolling 3 --how=mean
cat a.csv | phrs rolling 2 --how=sum
cat a.csv | phrs rolling 3 --how=median
cat a.csv | phrs rolling 5 --how=std --ddof=0
cat a.csv | phrs rolling 7 x y --how=mean  # Preserve unselected columns
cat a.csv | phrs rolling 3 --how=max --min_periods=1
cat a.csv | phrs rolling 3 --how=mean --center=True
```

When specific column names are supplied, only those columns are replaced;
other columns (such as timestamps) retain their original values and order.
Without column names, rolling aggregates all numeric columns and excludes
nonnumeric columns. `--min_periods` controls the minimum number of nonmissing
observations (default: the window size). `--center=True` centers the output
within each window. `std` uses sample standard deviation (`ddof=1`) unless
another `--ddof` is supplied.

### Parse dates with `date`

`date` converts a column to ISO-formatted dates. Numeric values are interpreted as days since the Unix epoch, or as Unix timestamps in seconds with `--utc=True`. String dates can use an explicit format or day-first parsing.

```bash
cat a.csv | phrs date x
cat dates.csv | phrs date recorded --format="%d/%m/%Y"
cat dates.csv | phrs date recorded --dayfirst=True
cat timestamps.csv | phrs date timestamp --utc=True
```

### Display a table

```bash
cat a.csv | phrs show
cat a.csv | phrs show --no-index  # Omit the generated row index
```

### Open Excel workbooks

`open excel` reads the first worksheet of an `.xls`, `.xlsx`, `.xlsm`, or
`.xlsb` file and writes CSV to standard output. Use `--sheet=NAME` to select
another worksheet, or `--sheet_name=N` to select a zero-based worksheet index
(as in `ph` and pandas). `--sheet_name=NAME` also accepts worksheet names.
Unlike ordinary commands, it reads the named file, not stdin.

```bash
phrs open excel data.xls
phrs open excel data.xlsx --sheet=Measurements
phrs open excel data.xlsx --sheet_name=1  # Second worksheet
phrs open excel data.xls | phrs head 10
```

Cells are exported as values: dates use ISO notation, blank cells become empty
fields, and commas, quotes, and newlines are CSV-escaped. Excel cell formatting
(such as currency symbols or formatted leading zeros) is not reproduced.
Formulas are **not evaluated**; their cached results, if available, are exported.

### Concatenate CSV files

Unlike most commands, `cat` takes filenames. With no filenames, it reads standard input.

```bash
phrs cat a.csv b.csv
phrs cat a.csv b.csv --axis=index
phrs cat a.csv b.csv --axis=columns
cat a.csv | phrs cat
```

### Merge CSV files

`merge` reads two named CSV files and joins their rows. The default join type is `inner`; supported types are `inner`, `left`, `right`, and `outer`.

```bash
phrs merge left.csv right.csv
phrs merge left.csv right.csv --how=outer
phrs merge left.csv right.csv --on=key
phrs merge left.csv right.csv --left=left_key --right=right_key
```

When no join key is provided, common column names are used as join keys.

## Pipelines

```bash
cat a.csv \
  | phrs query 'x>4' \
  | phrs eval 'z = x * y' \
  | phrs columns x z \
  | phrs sort x \
  | phrs show
```

## Development

```bash
cargo test
cargo build --release
```

The optimized executable is written to `target/release/phrs`.

`phrs` aims to reproduce the behavior of the Python [`ph`](https://github.com/pgdr/ph) tool for its implemented commands, but does not yet support every `ph` operation. Differences are tracked using differential tests against the Python implementation.

## License

MIT.
