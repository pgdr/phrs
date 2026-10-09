//! CLI tests for ph-compatible dropna and fillna handling.
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn invoke(args: &[&str], csv: &str) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_phrs"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(csv.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn success(args: &[&str], input: &str) -> String {
    let (code, stdout, stderr) = invoke(args, input);
    assert_eq!(code, 0, "{args:?}: {stderr}");
    stdout
}

const DATA: &str = "a,b,c\n1,,x\n2,3,y\n,4,z\n";

// pandas.read_csv promotes nullable integral columns to float64. Output
// after dropna (including --how=all and --axis=1) retains that dtype.
const DATA_AS_PANDAS_CSV: &str = "a,b,c\n1.0,,x\n2.0,3.0,y\n,4.0,z\n";

#[test]
fn dropna_rows_how_and_threshold() {
    assert_eq!(success(&["dropna"], DATA), "a,b,c\n2.0,3.0,y\n");
    assert_eq!(success(&["dropna", "--how=all"], DATA), DATA_AS_PANDAS_CSV);
    assert_eq!(success(&["dropna", "--thresh=2"], DATA), DATA_AS_PANDAS_CSV);
    assert_eq!(
        success(&["dropna", "--thresh=3"], DATA),
        "a,b,c\n2.0,3.0,y\n"
    );
    assert_eq!(success(&["dropna", "--thresh=4"], DATA), "a,b,c\n");
    assert_eq!(success(&["dropna", "--thresh=0"], DATA), DATA_AS_PANDAS_CSV);
    assert_eq!(
        success(&["dropna", "--thresh=-1"], DATA),
        DATA_AS_PANDAS_CSV
    );
}

#[test]
fn dropna_columns_how_and_threshold() {
    assert_eq!(success(&["dropna", "--axis=1"], DATA), "c\nx\ny\nz\n");
    assert_eq!(
        success(&["dropna", "--axis=1", "--how=all"], DATA),
        DATA_AS_PANDAS_CSV
    );
    assert_eq!(
        success(&["dropna", "--axis=1", "--thresh=2"], DATA),
        DATA_AS_PANDAS_CSV
    );
    assert_eq!(
        success(&["dropna", "--axis=1", "--thresh=3"], DATA),
        "c\nx\ny\nz\n"
    );
    assert_eq!(
        success(&["dropna", "--how=all"], "a,b\n,\n1,\n"),
        "a,b\n1.0,\n"
    );
    assert_eq!(
        success(&["dropna", "--axis=1", "--how=all"], "a,b\n1,\n2,\n"),
        "a\n1\n2\n"
    );
}

#[test]
fn dropna_handles_pandas_na_markers_and_nan() {
    let input = "a,b\n1,x\nNaN,y\nNA,z\nN/A,w\n5,v\n";
    assert_eq!(success(&["dropna"], input), "a,b\n1,x\n5,v\n");
    assert_eq!(
        success(&["dropna", "--axis=1"], input),
        "b\nx\ny\nz\nw\nv\n"
    );
    assert_eq!(
        success(&["dropna", "--how=all"], "a,b\nNA,\nNone,NULL\n2,z\n"),
        "a,b\n2,z\n"
    );
}

#[test]
fn fillna_numeric_scalar_and_quotas() {
    let input = "x,y\n1,\n,4\n,\n3,6\n";
    assert_eq!(
        success(&["fillna", "0"], input),
        "x,y\n1.0,0.0\n0.0,4.0\n0.0,0.0\n3.0,6.0\n"
    );
    assert_eq!(
        success(&["fillna", "99.75"], input),
        "x,y\n1.0,99.75\n99.75,4.0\n99.75,99.75\n3.0,6.0\n"
    );
    assert_eq!(
        success(&["fillna", "-1", "--limit=1"], input),
        "x,y\n1.0,-1.0\n-1.0,4.0\n,\n3.0,6.0\n"
    );
}

#[test]
fn fillna_forward_backward_and_aliases() {
    let input = "x,y\n,1\n2,\n,3\n,\n5,5\n,\n";
    assert_eq!(
        success(&["fillna", "--method=pad"], input),
        "x,y\n,1.0\n2.0,1.0\n2.0,3.0\n2.0,3.0\n5.0,5.0\n5.0,5.0\n"
    );
    assert_eq!(
        success(&["fillna", "--method=ffill", "--limit=1"], input),
        "x,y\n,1.0\n2.0,1.0\n2.0,3.0\n,3.0\n5.0,5.0\n5.0,5.0\n"
    );
    assert_eq!(
        success(&["fillna", "--method=bfill", "--limit=1"], input),
        "x,y\n2.0,1.0\n2.0,3.0\n,3.0\n5.0,5.0\n5.0,5.0\n,\n"
    );
    assert_eq!(
        success(&["fillna", "--method=backfill"], input),
        "x,y\n2.0,1.0\n2.0,3.0\n5.0,3.0\n5.0,5.0\n5.0,5.0\n,\n"
    );
}

#[test]
fn fillna_handles_text_and_na_literals() {
    let input = "name,x\nalpha,1\nNA,NaN\nbeta,3\n";
    assert_eq!(
        success(&["fillna", "0"], input),
        "name,x\nalpha,1.0\n0,0.0\nbeta,3.0\n"
    );
    assert_eq!(
        success(&["fillna", "missing"], input),
        "name,x\nalpha,1.0\nmissing,missing\nbeta,3.0\n"
    );
    assert_eq!(
        success(&["fillna", "--method=ffill"], input),
        "name,x\nalpha,1.0\nalpha,1.0\nbeta,3.0\n"
    );
    assert_eq!(
        success(&["fillna", "0"], "label\na\nNaN\nb\n"),
        "label\na\n0\nb\n"
    );
}

#[test]
fn fillna_preserves_literal_quotes_and_csv_escaping() {
    // The CSV writer, not AnyValue::Display, must quote and escape strings.
    let input = "name,x\n\"alpha, beta\",1\nNA,NaN\n\"beta \"\"quoted\"\"\",3\n";
    assert_eq!(
        success(&["fillna", "0"], input),
        "name,x\n\"alpha, beta\",1.0\n0,0.0\n\"beta \"\"quoted\"\"\",3.0\n"
    );
}

#[test]
fn dropna_and_fillna_compose_with_rolling() {
    let input = "x\n1\n2\n3\n4\n";
    let rolling = success(&["rolling", "2", "--how=mean"], input);
    assert_eq!(success(&["dropna"], &rolling), "x\n1.5\n2.5\n3.5\n");
    assert_eq!(
        success(&["fillna", "0"], &rolling),
        "x\n0.0\n1.5\n2.5\n3.5\n"
    );
}

#[test]
fn erroneous_invocations_fail_without_output() {
    for args in [
        vec!["dropna", "extra"],
        vec!["dropna", "--axis=2"],
        vec!["dropna", "--axis=columns"],
        vec!["dropna", "--how=neither"],
        vec!["dropna", "--thresh=abc"],
        vec!["dropna", "--extra=1"],
        vec!["fillna"],
        vec!["fillna", "0", "1"],
        vec!["fillna", "0", "--method=ffill"],
        vec!["fillna", "--method=sideways"],
        vec!["fillna", "0", "--limit=0"],
        vec!["fillna", "0", "--limit=-1"],
        vec!["fillna", "0", "--limit=hi"],
        vec!["fillna", "0", "--axis=1"],
    ] {
        let (code, output, _) = invoke(&args, DATA);
        assert_ne!(code, 0, "{args:?}");
        assert_eq!(output, "", "{args:?}");
    }
}

#[test]
fn dropna_dispatch_does_not_require_fillna_arguments() {
    // Exercise the library entry point independently of CLI subprocess routing.
    use std::ffi::OsString;
    let invocation = phrs::cli::parse(["phrs", "dropna"].into_iter().map(OsString::from)).unwrap();
    let mut result = Vec::new();
    phrs::execute_io(invocation, DATA.as_bytes(), &mut result).unwrap();
    assert_eq!(String::from_utf8(result).unwrap(), "a,b,c\n2.0,3.0,y\n");
}
