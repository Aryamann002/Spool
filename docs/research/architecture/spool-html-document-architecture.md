# Spool — Document Representation Architecture Options

> **This is an options analysis, not a decision and not an implementation specification.** It turns
> `spool-html-document-research.md` into a set of architectures that can be compared, and names the
> trade-offs of each. It does **not** declare a winner.
>
> Evidence labels are as in the research file. `PROPOSED SPOOL DESIGN` marks a shape for discussion.
> `Decision: TBD — requires architecture review.` terminates every option.

---

## 1. What this document is for

The research file answered *"what does the evidence say?"*. This document answers *"what architectures does
that evidence make available, and what does each cost?"* — so that the Spool humans can choose with the
trade-offs on the table rather than with a recommendation disguised as a finding.

**Five options.** A is the incumbent-hypothesis control. B–E are the live HTML/CSS variants. Each is
assessed on the same eleven axes: architecture · advantages · disadvantages · performance implications ·
agent implications · human implications · source-control implications · implementation complexity ·
migration risk · long-term extensibility.

**One thing every option must answer.** The brief's closing question:

> Is a proprietary semantic document model still required?

`INFERENCE` — the honest answer this evidence supports is that all five options need *some* Spool-owned
layer, and that the meaningful variable is its **size and kind** (data vs. rules), not its presence. §12.

---

## 2. Constraints every option inherits

From the settled Spool architecture (research file §3) — these are inputs, not variables:

| # | Constraint | Consequence for any document representation |
|---|---|---|
| K1 | Persistent document state is **type-separated** from editor runtime state | Selection, camera, tool, hover, caret, guides, interaction state **cannot** be persisted. Any option that wants them must fail this constraint. |
| K2 | A typed **semantic operation** system is the single mutation path | Every option needs its documents to be *addressable by stable id* and *mutable by typed operation*. A representation with no addressable unit is unusable. |
| K3 | **Eager interaction commit**; cancel creates no history entry | History records operations, not mutations. So the document format does not need to be undoable at the byte level — only addressable and re-spliceable. This *relaxes* the requirements on the format considerably. |

`SOURCE-CODE FACT` — the prototype already satisfies K2 and K3 as read from `app/src/canvas.rs`:
`CommandOperation` is a closed 5-variant enum (`Geometry`, `Style`, `Text`, `Insert`, `Delete`; `:379-385`);
`History::record` (`:648-680`) normalises by dropping no-op changes and clears the redo stack; `undo`/`redo`
(`:687-745`) pop one `DocumentCommand` and replay its `before`/`after` fields; gesture paths record at
commit (`:1910-1913`, `:1917-1920`), and `Interaction::restore` (`:1007-1019`) touches no history. There is
no mark, no bail, no squash, and no capture-time accumulation anywhere in the prototype — which is K5
satisfied by absence.

`INFERENCE` — this is the single most important input to the whole options analysis: **Spool already has
the operation system that every option here requires, and it has it in the *eager, closed, no-marks* shape
that File 2's Candidate 3 assumes.** The document representation is the open half; the operation half is
not.
| K4 | History is **semantic/operation-oriented** | History entries are operations. Source ranges are therefore *reconstructible at undo time* rather than stored (research file §20.1). |
| K5 | tldraw's capture-time/bail model is **not** to be copied | Rules out designs that make the *format* responsible for history. |

`INFERENCE` — **K3 is the most underrated constraint in the whole evaluation.** Because Spool commits
eagerly at the operation level, the document format never has to be a transactional log. A format only has
to be *spliceable*. That single fact removes the largest historical objection to text-based canonical
documents (that you cannot atomically commit to them) and it removes it **for Spool specifically**.

---

## 3. Option A — Proprietary Spool document model

**The control option.** A typed, closed, Spool-owned schema persisted as JSON (or a binary encoding of the
same).

### Architecture

```
Semantic operation
   ↓
Spool Document (typed nodes, ids, styles, hierarchy)
   ↓ serialise
document.spool.json  ── or ──  document.spool/  (multi-file)
   ↓
Runtime (same shape, no transform)
```

Properties: one record type per concept; ids are first-class fields; style ownership is *implicit* because
there is exactly one place a property can live; constraints, blend groups, component overrides and prototype
connections are all **native**.

### Advantages

- **Style ownership is free.** This is the single largest cost in every HTML/CSS option, and Option A simply
  does not have it (research file §12, §25 row 2).
- **Extensibility is typed and discoverable.** The corpus's `ShapeUtil` finding (`document-model.md`
  Implication C) has a natural home: a trait, added once, serving snapping/culling/traversal/text/rendering.
- **Validation is total.** JSON Schema or a Rust type system can reject every malformed state. Research
  file §12.1 lists validation as JSON's one unambiguous win, and Option A inherits it.
- **Random access and partial I/O** are available via SQLite or a chunked directory (§12.1's strongest
  column).
- **It answers every open corpus question directly** — constraints, variables, components, layout — because
  it is designed to.

### Disadvantages

- **It is the schema an agent must learn.** Research file §18.1 is explicit that this is an *inference*
  about training distribution, not a measurement. `INFERENCE`: the cost is real and unquantified, and §27 of
  the research file proposes the experiment that would measure it.
- **It answers "can I import a website?" with "no, not without a converter."** Research file §14 shows what
  that converter would have to do: normalise a computed DOM result into an authored-looking document. It is
  the same work as HTML import, plus a lossier intermediate.
- **It has no ecosystem.** No tool, no formatter, no linter, no viewer, no diff viewer, no AI prior.
- **Git merge quality depends entirely on layout discipline** (research file §12.2). A pretty-printed,
  one-node-per-line, sorted-key JSON document merges *well*. A compact one merges badly. This is a solvable
  formatting problem, not a format problem — but it is work, and it is invisible until it is done.

### Performance implications

- **Best of the five for runtime.** No parse-to-semantic transform on load beyond deserialisation; no
  cascade engine; no CSS engine.
- **Startup is the worst of the five unless chunked.** Research file §12.1: partial loading is "poor" for
  JSON. A 10⁶-node document is one file. This is Option A's most serious performance weakness and it is why
  Option E exists.
- **Best memory profile** — no duplicate representations.

### Agent implications

- **Semantic operations are trivially expressible**, because the document *is* the operation target. No
  normaliser (research file §18.2) is required for the MCP path.
- **Direct source editing is expensive**: the agent must learn the schema, and an agent's malformed edit is a
  validation failure rather than a recoverable text diff.
- **Context extraction is easy and cheap** — but "give the agent the source of this subtree" means
  serialising a subtree, which is exactly what makes the format proprietary and verbose.

### Human implications

- **Worst readability.** A human opening the project sees schema, not design.
- **No "read it in a text editor and understand it" property.** For a project that positions itself as
  *editable by humans directly through source files*, this is a direct hit on a stated product goal.

### Source-control implications

- **Depends entirely on serialiser layout** (see above). Achievable; not free.
- **No merge drivers exist** for a custom schema. Research file §12.2 notes `git-json-merge` exists; nothing
  equivalent would exist for a Spool schema.

### Implementation complexity

**High.** Everything in the corpus's open questions must be designed and built: layout, constraints,
variables, components, blend groups, prototype. This is the option with the most *design* work and the
least *infrastructure* work.

### Migration risk

**Low in one direction, catastrophic in the other.** Building it is straightforward. Converting away from it
later — after users have authored content and agents have learned it — is the expensive direction. `INFERENCE`:
this is the option that is cheapest to start and most expensive to abandon.

### Long-term extensibility

**Genuinely the best.** A new object type, layout mode, or binding is a Rust trait plus a schema change,
with the type system enforcing exhaustiveness. Nothing in the HTML/CSS options can match this, because none
of them has a registration point (research file §7 L3).

**Decision: TBD — requires architecture review.**

---

## 4. Option B — HTML/CSS canonical, minimal Spool layer

**The hypothesis taken literally.**

### Architecture

```
Semantic operation → source-preserving splice into .html/.css
                                ↓
                     Spool Runtime (compiled from source)
                                ↓
                     History (operations, K3/K4)
```

The layer between them: a parser, a cascade engine, a layout engine, a renderer, and **one** added concept —
`data-spool-id`.

### Advantages

- **Highest human readability of the five.** It is HTML and CSS. (Research file §4, §5.)
- **Highest agent familiarity.** Inference, flagged as such (research file §18.1).
- **Smallest possible Spool layer.** Research file §25's table: exactly one *data* addition (identity) is
  unavoidable; the rest are rules.
- **Gains, not costs, on three capabilities** the corpus lists as open Spool questions: design tokens,
  responsive breakpoints, accessibility (research file §5.2).
- **Developer handoff is free.** The document already *is* the handoff.
- **Merge quality is genuinely better than JSON** for a nameable, mechanistic reason (research file §12.2).

### Disadvantages

- **It requires the cascade engine, and that is the whole problem.** Research file §12: Chrome needs 13
  result channels and a full invalidation-sets subsystem to answer "what caused this pixel". There is no
  off-the-shelf Rust implementation (§26.4). This is Option B's dominant cost and it is not a small one.
- **Style ownership is not free — it is *designed*.** Whichever rule Spool adopts ("always edit an existing
  declaration in the most specific matching rule", "write to the element's own class", "write inline") is a
  product decision with real trade-offs, and every rule has a wrong case. §12 below works this through.
- **The Spool layer is not actually minimal** once (a) style ownership, (b) component semantics, and (c) the
  user-defined-object-type registration problem (research file §7 L3) are counted. Option B's name
  understates its content.
- **No user-defined object types** without Spool building a registration mechanism that HTML does not have.
- **It inherits the web's worst habits**: utility classes, `!important`, specificity escalation, `z-index`
  arms races — all *invited* by the format, and all hostile to direct manipulation.

### Performance implications

- **Runtime is entirely Spool's to build** and therefore can meet the 10⁶ target — but only with the whole
  scene-graph stack (File 3). Option B does not make the performance problem easier; it makes it *possible*.
- **Parse cost is real but bounded** by tree-sitter's incremental reparse (research file §8.1), provided
  storage is chunked.
- **Memory is the worst of the five at load**, because source text and the runtime tree coexist. Research
  file §22.3 quantifies this: ~30 MB of `data-spool-id` for 10⁶ nodes, before the source itself.

### Agent implications

- **All three agent paths (MCP, file edit, script) become the same path**, because they all produce HTML/CSS.
  Research file §18.2 identifies the one hard cell — *file edits must be inverted into operations* — and that
  cell is exactly the cascade engine again.
- **Context extraction is free and excellent**: the agent can `read_file` a component and read exactly what
  a human would. This is Option B's strongest single property.
- **Agent edits are reviewable as diffs by construction.**

### Human implications

- **A human can open the project in any text editor, any browser, any code host, and understand it.**
- **A human can hand the file to an agent.**
- **The reverse risk**: a human can *break* it, and Spool must diagnose the breakage. That is a support cost
  Option A does not have (research file §18.2's normaliser must handle arbitrary broken input).

### Source-control implications

- **Best-in-class diff readability** for declaration-level and element-level edits.
- **A new conflict class**: same-rule edits, and cascade-order-sensitive reordering (research file §20.4).
- **Multi-file shared classes** are a second conflict class. The mitigation is the same one Unreal's One
  File Per Actor implies (research file §20.4): **a shared class must live in exactly one file**.

### Implementation complexity

**Highest.** Not because parsing is hard — tree-sitter handles that — but because three substantial engines
are required and none exists off the shelf: **cascade/provenance**, **layout**, and **rendering**. Plus the
Spool rule layer.

### Migration risk

**Moderate in both directions.** Building it is expensive. Abandoning it means rewriting the agents, the
import, and the rule layer — but **not** migrating user content, because nothing Spool-authored exists yet.
That is the mirror image of Option A's profile and it is the single most important asymmetry in this
document: **Option B is expensive to start and cheap to abandon; Option A is cheap to start and expensive to
abandon.**

### Long-term extensibility

**Weakest of the five**, because there is no registration point for a new object type. This is structural,
not incidental.

**Decision: TBD — requires architecture review.**

---

## 5. Option C — HTML/CSS + minimal Spool metadata (layered)

**Option B plus an explicit, bounded metadata layer**, chosen so that the *expensive* part of B's Spool layer
is not accidental.

### Architecture

```
Semantic operation
   ↓
Spool Layer  ← the design variable of this option
   ├── identity          data-spool-id (research §22.3, option 1)
   ├── style ownership   a convention, declared once (§12)
   ├── components        defn + instance + overrides (research §11, §7)
   ├── prototype         connections (research §5)
   ├── constraints       convention or explicit non-goal (research §7 L1)
   └── object registry   user-defined types (research §7 L3)
   ↓
source-preserving splice → HTML/CSS/SVG on disk
   ↓
Spool Runtime (compiled from source + layer)
```

### Advantages

- **Option B's human and agent properties, unchanged.**
- **The Spool layer becomes explicit and reviewable.** This is the option's real contribution: it converts
  "however much HTML/CSS fails to express" from an unbounded risk into **a list to be decided line by line**.
  Research file §25's table *is* that list.
- **Each item can be accepted, rejected, or deferred** with a stated cost — which is what an architecture
  review needs.
- **The layer is largely *rules*, not data** (§12 of the research file). Rules are cheap to store, cheap to
  diff (they live in one file), and cheap to evolve. Only identity is per-node data.

### Disadvantages

- **Same three engines as B.** The layer does not remove the cascade engine; it adds a *convention* the
  engine must honour.
- **Risk of layer creep.** Every "CSS can't express X" pressure lands here. Without a hard rule about what
  may enter the layer, C converges into A with worse tooling. `PROPOSED SPOOL DESIGN`: the layer's admission
  rule should be §25's question — *can standard HTML/CSS carry it, and if not, is Spool's addition the
  smallest thing that works?*
- **Two sources of truth for style**, if the convention and the source can disagree. This is exactly research
  file §12's difficulty, relocated rather than solved.

### Performance implications

- **Same as B**, plus a small constant per node for the id and a modest cost for the registry lookup.
- **The layer is a good cache-invalidation boundary**: registry changes invalidate the runtime wholesale;
  per-node id changes invalidate nothing.

### Agent implications

- **The layer is machine-readable, which is an advantage**: an agent can be told the convention and can
  reason about ownership without inferring it.
- **It is also a second schema the agent must learn** — the thing Option B hoped to avoid. `INFERENCE`: the
  size of this cost is directly proportional to how disciplined the layer admission rule is.

### Human implications

- **HTML/CSS still dominates what a human reads**; the layer is one file (or one directory) they can read to
  understand Spool's conventions. That is a *good* property: conventions in one place rather than scattered
  through the document.

### Source-control implications

- **Same as B.** If the layer lives in its own file(s), it is also independently mergeable — a small
  genuine improvement over scattering conventions through the document.

### Implementation complexity

**Highest**, equal to B, plus the discipline to hold the layer boundary.

### Migration risk

**Low.** The layer is a small, isolated, well-defined thing. Shrinking it later is easy; the source stays
valid.

### Long-term extensibility

**Better than B**, because the registry is an explicit extension point that Option B lacks — it is just built
by Spool rather than inherited from HTML.

**Decision: TBD — requires architecture review.**

---

## 6. Option D — HTML/CSS canonical + chunked/multi-file project

**Option C plus the storage topology that the scalability target requires.** This is the only option that
addresses research file §15's central finding (tldraw's `maxShapesPerPage: 4000`) as a *storage* question
rather than a rendering one.

### Architecture

```
.spool/
├── manifest.json            chunks, graph, index hashes
├── pages/home.html
├── pages/about.html
├── components/navbar.html
├── components/hero.html
├── styles/tokens.css        custom properties + @property
├── styles/shared.css        classes shared across ≥1 component (ownership rule §12)
├── assets/…                 content-addressed
└── .cache/                  derived, disposable, rebuildable
```

### Advantages

- **Startup becomes tractable.** Manifest → load only visible page's chunks → render → background-load the
  rest. This is research file §16.1's Unreal pattern, verbatim in shape.
- **Merge conflicts fall dramatically** because edits land in different files (research file §16.1: "you do
  not need to check out the Level file from source control to make changes to the Actors in the world").
- **Agent context is bounded by construction** — the agent reads `manifest.json`, then reads the files it
  needs. This is the brief's own requirement.
- **Chunking bounds CST size**, which bounds the `ts_tree_get_changed_ranges` width problem (research file
  §26.3).
- **A shared class lives in exactly one file**, which removes research file §20.4's second conflict class.
- **Watch granularity is per chunk**, which is what makes research file §19's incremental path fast.

### Disadvantages

- **A dependency graph exists and must be maintained.** `components/navbar.html` ↔ `components/hero.html` is
  a cycle risk. research file §11 notes `lightningcss` ships `analyze_dependencies` as a *separate feature*,
  evidence that this is real work.
- **Cascade order across files is semantically load-bearing**, which multi-file makes *more* fragile, not
  less, unless the file order is fixed and declared. `PROPOSED SPOOL DESIGN`: fix and declare the cascade
  order in the manifest; do not let filesystem iteration order decide it.
- **A single logical document is now many files**, so "open the project" is really "open the graph". This
  costs a real amount of UI work that Option A/B do not have.
- **Partial writes are now possible**, which raises the save-race surface (research file §19).

### Performance implications

- **The only option that makes the 10⁶–10⁷ target structurally plausible on load.** §8 of File 3 develops
  this; the summary is that everything else is a rendering problem and this is the *I/O* problem.
- **Worst per-edit locality if chunks are badly chosen.** Research file §10's note applies: chunk boundaries
  must be **semantic** (page, component), not spatial or arbitrary, or a single component's edit straddles
  two chunks.

### Agent implications

- **Best of all five.** `get_project()` returns the manifest; `get_subtree()` returns one file. The agent's
  context is naturally bounded and naturally reviewable.
- **Composes directly with MCP's `resource_link`** (research file §18.3), which already models
  file-addressable project resources.

### Human implications

- **A human sees a normal web project.** Familiar, navigable, and diffable.
- **A human also sees a directory**, and must learn Spool's chunk conventions — a real cost that Option A's
  single file avoids and Option B's single file avoids.

### Source-control implications

- **Best of the five.** Independent files, bounded merges, and the Unreal precedent for the level-file
  contention problem.
- **Cost**: `.cache/` must be gitignored and provably disposable, or it becomes a merge hazard. This requires
  the determinism property research file §8 of File 3 depends on.

### Implementation complexity

**Highest**, but the increment over C is *storage and graph management* — well-understood work — rather than
any new engine.

### Migration risk

**Lowest of the HTML/CSS options.** Chunking can be changed later without changing the document format,
because the format is the same HTML/CSS in all of B, C, and D. D is a **deployment** choice layered over C.

### Long-term extensibility

**Same as C**, plus a natural home for the object registry (one directory).

**Decision: TBD — requires architecture review.**

---

## 7. Option E — Hybrid persistent source + optimised binary cache

**Not a document format choice. A caching strategy that is orthogonal to A–D and can be combined with any of
them.** It is presented as an option because the brief asks for it, and the finding is that it is
**composable rather than competing**.

### Architecture

```
              ┌────────── persistent (authoritative, human- and agent-editable)
              │          HTML/CSS/SVG  ·  or  JSON  ·  or  SQLite
              ↓
        .cache/  derived, disposable, rebuildable, never trusted
          ├── typed style/layout results  (cascade output, layout boxes)
          ├── geometry + bounds
          ├── spatial index shards
          └── asset thumbnails / mip levels
```

Key properties: the cache is **content-addressed by the hash of the inputs that produced it**; a cache
miss is always correct (recompute); a cache *hit* is never trusted without a hash match.

### Advantages

- **Startup improves without touching the document format.** This is its entire purpose and it is the
  highest-leverage move available for the brief's startup goal.
- **Applies to Options A–D equally.** Research file §9's conclusion — compiler-grade vs editor-grade parsers
  are different tools — has an exact analogue here: the persistent format is for editing, the cache is for
  speed, and neither compromises the other.
- **Determinism is the requirement, and it is testable**: same input bytes → same cache bytes. If that
  holds, the cache is safe; if it does not, the cache is a correctness hazard. This is a sharp, checkable
  gate (see §11).
- **Bounded blast radius**: a corrupt or stale cache is discarded, not repaired.

### Disadvantages

- **The cache cannot be trusted blindly, and proving that is ongoing work.** Every derived value must name
  its inputs. This is the same discipline as a build system, and it is easy to get subtly wrong.
- **Two representations exist**, so "what is the document?" has two answers. Discipline is required to keep
  the cache strictly derived.
- **Disk cost.** Derived caches at 10⁶ nodes are not small.
- **Cache invalidation across a chunk graph** (Option D) is a graph problem, not a map problem.

### Performance implications

- **The single largest startup win available**, because it moves the expensive work (cascade, layout,
  geometry, indexing) off the critical path *without* giving up source as truth.
- **Amortises across sessions**, which matters because the brief's stated pain is "open huge file → wait
  forever → editor becomes usable", and that pain is *repeated every open*.

### Agent implications

- **Neutral.** The cache is invisible to agents, and correctly so — an agent must read *source*, never
  cache. `PROPOSED SPOOL DESIGN`: make the cache path unreachable from the MCP surface, so an agent cannot
  read stale derived state.

### Human implications

- **Invisible if it works.** If it is slow, it is invisible until it is missing.

### Source-control implications

- **`.cache/` must be gitignored.** It is derived data with no merge semantics. This is a discipline, not a
  feature.

### Implementation complexity

**Moderate, and highly leveraged.** Cheaper than any of the engines it accelerates.

### Migration risk

**Very low.** Additive. Removable at any time by recomputing.

### Long-term extensibility

**Neutral-to-positive**: it decouples "how fast we open" from "how the document is stored".

**Decision: TBD — requires architecture review.** *(Recommended as composable with, not instead of, A–D —
see §10.)*

---

## 8. Side-by-side comparison

| Axis | A: proprietary | B: HTML/CSS | C: + Spool layer | D: + chunked | E: cache |
|---|---|---|---|---|---|
| Human readability | ✘ | **✔✔** | **✔✔** | **✔✔** | n/a |
| Agent familiarity | ✘ (inference) | **✔✔** | ✔✔ | ✔✔ | n/a |
| Style ownership cost | **✔ free** | ✘✘ | ◐ (convention) | ◐ | n/a |
| Validation | **✔✔** | ◐ (parse only) | ◐ | ◐ | n/a |
| Git diff | ◐ (format-dependent) | **✔** | **✔** | **✔✔** | n/a |
| Git merge | ◐ | ✔ | ✔ | **✔✔** | n/a |
| Partial load / startup | ✘ | ✘ | ✘ | **✔✔** | **✔✔** |
| Runtime perf ceiling | **✔✔** | ✔ (buildable) | ✔ | ✔ | ✔✔ |
| Agent context extraction | ◐ | **✔✔** | **✔✔** | **✔✔** | n/a |
| External file editing | ✘ | **✔✔** | **✔✔** | ✔✔ | n/a |
| Object-type extensibility | **✔✔** | ✘ | ✔ | ✔ | n/a |
| Implementation complexity | high | **highest** | **highest** | **highest** | moderate |
| Migration risk (start) | low | **high** | **high** | **high** | very low |
| Migration risk (abandon) | **high** | low | low | **lowest** | very low |
| Website import | ✘ | ✔ (normalising) | ✔ | ✔ | n/a |

---

## 9. The three findings that actually discriminate

Most of the table above is trade-off, not signal. These three are different in kind.

### Finding 1 — The document format is not the risk; the cascade engine is

`INFERENCE`. Options B, C, and D differ from A in ways that are all *recoverable*. The one cost they share
and cannot avoid is a **cascade/provenance engine**, and research file §12 establishes that this is not a
lookup but an entire subsystem: Chrome's own answer has 13 result channels, an `inherited` ancestor chain,
four dirty bits per DOM node, and a statically-compiled `RuleFeatureSet` of invalidation sets that
conservatively over-approximate blast radius.

`INFERENCE`. So the real architectural question is not *"HTML or JSON?"* but **"will Spool build a CSS
cascade engine, and does it have to?"** Everything else is comparatively cheap. §26.4 of the research file
records that no suitable Rust implementation is known — which makes this the highest-value item to spike.

### Finding 2 — K3 (eager commit) removes the classic objection to text documents

`INFERENCE`. The traditional reason text documents are rejected as canonical design formats is atomicity:
you cannot atomically commit a file. Spool's settled history direction commits **eagerly, per operation,
at the semantic layer** — never mid-gesture, never from the file's point of view. Therefore the document
format only needs to be **spliceable and addressable**, and atomicity is supplied by the operation layer.

`INFERENCE`. This is the single strongest technical argument *for* the HTML/CSS hypothesis that does not
depend on agent familiarity, and it is not a claim any other product in the corpus has had the opportunity
to make, because none of them uses text as canonical source.

### Finding 3 — Option D's chunking is required by the scalability target regardless of format

`INFERENCE`. Research file §15 establishes that the best-documented comparable editor ships a 4,000-shape
page default, against Spool's 10⁶–10⁷ target. Research file §16.1 establishes that the shipping answer to
that scale is *chunked persistent storage with streaming*, and research file §11 establishes that the same
chunking independently fixes agent context size, merge conflicts, and watch granularity. Four independent
axes converge on one structural move.

`INFERENCE`. This means **Option D's topology should be treated as near-mandatory regardless of whether the
format ends up HTML/CSS or proprietary** — which makes it the most transferable finding in this document.

---

## 10. Candidate architectures based on the evidence

`PROPOSED SPOOL DESIGN` — the brief asks for one or more candidates. The evidence supports naming three
compositions, each of which is a *starting point for review*, not a recommendation. Each is stated with the
finding that motivates it and the thing that would falsify it.

### Candidate 1 — "Text-canonical, chunked, cached" (C + D + E)

```
.spool/  manifest + pages/ + components/ + styles/ + assets/ + .cache/
         ↑ source of truth, HTML/CSS/SVG, tree-sitter CSTs
         ↑ Spool layer: identity + style-ownership convention + registry
           runtime: native Rust, own cascade + layout + renderer
           cache: derived, content-addressed, disposable
```

**Motivated by** findings 1–3 plus research file §8 (source preservation is solved), §12.2 (merge), §14
(import), §23 (security is *easier* with a native renderer and no browser).

**Would be falsified by** a spike showing the cascade engine is materially harder than a straight
property-set model — i.e. if research file §26.4 resolves badly.

**Honest cost:** the highest implementation cost of any candidate, concentrated in three engines.

### Candidate 2 — "Proprietary canonical, HTML/CSS as interchange" (A + export/import)

```
document.spool/     typed, validated, operation-native
     ↕                one-way, documented, lossy where it must be
HTML/CSS/SVG         developer handoff, website import source, agent *output* format
```

**Motivated by** finding 1 alone: if Spool does not want to build a cascade engine, it must not make CSS
canonical. It keeps Option A's style ownership, validation, and extensibility; and it satisfies the
brief's *"agents can edit precisely"* goal via operations rather than files, with HTML/CSS as the
**agent-facing output** (research file §13.3).

**Would be falsified by** evidence that agent edit quality on a bespoke schema is materially worse — i.e.
if research file §18.1's inference turns out to be weak.

**Honest cost:** the "editable by humans through source files" product goal is not met by the canonical
format; it is met only by the export.

### Candidate 3 — "Spool layer first, format decided later" (the spike candidate)

```
                 Spool Layer (identity, style ownership, object registry)
                 ─────────────────────────────────────────────────────
        operates over:  a DocumentStore trait
                 ─────────────────────────────────────────────────────
   impl A: proprietary JSON   |   impl C/D: HTML/CSS via tree-sitter
```

**Motivated by** research file §3's finding that Spool *already has* the one thing the HTML/CSS hypothesis
needs and the corpus says is hardest: a **closed, typed semantic operation system** (`CommandOperation`,
5 variants) with **eager commit** and **no marks**. The layer that B/C/D require is largely a *document-side*
concern, and the *operation* side already exists.

**Would be falsified by** discovering that the `DocumentStore` abstraction cannot be made honest — i.e. that
every format's requirements leak into the operation layer, which would mean the format choice cannot be
deferred.

**Honest cost:** the format decision is deferred, not avoided, and the trait may be the wrong shape.

`INFERENCE` — Candidate 3 is the one whose *cost* is lowest and whose *value* is highest, because it buys
information (which implementation is better) rather than a position. It is also the only candidate that
directly serves the brief's stop condition: it is the shortest path to *removing uncertainty*.

**Decision: TBD — requires architecture review.**

---

## 11. Gates that would discriminate between the candidates

`PROPOSED SPOOL DESIGN` — these are cheap, falsifiable checks, ordered by information-per-effort. Each is
designed to be answerable before committing to a format.

| # | Gate | What it tests | Discriminates |
|---|---|---|---|
| G1 | Build a **spike**: 10³-node HTML fixture; parse with tree-sitter; change `gap` on a selected element; assert the file differs by exactly the spliced bytes | research §8's central claim | B/C/D vs. A |
| G2 | Build a **cascade spike**: given a stylesheet and a node, return `(winning declaration, source range, overridden declarations)` for 10⁴ styles | research §12; the largest risk | B/C/D vs. A |
| G3 | Measure `ts_tree_get_changed_ranges` width vs. file size, at 10⁴/10⁵/10⁶ nodes | research §26.3 | D's chunk size |
| G4 | Model comparison: give a model the same change in HTML/CSS vs. in the current `DocumentCommand` JSON; measure validity and diff minimality | research §18.1 | A vs. B/C/D |
| G5 | Import 100 real-world pages; measure the normalisation rate (what fraction of nodes needed authoring invented?) | research §14 | Whether import is a product or a demo |
| G6 | Determinism test: cache in → cache out, byte-identical, across 100 runs with varying hash seeds | Option E's safety property | E's viability |
| G7 | Round-trip: apply 10⁵ random operations; assert source outside splices is byte-identical | research §7 L7 (shorthands) | C/D correctness |

`INFERENCE` — G1, G2 and G4 are the three that matter. G1 and G2 together answer the format question; G4
answers the agent question. All three are days of work, not months.

---

## 12. The style-ownership problem, worked through

Because it is the load-bearing problem for B/C/D and the brief asks for it specifically, here is the actual
tension, not a resolution.

### The situation

```css
.button        { color: red; }
.card .button  { color: blue; }
.theme-dark .button { color: white; }
```

The user selects the rendered blue button and sets `color: green`.

**Sub-question A — which node is selected?** The rendered element. `INFERENCE`: in Spool's model, that is
the DOM node, so this is answerable, and `DOM.getNodeForLocation` (research §15.3) is the contract to match.

**Sub-question B — which declarations are candidates?** All three, plus any inline `style`, plus any value
from a custom property, plus anything from `@layer`/`@scope`/`!important`. Blink's `getMatchedStylesForNode`
answers with `matchedCSSRules`, `inlineStyle`, `attributesStyle`, `pseudoElements`, `inherited`, and
`cssKeyframesRules` as *separate* channels (research §12).

**Sub-question C — was the value inherited?** If `color` is not declared on any matching rule for this node,
the value came from an ancestor — and the CDP answer is the entire ancestor chain (`inherited:
InheritedStyleEntry[]`), each entry carrying that ancestor's own inline style and matched rules.

**Sub-question D — is a variable in play?** `color: var(--brand)`. `INFERENCE`: the computed value is the
resolved colour; the *authored* value is the `var()` reference; `--brand` may be declared anywhere,
including in `:root`. The design tool question is "do I edit the button or the token?", and it has no
answer derivable from the declaration alone.

**Sub-question E — which of the candidates is the winning declaration?** This is the cascade, and it is
what `getMatchedStylesForNode` is *for*. Chrome spends 13 result channels on it. `INFERENCE`: Spool would
need the equivalent of Blink's `RuleFeatureSet` + invalidation sets (research §19), which is the single
largest piece of unbuilt machinery in the whole proposal.

**Sub-question F — should the edit be inline, a new declaration, or an edit of the winner?**

| Option | Upside | Downside |
|---|---|---|
| Edit the winning declaration | Minimal diff; preserves author intent; matches "edit what won" | If the winner is a shared rule (`.card .button`), editing it changes every card. **Wrong.** |
| Add a more-specific rule | Correct scoping | Grows specificity; arms race with the cascade |
| Write to `style=""` | Unambiguous, always wins, trivial to implement | Degrades the source; DevTools-marks it as "inline"; the document becomes ugly fast |
| Write to a per-node class | Correct and readable | Requires minting a class; the original rule still "wins" by specificity and must be beaten |

`INFERENCE`. **None of these four is correct in general**, and the reason is that the design tool's mental
model is *this element has this property* while CSS's is *this rule sets this property wherever it matches*.
That is a semantic mismatch, not an implementation difficulty. Options C and D do not remove it; they add a
convention that picks one of the four behaviours and accepts the wrong cases.

`PROPOSED SPOOL DESIGN` — the convention that minimises surprise, for discussion: **prefer editing the
winning declaration when its selector matches exactly one element; otherwise mint/extend a per-node class
that beats it by specificity; never write inline; always surface the alternative in the inspector.** The
"matches exactly one element" test is itself a selector-match query, so it needs the engine — which is why
this remains gated on G2.

`PROPOSED SPOOL DESIGN` — an inspector affordance that follows directly from the evidence and costs little:
**show all matching declarations with the winner marked and the losers struck through, exactly as DevTools
does.** Chrome has solved the *presentation* of this problem and its shape can be copied even before the
engine exists.

---

## 13. What this document does not decide

- Whether HTML/CSS is canonical. (§1)
- Whether Spool has a cascade engine. (Finding 1; gate G2)
- Whether the Spool layer is admissible, and its size. (§12; research §25)
- Whether constraints exist. (research §7 L1)
- Whether components exist. (corpus `document-model.md` Q11; research §11)
- Whether the format is React/TSX. (research §13: not supported as canonical)
- Which of Candidates 1–3 Spool pursues. (§10)

**Decision: TBD — requires architecture review.**

---

## 14. Sources

All evidence is cited in `spool-html-document-research.md` §28 and is not repeated here. This file adds:

- The settled Spool architecture constraints K1–K5 (§2), from research file §3 and
  `docs/research/architecture/{document-model,history,editor-runtime}.md`.
- The Option A/B/C/D/E option framework requested by the brief.
- The gates G1–G7 (§11), which are `PROPOSED SPOOL DESIGN` and have no external evidence.
- The style-ownership worked example (§12), which is `INFERENCE` over CDP evidence
  (`browser_protocol.json`, `CSS.getMatchedStylesForNode`) and Blink's style-invalidation design document.
