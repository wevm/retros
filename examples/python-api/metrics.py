"""Starts the API on a free port, then writes latency per route, throughput, and a health check."""

import http.client
import os
import statistics
import subprocess
import sys
import time

server = subprocess.Popen([sys.executable, "app.py", "0"], stdout=subprocess.PIPE, text=True)
port = int(server.stdout.readline())


def get(path):
    """Sends one GET request and returns its status."""
    connection = http.client.HTTPConnection("127.0.0.1", port)
    connection.request("GET", path)
    response = connection.getresponse()
    response.read()
    connection.close()
    return response.status


lines = []

for route in ["/health", "/users", "/users/42"]:
    samples = []
    for _ in range(50):
        start = time.perf_counter()
        get(route)
        samples.append(time.perf_counter() - start)
    lines.append(f"latency {route} {statistics.median(samples) * 1000:.3f}ms")

start = time.perf_counter()
requests = 0
while time.perf_counter() - start < 0.5:
    get("/users/1")
    requests += 1
lines.append(f"throughput {requests / (time.perf_counter() - start):.0f}")

lines.append(f"health {'pass' if get('/health') == 200 else 'fail'}")

# Retros also stops processes that a script leaves running.
server.terminate()

with open(os.environ["RETROS_METRICS"], "a") as metrics:
    metrics.write("\n".join(lines) + "\n")
