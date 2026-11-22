# Examples

Each example is a small project with a `retros.toml` and the scripts it runs. The examples need only `sh`, Node, or Python, with no packages to install.

| Example | Language | Measures |
| --- | --- | --- |
| [`static-site`](static-site) | Shell | The size of each page, the stylesheet size, and a link check per page |
| [`node-library`](node-library) | JavaScript | The gzipped size of each entry point, and the import time of each entry point |
| [`python-api`](python-api) | Python | Latency per route from a server the script starts, throughput where higher is better, and a health check |

```sh
# Run each script once and check what it writes.
cd examples/static-site
retros check
```

Retros itself is measured the same way. Its [`retros.toml`](../retros.toml) tracks the release binary size, its dependencies, and its startup time.
