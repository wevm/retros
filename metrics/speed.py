"""Times the release binary: `retros --version`, and `retros check` on a one-category project.

Each value is the median of 20 runs, measured from Python, so it includes the cost of starting a
process. Retros repeats the script for its rounds and takes the median of those values.
"""

import os
import statistics
import subprocess
import tempfile
import time

RUNS = 20

subprocess.run(["cargo", "build", "--release", "--locked", "--quiet", "-p", "cli"], check=True)
binary = os.path.abspath("target/release/retros")


def median_ms(args, cwd=None):
    """Returns the median wall time of `args` over RUNS runs, in milliseconds."""
    samples = []

    for _ in range(RUNS):
        start = time.perf_counter_ns()
        subprocess.run(args, cwd=cwd, check=True, stdout=subprocess.DEVNULL)
        samples.append(time.perf_counter_ns() - start)

    return statistics.median(samples) / 1e6


with tempfile.TemporaryDirectory() as project:
    with open(os.path.join(project, "retros.toml"), "w") as config:
        config.write('[one]\nrun = "echo \'value 1\' >> \\"$RETROS_METRICS\\""\nvalue = {}\n')

    startup = median_ms([binary, "--version"])
    check = median_ms([binary, "check"], cwd=project)

with open(os.environ["RETROS_METRICS"], "a") as metrics:
    metrics.write(f"startup {startup:.3f}ms\n")
    metrics.write(f"check {check:.3f}ms\n")
