use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

fn invoke(args: &[&str], input: &str) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_phrs"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start phrs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn success(args: &[&str], input: &str) -> String {
    let (status, output, stderr) = invoke(args, input);
    assert_eq!(status, 0, "{args:?}: {stderr}");
    assert!(stderr.is_empty(), "{args:?}: {stderr}");
    output
}

#[test]
fn round_defaults_to_zero_and_preserves_other_columns() {
    let csv = "x,label\n1.25,a\n2.5,b\n3.5,c\n-2.5,d\n";
    assert_eq!(
        success(&["round", "x"], csv),
        "x,label\n1.0,a\n2.0,b\n4.0,c\n-2.0,d\n"
    );
}

#[test]
fn round_accepts_positional_and_option_decimals() {
    let csv = "x,y\n1.25,one\n1.35,two\n-1.25,three\n";
    let expected = "x,y\n1.2,one\n1.4,two\n-1.2,three\n";
    assert_eq!(success(&["round", "x", "1"], csv), expected);
    assert_eq!(success(&["round", "x", "--decimals=1"], csv), expected);
}

#[test]
fn round_negative_decimals_on_integers_exactly() {
    let csv = "x,y\n25,first\n35,second\n-25,third\n125,fourth\n";
    assert_eq!(
        success(&["round", "x", "--decimals=-1"], csv),
        "x,y\n20,first\n40,second\n-20,third\n120,fourth\n"
    );
}

#[test]
fn round_preserves_large_integers() {
    let csv = "large\n9007199254740993\n9007199254740995\n";
    assert_eq!(success(&["round", "large", "0"], csv), csv);
    assert_eq!(
        success(&["round", "large", "-1"], csv),
        "large\n9007199254740990\n9007199254741000\n"
    );
}

#[test]
fn round_preserves_nulls_and_unselected_columns() {
    let csv = "x,label\n1.25,a\n,b\n2.75,c\n";
    assert_eq!(
        success(&["round", "x", "1"], csv),
        "x,label\n1.2,a\n,b\n2.8,c\n"
    );
}

#[test]
fn round_rejects_invalid_arguments_without_stdout() {
    for args in [
        vec!["round"],
        vec!["round", "x", "not-a-number"],
        vec!["round", "x", "1", "--decimals=2"],
        vec!["round", "x", "--decimals=1.2"],
        vec!["round", "x", "--unknown=1"],
        vec!["round", "missing"],
        vec!["round", "label"],
    ] {
        let (status, stdout, stderr) = invoke(&args, "x,label\n1.25,alpha\n");
        assert_ne!(status, 0, "{args:?}");
        assert!(stdout.is_empty(), "{args:?}: unexpected stdout");
        assert!(!stderr.is_empty(), "{args:?}: missing diagnostic");
    }
}

#[test]
fn help_lists_commands_and_explains_round() {
    let general = success(&["help"], "");
    assert!(general.contains("round"));
    assert!(general.contains("describe"));
    assert!(general.contains("rolling"));
    assert!(general.contains("help COMMAND"));
    let round_help = success(&["help", "round"], "");
    assert!(round_help.contains("Usage: phrs round COLUMN"));
    assert!(round_help.contains("--decimals=N"));
    assert!(round_help.contains("ties-to-even"));
    assert!(success(&["help", "help"], "").contains("help [COMMAND]"));
}

#[test]
fn help_unknown_command_and_bad_arguments_error() {
    for args in [
        vec!["help", "missing"],
        vec!["help", "round", "other"],
        vec!["help", "--unknown"],
    ] {
        let (status, stdout, stderr) = invoke(&args, "");
        assert_ne!(status, 0, "{args:?}");
        assert!(stdout.is_empty(), "{args:?}");
        assert!(!stderr.is_empty(), "{args:?}");
    }
}

struct FailIfRead;
impl Read for FailIfRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("help must not read stdin"))
    }
}

#[test]
fn help_does_not_read_stdin() {
    let inv = phrs::cli::parse(
        ["phrs", "help", "round"]
            .into_iter()
            .map(std::ffi::OsString::from),
    )
    .unwrap();
    let mut output = Vec::new();
    phrs::execute_io(inv, FailIfRead, &mut output).unwrap();
    assert!(String::from_utf8(output).unwrap().contains("round COLUMN"));
}
