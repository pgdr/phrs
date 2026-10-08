use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(args: &[&str], csv: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_phrs"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(csv).unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        output.stdout,
        output.stderr,
    )
}
const CSV: &[u8] = b"x,y\n3,8\n4,9\n5,10\n";

#[test]
fn columns_list() {
    let (code, stdout, _) = invoke(&["columns"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"columns\nx\ny\n");
}
#[test]
fn columns_select() {
    let (code, stdout, _) = invoke(&["columns", "y"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"y\n8\n9\n10\n");
}
#[test]
fn head_two() {
    let (code, stdout, _) = invoke(&["head", "2"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"x,y\n3,8\n4,9\n");
}
#[test]
fn tail_one() {
    let (code, stdout, _) = invoke(&["tail", "1"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"x,y\n5,10\n");
}
#[test]
fn rename_column() {
    let (code, stdout, _) = invoke(&["rename", "x", "z"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"z,y\n3,8\n4,9\n5,10\n");
}
#[test]
fn sort_descending() {
    let (code, stdout, _) = invoke(&["sort", "x", "--ascending=False"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"x,y\n5,10\n4,9\n3,8\n");
}
#[test]
fn shape() {
    let (code, stdout, _) = invoke(&["shape"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"rows,columns\n3,2\n");
}
#[test]
fn invalid_command() {
    let (code, _stdout, stderr) = invoke(&["unknown"], CSV);
    assert_ne!(code, 0);
    assert_eq!(
        stderr,
        b"Unknown command unknown.\nUsage: ph command [args]\n"
    );
}

#[test]
fn head_negative_excludes_last_rows() {
    let (code, stdout, _) = invoke(&["head", "-1"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"x,y\n3,8\n4,9\n");
}
#[test]
fn tail_negative_excludes_first_rows() {
    let (code, stdout, _) = invoke(&["tail", "-1"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"x,y\n4,9\n5,10\n");
}
#[test]
fn head_zero_preserves_header() {
    let (code, stdout, _) = invoke(&["head", "0"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"x,y\n");
}
#[test]
fn tail_zero_preserves_header() {
    let (code, stdout, _) = invoke(&["tail", "0"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"x,y\n");
}
#[test]
fn head_quoted_comma() {
    let (code, stdout, _) = invoke(&["head", "1"], b"x,y\n\"a,b\",2\nlast,3\n");
    assert_eq!(code, 0);
    assert_eq!(stdout, b"x,y\n\"a,b\",2\n");
}
#[test]
fn columns_reorder() {
    let (code, stdout, _) = invoke(&["columns", "y", "x"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, b"y,x\n8,3\n9,4\n10,5\n");
}
#[test]
fn stable_sort_ties() {
    let (code, stdout, _) = invoke(&["sort", "x"], b"x,y\n2,first\n1,a\n2,second\n");
    assert_eq!(code, 0);
    assert_eq!(stdout, b"x,y\n1,a\n2,first\n2,second\n");
}
#[test]
fn invalid_count_is_an_error() {
    let (code, stdout, _) = invoke(&["head", "nope"], CSV);
    assert_ne!(code, 0);
    assert!(stdout.is_empty());
}

#[test]
fn query_numeric() {
    let (code, out, err) = invoke(&["query", "x>3"], CSV);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(out, b"x,y\n4,9\n5,10\n");
}
#[test]
fn query_string() {
    let (code, out, err) = invoke(&["query", "name=='Alice'"], b"name,x\nAlice,1\nBob,2\n");
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(out, b"name,x\nAlice,1\n");
}
#[test]
fn strip_columns() {
    let (code, out, err) = invoke(&["strip", "name"], b"name,x\n  Alice  ,1\n Bob ,2\n");
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(out, b"name,x\nAlice,1\nBob,2\n");
}
#[test]
fn slugify_columns() {
    let (code, out, err) = invoke(&["slugify"], b"First Name,Some-Value\nAlice,1\n");
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(out, b"first_name,some_value\nAlice,1\n");
}

#[test]
fn eval_add_columns() {
    let (code, stdout, stderr) = invoke(&["eval", "z = x + y"], CSV);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"x,y,z\n3,8,11\n4,9,13\n5,10,15\n");
}
#[test]
fn eval_power_without_assignment() {
    let (code, stdout, stderr) = invoke(&["eval", "x**2"], CSV);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"x\n9\n16\n25\n");
}
#[test]
fn eval_parentheses_and_precedence() {
    let (code, stdout, stderr) = invoke(&["eval", "z = (x + y) * 2"], CSV);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"x,y,z\n3,8,22\n4,9,26\n5,10,30\n");
}
#[test]
fn eval_invalid_expression() {
    let (code, _, _) = invoke(&["eval", "z = x +"], CSV);
    assert_ne!(code, 0);
}

#[test]
fn show_simple_table() {
    let (code, out, err) = invoke(&["show"], CSV);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(
        out,
        b"      x    y\n--  ---  ---\n 0    3    8\n 1    4    9\n 2    5   10\n"
    );
}

#[test]
fn show_contains_null_and_unicode() {
    let csv = "name,age\nÅse,25\nBob,\n";
    let (code, out, err) = invoke(&["show"], csv.as_bytes());
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    let output = String::from_utf8(out).unwrap();
    assert!(output.contains("name"));
    assert!(output.contains("Åse"));
    assert!(output.contains("nan"));
}

#[test]
fn show_no_index() {
    let (code, out, err) = invoke(&["show", "--no-index"], CSV);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(
        out,
        b"  x    y
---  ---
  3    8
  4    9
  5   10
"
    );
}

#[test]
fn show_rejects_unknown_flag() {
    let (code, _, _) = invoke(&["show", "--typo"], CSV);
    assert_ne!(code, 0);
}

#[test]
fn cat_no_arguments_reads_stdin() {
    let (code, stdout, _) = invoke(&["cat"], CSV);
    assert_eq!(code, 0);
    assert_eq!(stdout, CSV);
}

#[test]
fn cat_axis_unknown() {
    let (code, _, stderr) = invoke(&["cat", "--axis=incorrect"], CSV);
    assert_ne!(code, 0);
    assert_eq!(stderr, b"Unknown axis command 'incorrect'\n");
}

#[test]
fn cat_files_vertical_and_horizontal() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.csv");
    let b = dir.path().join("b.csv");
    std::fs::write(&a, b"x,y\n1,2\n3,4\n").unwrap();
    std::fs::write(&b, b"z\n7\n").unwrap();
    let a = a.to_str().unwrap();
    let b = b.to_str().unwrap();
    let (code, stdout, stderr) = invoke(&["cat", a, b, "--axis=columns"], b"");
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"x,y,z\n1,2,7.0\n3,4,\n");
    let (code, stdout, stderr) = invoke(&["cat", a, a], b"");
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"x,y\n1,2\n3,4\n1,2\n3,4\n");
}

#[test]
fn merge_on_common_key_and_different_keys() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.csv");
    let b = dir.path().join("b.csv");
    std::fs::write(&a, b"key,A\n1,a\n2,b\n").unwrap();
    std::fs::write(&b, b"key,B\n2,c\n3,d\n").unwrap();
    let a = a.to_str().unwrap();
    let b = b.to_str().unwrap();
    let (code, stdout, stderr) = invoke(&["merge", a, b], b"");
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"key,A,B\n2,b,c\n");
    let (code, stdout, stderr) = invoke(&["merge", a, b, "--how=outer"], b"");
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"key,A,B\n1,a,\n2,b,c\n3,,d\n");
}
