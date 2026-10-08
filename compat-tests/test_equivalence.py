"""Opt-in byte-for-byte comparisons against an installed Python ph executable.

These are genuine comparisons: no xfail or normalized stderr. Set PH_PYTHON to
an executable from the chosen, pinned upstream checkout.
"""
import pytest
from conftest import run

CSV = b"x,y\n3,8\n4,9\n5,10\n6,11\n7,12\n8,13\n"

CASES = [
    (["columns"], CSV),
    (["columns", "y", "x"], CSV),
    (["columns", "x"], CSV),
    (["head"], CSV),
    (["head", "0"], CSV),
    (["head", "2"], CSV),
    (["head", "-2"], CSV),
    (["head", "100"], CSV),
    (["tail"], CSV),
    (["tail", "0"], CSV),
    (["tail", "1"], CSV),
    (["tail", "-2"], CSV),
    (["tail", "100"], CSV),
    (["rename", "x", "z"], CSV),
    (["sort", "x"], CSV),
    (["sort", "x", "--ascending=False"], CSV),
    (["sort", "x"], b"x,y\n3,a\n1,b\n2,c\n"),
    (["shape"], CSV),
    (["columns", "x"], b"x,y\n"),
    (["head", "1"], b'x,y\n"hello, there",2\n'),
    (["tail", "1"], b"x,y\n,NaN\n"),
    (["head", "1"], b'x,y\n"multi\nline",1\nhi,2\n'),
    # Failure semantics must be compared, not guessed.
    (["bogus"], CSV),
    (["columns", "does_not_exist"], CSV),
    (["head", "xyz"], CSV),
    (["rename", "missing", "new"], CSV),
    (["sort", "missing"], CSV),
]

@pytest.mark.parametrize("argv,payload", CASES, ids=lambda case: str(case)[:70])
def test_equivalent(argv, payload, phrs_bin, oracle):
    py = run(argv, payload, oracle)
    rs = run(argv, payload, phrs_bin)
    assert (rs.returncode, rs.stdout, rs.stderr) == (py.returncode, py.stdout, py.stderr)
