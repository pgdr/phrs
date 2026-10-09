//! Integration tests for the six reductions and the rolling command.
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn phrs(args: &[&str], input: &str) -> (i32, String, String) {
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
        .write_all(input.as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    (
        result.status.code().unwrap_or(-1),
        String::from_utf8(result.stdout).unwrap(),
        String::from_utf8(result.stderr).unwrap(),
    )
}

fn success(args: &[&str], input: &str) -> String {
    let (status, stdout, stderr) = phrs(args, input);
    assert_eq!(status, 0, "{args:?}: {stderr}");
    stdout
}

const SAMPLE: &str = "x,y\n3,8\n4,9\n5,10\n6,11\n7,12\n8,13\n";

#[test]
fn basic_six_reductions() {
    assert_eq!(success(&["sum"], SAMPLE), "x,y\n33,63\n");
    assert_eq!(success(&["mean"], SAMPLE), "x,y\n5.5,10.5\n");
    assert_eq!(success(&["median"], SAMPLE), "x,y\n5.5,10.5\n");
    assert_eq!(success(&["min"], SAMPLE), "x,y\n3,8\n");
    assert_eq!(success(&["max"], SAMPLE), "x,y\n8,13\n");
    let out = success(&["std"], SAMPLE);
    let values: Vec<f64> = out
        .lines()
        .nth(1)
        .unwrap()
        .split(',')
        .map(|s| s.parse::<f64>().unwrap())
        .collect();
    assert_eq!(values.len(), 2);
    for value in values {
        assert!((value - 1.8708286933869707).abs() < 1e-12);
    }
}

#[test]
fn reduction_selects_columns_and_supports_options() {
    assert_eq!(success(&["sum", "y"], SAMPLE), "y\n63\n");
    assert_eq!(
        success(&["sum", "--axis=1"], "x,y\n3,8\n4,9\n"),
        "0,1\n11,13\n"
    );
    assert_eq!(
        success(&["std", "--ddof=0"], "x\n1\n2\n3\n"),
        "x\n0.816496580927726\n"
    );
    assert_eq!(success(&["median"], "x\n1\n2\n3\n4\n"), "x\n2.5\n");
}

#[test]
fn missing_values_and_numeric_only() {
    let input = "x,y\n1,\n,4\n3,6\n";
    assert_eq!(success(&["sum"], input), "x,y\n4.0,10.0\n");
    assert_eq!(success(&["sum", "--min_count=3"], input), "x,y\n,\n");
    assert_eq!(success(&["sum", "--skipna=False"], input), "x,y\n,\n");
    assert_eq!(success(&["mean"], input), "x,y\n2.0,5.0\n");
    assert_eq!(
        success(&["std"], input),
        "x,y\n1.4142135623730951,1.4142135623730951\n"
    );
    assert_eq!(
        success(&["sum", "--numeric_only=True"], "x,name\n3,A\n4,B\n"),
        "x\n7\n"
    );
    assert_eq!(success(&["sum"], "x,name\n3,A\n4,B\n"), "x,name\n7,AB\n");
    assert_eq!(success(&["max"], "x,name\n3,A\n4,B\n"), "x,name\n4,B\n");
    let (status, _, _) = phrs(&["mean"], "x,name\n3,A\n4,B\n");
    assert_ne!(status, 0);
}

#[test]
fn rolling_mean_sum_median_min_max_std() {
    assert_eq!(
        success(&["rolling", "3", "--how=mean"], SAMPLE),
        "x,y\n,\n,\n4.0,9.0\n5.0,10.0\n6.0,11.0\n7.0,12.0\n"
    );
    assert_eq!(
        success(&["rolling", "2", "--how=sum"], SAMPLE),
        "x,y\n,\n7.0,17.0\n9.0,19.0\n11.0,21.0\n13.0,23.0\n15.0,25.0\n"
    );
    assert_eq!(
        success(
            &["rolling", "3", "x", "--how=median"],
            "x,y\n5,1\n1,2\n3,3\n7,4\n"
        ),
        "x,y\n,1\n,2\n3.0,3\n3.0,4\n"
    );
    assert_eq!(
        success(&["rolling", "2", "x", "--how=min"], "x,y\n5,1\n1,2\n3,3\n"),
        "x,y\n,1\n1.0,2\n1.0,3\n"
    );
    assert_eq!(
        success(&["rolling", "2", "x", "--how=max"], "x,y\n5,1\n1,2\n3,3\n"),
        "x,y\n,1\n5.0,2\n3.0,3\n"
    );
    assert_eq!(
        success(&["rolling", "2", "x", "--how=std"], "x,y\n1,1\n3,2\n5,3\n"),
        "x,y\n,1\n1.4142135623730951,2\n1.4142135623730951,3\n"
    );
}

#[test]
fn rolling_column_selection_and_missing_values() {
    assert_eq!(
        success(
            &["rolling", "2", "x", "--how=mean"],
            "date,x,y\na,1,10\nb,2,20\nc,3,30\n"
        ),
        "date,x,y\na,,10\nb,1.5,20\nc,2.5,30\n"
    );
    assert_eq!(
        success(
            &["rolling", "2", "--how=mean"],
            "date,x,y\na,1,10\nb,2,20\nc,3,30\n"
        ),
        "x,y\n,\n1.5,15.0\n2.5,25.0\n"
    );
    assert_eq!(
        success(
            &["rolling", "3", "x", "--how=sum", "--min_periods=1"],
            "x,y\n1,1\n,2\n3,3\n4,4\n"
        ),
        "x,y\n1.0,1\n1.0,2\n4.0,3\n7.0,4\n"
    );
    assert_eq!(
        success(
            &["rolling", "3", "x", "--how=mean", "--center=True"],
            "x,y\n1,1\n2,2\n3,3\n4,4\n5,5\n"
        ),
        "x,y\n,1\n2.0,2\n3.0,3\n4.0,4\n,5\n"
    );
    // An even centered window is biased one position to the left, as in pandas.
    assert_eq!(
        success(
            &["rolling", "2", "x", "--how=sum", "--center=True"],
            "x,y\n1,1\n2,2\n3,3\n4,4\n"
        ),
        "x,y\n,1\n3.0,2\n5.0,3\n7.0,4\n"
    );
}

#[test]
fn invalid_options_and_windows_do_not_write_results() {
    for args in [
        vec!["sum", "--typo=1"],
        vec!["std", "--ddof=abc"],
        vec!["max", "missing"],
        vec!["rolling"],
        vec!["rolling", "0"],
        vec!["rolling", "bad"],
        vec!["rolling", "2", "--how=unknown"],
        vec!["rolling", "2", "--min_periods=3"],
        vec!["rolling", "2", "--center=maybe"],
        vec!["rolling", "2", "--typo=1"],
        vec!["rolling", "2", "missing"],
    ] {
        let (status, stdout, _) = phrs(&args, SAMPLE);
        assert_ne!(status, 0, "{args:?}");
        assert!(stdout.is_empty(), "{args:?}");
    }
}
