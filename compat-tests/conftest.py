import os
import pathlib
import subprocess
import pytest

ROOT = pathlib.Path(__file__).resolve().parents[1]

@pytest.fixture(scope="session")
def phrs_bin():
    subprocess.run(["cargo", "build"], cwd=ROOT, check=True)
    return ROOT / "target" / "debug" / "phrs"

@pytest.fixture(scope="session")
def oracle():
    # PH_PYTHON points to the executable of a checked-out Python `ph`.
    return os.environ.get("PH_PYTHON", "ph")

def run(argv, payload, exe):
    return subprocess.run([str(exe), *argv], input=payload, capture_output=True)
