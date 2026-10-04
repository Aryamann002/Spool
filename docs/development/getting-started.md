# Development setup

Use this guide to clone Spool and run either the native app or the website.
They are separate projects, so you can set up just the one you want to work
on. For contribution expectations, see [CONTRIBUTING.md](../../CONTRIBUTING.md).

## Clone the repository

```sh
git clone https://github.com/atpugvaraa/Spool.git
cd Spool
```

## Run the native app

Install a current stable Rust toolchain with [rustup](https://rustup.rs/), then
run the app from its Cargo package directory:

```sh
cd app
cargo run
```

With no project configured, Spool opens a blank starter scene. To open the
included source-backed landing-page fixture instead:

```sh
cd app
SPOOL_PROJECT=./fixtures/landing cargo run
```

The first build can take a while: GPUI is fetched from the Zed repository at
the revision pinned in `app/Cargo.lock` and compiled from source. Network
access is needed the first time. Once dependencies are available locally,
`cargo build --offline` can build without network access. Avoid `cargo update`,
which changes dependency revisions.

### Opening your own project

Set `SPOOL_PROJECT` to a directory containing a `lamine.yaml` file. Project
paths are resolved relative to that directory. A project typically contains
HTML, stylesheets and the Spool structure file:

```text
my-project/
├── lamine.yaml
├── index.html
└── styles.css
```

The `app/fixtures/landing` and `app/fixtures/nested` directories are working
examples. On startup, Spool reports whether the project opened successfully;
if it fails, the reason is printed and the editor falls back to the starter
scene.

## Run the website

The website requires Bun 1.4.2, specified by `packageManager` in
`website/package.json`. Install Bun from [bun.sh](https://bun.sh/), then:

```sh
cd website
bun install
bun run dev
```

Open the local address printed by the development server. The website has its
own contributor instructions in [`website/README.md`](../../website/README.md)
and [`website/AGENTS.md`](../../website/AGENTS.md).

## Verify a change

Run the checks for the project you changed. From `app/`:

```sh
cargo fmt --all -- --check
cargo test
cargo check
cargo clippy --all-targets
```

From `website/`:

```sh
bun run lint
bun run build
```

## If something goes wrong

- **The first app build is slow:** expected; GPUI and its dependencies compile
  from source. Later builds use Cargo's incremental cache.
- **Cargo cannot fetch Zed:** the initial dependency fetch needs network access.
  After it succeeds, Cargo can use its local cache, including for offline
  builds.
- **A project does not open:** check that `SPOOL_PROJECT` points to the
  directory containing `lamine.yaml`, and look for the startup error in the
  terminal.
- **A check fails:** rerun the exact command and include the relevant output
  when asking for help or opening an issue.

## Further reading

- [Contributing to Spool](../../CONTRIBUTING.md)
- [Product and architecture boundaries](../01-product-and-boundaries.md)
- [Implementation roadmap](../04-implementation-roadmap.md)
