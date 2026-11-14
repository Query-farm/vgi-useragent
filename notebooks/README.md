# VGI User-Agent notebooks

[01 · Count requests without inventing identities](01_count_requests_without_inventing_identities.ipynb) parses synthetic User-Agent strings, joins a distinct-string dictionary back to request rows, and compares requests with distinct nonempty strings. It preserves missing/empty/unrecognized headers and makes bot-signature filtering explicit.

## Run

Install [uv](https://docs.astral.sh/uv/). From the repository root:

```sh
uv sync --project notebooks --frozen
uv run --project notebooks --frozen python notebooks/execute.py
```

The complete `notebooks/` directory also runs without the worker checkout:

```sh
cd notebooks
uv sync --frozen
uv run --frozen python execute.py
```

Initial setup needs network access for the Python environment, community VGI extension and pinned native worker release. No Rust toolchain, local build, credentials or external dataset is required. Parsing is local, using regex data embedded in the release. Native archives are available for macOS Apple silicon, Linux x86-64/arm64 and Windows x86-64. Notebook execution was verified on macOS Apple silicon.

In VS Code choose `notebooks/.venv/bin/python` and use `notebooks/` as the kernel working directory. For other Jupyter editors, optionally register the environment:

```sh
uv run --project notebooks --frozen python -m ipykernel install --user --name vgi-useragent-notebooks --display-name 'Python (vgi-useragent notebooks)'
```

Restart and run all after editing parameters. Connection and analytical SQL remain expanded; setup, plotting, checks and cleanup source are folded. The saved notebook embeds both charts.

## Experiment

```sh
uv run --project notebooks --frozen python notebooks/execute.py \
  --parameters '{"EXCLUDE_IDENTIFIED_BOTS":false}' \
  --output-dir notebooks/validation/include-bots
```

The default excludes only explicit true bot flags. Switching to false includes the identified signatures and changes the reporting denominator; all parsed values and the full header-state audit remain fixed. Reset the parameter to true for the saved example.

The runner requires a separate destination for overrides, rejects unknown parameter names, and saves only successful full executions. Failed runs leave prior saved outputs untouched. Experiment copies include relative branding and runtime companions. Review PNGs and other generated validation files are ignored.

## Portable startup and input provenance

The literal `ATTACH` location is `github-auto://Query-farm/vgi-useragent@v0.4.0`. The VGI extension downloads the platform release, checks its checksum and caches it. The notebook lock pins Haybarn 2.0.0rc11. To upgrade, change the tag explicitly and rerun both settings; the embedded uap-core regex data can change classifications between releases.

All eighteen request rows are synthetic, expanded from ten authored patterns. Familiar browser, curl and crawler strings are illustrative text, not records collected from those services. The worker does not follow the URLs inside the headers. The input includes two Chrome versions, repeated strings, a custom unknown client, an empty string and SQL NULL. There are no user, session or device identifiers.

The dictionary is parsed once per distinct supplied value, including one NULL key; an `IS NOT DISTINCT FROM` join preserves missing-header requests. The input strings are within the worker's 64 KiB matching bound. No completeness claim is made for oversized, spoofed, reduced or future headers.

## Interpretation

Request count and distinct nonempty-string count have different meanings. Multiple requests can reuse one string, and different strings can map to one family. Neither measure counts visitors. Empty and missing buckets contain requests but zero observed nonempty strings. Unknown families remain NULL; missing device/brand fields do not imply desktop hardware.

The worker's bot flag comes from the embedded parser's Spider classification. A false flag on curl or an unknown client is not evidence of a human. Identified spiders deliberately have NULL device/brand fields. A SQL NULL input returns a NULL struct/flag; an empty or unrecognized string returns a struct with unknown string fields and a false flag.

`WHERE NOT is_bot` drops NULL flags along with true flags. The selected view uses `IS DISTINCT FROM true` to retain missing headers without changing their stored values. Browser shares use the selected request denominator, so adding bot signatures can lower other families' shares even when their request counts stay unchanged. Recognition is not verification of what sent the request.

Validation covers the default run (13.6 seconds) and include-bots experiment from a detached notebook directory (2.5 seconds), four rendered charts, eleven SQL invariants plus count/share reconciliation, and three runner failure guards. Including the three bot-signature requests changes selected requests from 15 to 18 and distinct nonempty strings from 6 to 8; the full header-state chart stays identical. Source checks passed: 30 Rust tests, 64 stdio / 68 HTTP / 64 Unix SQL assertions (two cases per transport), Clippy, rustfmt, warning-clean docs and metadata scoring 100/100 L2 with zero findings. Source validation builds the local worker; notebook runs exclusively use the release. Original code, embedded regex data, tests and Cargo files are preserved.

Official Query.Farm Strata Sun and VGI artwork is copied unchanged from the approved assets and linked at the top and bottom.
