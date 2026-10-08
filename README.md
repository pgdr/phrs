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
