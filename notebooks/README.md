# User-Agent examples

[01 · Count requests without inventing identities](01_count_requests_without_inventing_identities.ipynb)
uses 18 synthetic requests built from ten authored header patterns. It compares
requests with distinct nonempty header strings, preserves missing and unknown
clients, and tests an explicit bot-signature policy. A field comparison shows when
to use `ua_parse` or the seven individual selectors, including browser/OS versions,
device/brand availability and NULL-safe agreement. No input comes from real users.

[Open in Colab](https://colab.research.google.com/github/Query-farm/vgi-useragent/blob/main/notebooks/01_count_requests_without_inventing_identities.ipynb),
run the install cell, then **Run all**. The notebook contains its synthetic inputs;
no clone or companion download is required. The install uses the base shared package
and preserves compatible Colab plotting/data libraries. Hosted Colab itself has not
been tested; a fresh Python 3.12 notebook-only execution and a preloaded-library
compatibility check are recorded in validation evidence.

For a local run, install Python 3.11+ and [uv](https://docs.astral.sh/uv/), then:

```sh
cd notebooks
uv sync --locked --extra runner
uv run --locked --extra runner vgi-notebooks
```

The optional `runner` extra supplies the local execution kernel. To register an
interactive kernel, run `uv run --locked --extra runner python -m ipykernel install
--user --name vgi-useragent-notebooks --display-name 'VGI User-Agent notebooks'`,
select it in Jupyter, and restart/run all after changing a parameter.

The visible SQL attaches the published native worker:

```sql
INSTALL vgi FROM community;
LOAD vgi;
ATTACH 'useragent' (TYPE vgi,
  LOCATION 'github-auto://Query-farm/vgi-useragent@v0.4.0');
```

First startup fetches the platform binary and verifies its release checksum. The
regex database is embedded; parsing itself is local, with no Rust installation,
account, token or service. This example is validated on macOS Apple silicon. The
shared connection creates an empty temporary SQL-secret directory, disables
persistent secrets and closes its database and worker at the end.

Change only `EXCLUDE_IDENTIFIED_BOTS` to include known spider signatures:

```sh
uv run --locked --extra runner vgi-notebooks \
  --parameters '{"EXCLUDE_IDENTIFIED_BOTS": false}' \
  --output-dir validation/include-bots
```

The selected population changes from 15 to 18 requests and from six to eight
distinct nonempty strings. The full-input chart and selector comparison stay
unchanged; field availability keeps all selected requests in its denominator.
Unknown/missing flags survive either policy. None of these counts identifies
people, sessions or verified hardware; a false bot flag does not establish a human.

The shared [standard](https://github.com/Query-farm/vgi-example-notebooks/blob/v0.3.6/docs/standard.md)
and runner are pinned at **v0.3.6** rather than copied into this repository. Charts
retain native SVG and content-addressed `charts/` assets for clean GitHub display.
Experiments retain native SVG in separate copies. Failed execution and invalid
parameter overrides cannot replace the last successful default. Keep prior chart
assets so historical notebook image references remain valid.

[Coverage evidence](coverage.json) records eight of eight public SQL surfaces for
both release v0.4.0 and the reviewed current-public source revision. Complete
`duckdb_functions()` / `duckdb_tables()` exports, notebook execution hashes and
explicit narrative reviews are under `coverage/`. The notebook executes the
published release; current-source catalog discovery is a separate validation step.
No authenticated or write surfaces are omitted. New worker releases require a new
catalog comparison and notebook execution before updating this pin or coverage.
