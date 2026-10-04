# Spool

**A native design editor for building real interfaces, with local AI as the
long-term vision.**

Spool is a student-built, open-source design tool from Google Developer Groups
at Thapar Institute of Engineering and Technology. It brings visual editing
closer to the code that makes an interface real: your design is authored in
HTML, CSS and SVG, and Spool gives you a native canvas for working with it.

We think the work you make in a design tool should stay understandable and
editable outside that tool. In Spool, the source files remain yours to inspect,
version and build on. The longer-term idea is to bring private, local AI into
that structured design workflow, where it can work with the design itself
rather than return a picture of one.

Spool is early and actively being built. The editor foundation exists; the AI
direction is a goal, not a feature in the app today.

## What works today

- A native Rust and GPUI editor with Canvas, Layers and Inspector panels.
- Selection, multi-selection, marquee, move, resize, duplicate, delete and text
  editing, with alignment snapping and undo/redo.
- Projects made from HTML, CSS, SVG and a `lamine.yaml` structure file.
- A bounded CSS and layout model, plus source-preserving edits for supported
  changes.

This is a developing editor, not a finished replacement for existing design
tools. CSS support is intentionally limited; flexbox, inline layout, text
wrapping and several other browser behaviors are not implemented. Creating or
deleting authored objects is not yet supported on save. See the
[implementation roadmap](docs/04-implementation-roadmap.md) for current
priorities.

## Try it

### Native app

Install a current stable Rust toolchain and use a platform supported by GPUI.
From the repository root:

```sh
cd app
cargo run
```

To open the included source-backed example:

```sh
cd app
SPOOL_PROJECT=./fixtures/landing cargo run
```

The first build downloads and compiles GPUI from the pinned Zed source, so it
can take a while and needs network access. See the
[app setup guide](docs/development/getting-started.md) for details.

### Website

The website is a separate Next.js project. It requires Bun 1.4.2, pinned in
`website/package.json`:

```sh
cd website
bun install
bun run dev
```

## How Spool is put together

Authored HTML, CSS and SVG are the design. `lamine.yaml` records Spool-specific
identity, hierarchy and source bindings; it is not a second copy of the visual
properties. The native runtime interprets those sources for editing and
rendering, and can be rebuilt from them.

```text
HTML / CSS / SVG + lamine.yaml
              ↓
       native editor runtime
              ↓
       visual editing
              ↓
 supported edits written back to source
```

The architecture and its boundaries are documented in
[Product and Boundaries](docs/01-product-and-boundaries.md),
[Document and Source Model](docs/02-document-and-source-model.md), and
[Editor Runtime and History](docs/03-editor-runtime-history.md).

## Contributing

Spool is built by students, and contributors are welcome on both the native app
and the website. You don't need to know the whole codebase before making a
useful change. Start with [CONTRIBUTING.md](CONTRIBUTING.md), then use the
[development setup guide](docs/development/getting-started.md) to run the part
you want to work on.

## More to explore

- [Implementation roadmap](docs/04-implementation-roadmap.md)
- [Research notes](docs/research/README.md)
- [App development setup](docs/development/getting-started.md)

## License

This repository does not currently include a license. Until one is added, no
license should be assumed.
