# Creative: Animation

Animation is the least-documented domain in this research. Figma is the only product with a documented
animation model, and even there the documentation is spread across prototyping, variables, and a separate
"Motion" product surface.

## What exists

| Product | Animation capability | Evidence |
|---|---|---|
| **Figma** | **Prototype transitions**: Smart Animate, dissolve, instant, move in, slide in, push, with easing and duration. **Figma Motion**: preset animations and **keyframes**, with Timing and Easing variables | DOCUMENTED (prototype actions; variables overview) |
| tldraw | **Animation system for shapes and camera** — explicitly *not* per-shape animation | DOCUMENTED (`sdk-features/animation.mdx`) |
| Affinity | Not documented in pages read | Unknown |
| Canva | "apply animations" is mentioned in third-party summaries of Magic Studio; **no animation model documented** | THIRD-PARTY |

## Figma's model

[Fully DOCUMENTED — "Prototype actions" and "Overview of variables, collections, and modes"]

### Where animation lives

Animation is **prototype metadata**, not a property of objects. It is attached to an *interaction*:

```
Interaction { trigger, action, animation }
```

[FULLY DOCUMENTED structure from "Connect your prototype": "use the Interaction details panel to set the
trigger, action, and animation details"]

[INFERRED] **This is a crucial architectural choice.** Prototype animation is *between two states of the
document*, not *within an object*. It is therefore:
- stored on the **edge** (the connection), not the node,
- evaluated by a **runtime**, not the editor,
- only meaningful in **presentation mode**, not in the editor.

[INFERRED] A different model — per-object keyframe animation — would make animation a first-class document
feature but would also make every renderer an animation engine, every hit test time-dependent, and every
export ambiguous.

### Animatable properties

[DOCUMENTED — Number variables applied to "Preset animations in Figma Motion"]

- Position distance
- Scale amount
- Rotation amount
- Size amount
- Opacity amount

Plus Timing variables (duration in ms) applied to "a preset animation's **delay and duration**", and Easing
variables (easing curve or spring) applied to "animation presets **and keyframes**".

[DOCUMENTED]

[INFERRED] The animatable property set is **exactly the transform-and-opacity set**: position, scale,
rotation, size, opacity. Notably **not** colour, fill, stroke, or typography. This is a strong constraint —
it tells us that a transition system can be implemented entirely as a **transform + opacity interpolation
over a pair of geometry states**, without needing per-property tweening infrastructure.

### Smart Animate

[Referenced repeatedly in DOCUMENTED help articles]

- "Smart Animate transitions … automatically animate between two frames by matching layer names."
  [THIRD-PARTY phrasing; Figma docs reference the feature and its constraints]
- Documented constraint: "**Smart Animate transitions do not take into account the background of a frame.** If
  you want to use a Slide in or Move in transition with Smart Animate, you will need to add a background."
  [DOCUMENTED — "Guide to auto layout", prototyping considerations]
- Smart Animate can be used with variants, and with the "Change to" action for interactive components.

[INFERRED] **Matching layer names** is how Smart Animate pairs objects across two frames. That means object
**names are semantically load-bearing** for animation. It also means renaming a layer can silently break a
transition — a documented-in-practice footgun that community sources discuss. [THIRD-PARTY]

### Presentation mode

- Presentation mode exists; long frames scroll by default.
- "To apply scrolling overflow to a frame, you need to have content to extend beyond the frame's bounds."
- Overflow behaviour is set on a frame.
- "Presentation view supports scrolling of long frames by default. You will only need to use this workaround
  when you want to clip content." [DOCUMENTED]

### Prototype actions with animation-adjacent semantics

[DOCUMENTED — "Prototype actions"]

| Action | Animation relevance |
|---|---|
| Navigate to | The transition applies here |
| Back | Returns to previous screen; **"Using Swap overlay won't add that frame to the prototype's history"** |
| Open / Close / Swap overlay | Overlay settings are retained across swaps |
| Scroll to | "you can set the scroll animation to be **Instant** or set an ease using the Animate option" |
| Conditional | if/else over variables |
| Set variable / Set variable mode | Runtime state |
| Change to | Switch variant in interactive components |
| Play/pause/mute video; Set to specific time; Jump forward/backward in time | Media timeline control |

[INFERRED] The action list reveals a small runtime with: a **navigation stack** (Back), **overlay
stacking**, **scroll position**, **variable state**, **variant state**, and **media time**. That is more than
"links between screens" — it is a prototype runtime with state.

## tldraw's model

[DOCUMENTED — `sdk-features/animation.mdx`, `sdk-features/performance.mdx`]

> "The [Animation](?) article covers the editor's animation system. It handles **camera movement and
> occasional shape transitions**. It's not designed for continuous per-shape animation."

[DOCUMENTED guidance for custom shapes]

> "Avoid shape animations. Animating shape properties causes continuous re-renders. A spinning shape
> triggers updates every frame. If you have many shapes or complex rendering, this adds up quickly. If you
> need animation, use **CSS animations for purely visual effects that don't change shape data**, use a canvas
> for particle systems or complex effects, and **keep the number of concurrently animating shapes small**."

And in the culling docs:

> "Reasons to disable culling include shapes with visual effects (shadows, glows) that extend beyond their
> bounds, shapes running animations that should continue off-screen."

[DOCUMENTED]

[INFERRED] **tldraw's stance is that continuous animation is a performance liability, not a feature.** Its
animation system exists to make *editor transitions* smooth (camera, selection, shape creation), not to
author content animation. This is a coherent and defensible position for a whiteboard tool, and it is
worth noting that it is a **different product category** from Figma's prototyping.

## Non-animation "motion" in the other products

| Product | Transition-ish features |
|---|---|
| Affinity | Studio switching is instant or animated (implementation **Unknown**); presentation/stack view (**not confirmed**) |
| Canva | Page transitions in presentation mode (**not documented in the pages read**); Magic Studio "apply animations" [THIRD-PARTY] |
| tldraw | Camera animation; shape transitions |

## Spool prototype: what exists

From `app/src/shell.rs` [OBSERVED in source]:

- `prototype_inspector()` — a **visual panel only**. No prototype data model, no interactions, no
  transitions.
- `share_popover()` — visual only.

There is **no animation concept at all**: no tween, no easing, no keyframes, no timeline, no transitions.

[INFERRED] Animation is entirely ahead of the prototype, and its absence is consistent with the absence of
instances, bindings, and variable modes — prototype animation depends on all three (variants → transitions;
variables → conditional animation; bindings → connections).

## What a prototype layer requires

Derived from Figma's documented action and animation lists. [INFERRED]

```
Flow {
    starting_points: [Frame]
}

Interaction {                              // stored on a node, or on a connection?
    node: ObjectId
    trigger: Trigger
    action: Action
    animation: Animation?
}

Trigger = OnClick | OnHover | OnMouseEnter | OnMouseLeave
        | OnMouseDown | OnMouseUp | AfterDelay | WhilePressingKeys
        | WhileDragging | OnScroll | OnMediaEnd

Action = NavigateTo(frame) | Back | ScrollTo(target, behaviour)
       | OpenOverlay(frame, settings) | CloseOverlay | SwapOverlay(frame)
       | OpenLink(url)
       | SetVariable(name, value) | SetVariableMode(collection, mode)
       | Conditional(conditions, then: Action, else: Action?)
       | ChangeTo(variant)
       | Video(Play | Pause | Toggle | Mute | Unmute | Seek | Jump)

Animation = { kind, duration_ms, easing, delay_ms }
Animation.kind = SmartAnimate | Dissolve | Instant | MoveIn | SlideIn | Push
```

[INFERRED] Note that this needs **bindings** (connections between frames), **variables with modes**
(conditional state), **variants** (Change to), and a **runtime**. Three of the four are document features
Spool's prototype lacks.

## Candidate architectural implication

**Evidence:**

1. **Prototype animation lives on the edge, not the node.** [Figma DOCUMENTED — trigger/action/animation is
   an interaction record]
2. The animatable property set is **transform + opacity only**. [Figma DOCUMENTED via variable targets]
3. **Smart Animate pairs layers by name.** [INFERRED from Figma's documented behaviour + THIRD-PARTY]
4. The prototype layer requires a small **runtime with state**: navigation stack, overlay stack, scroll
   position, variable state, variant state, media time. [Figma DOCUMENTED via the action list]
5. **tldraw treats continuous animation as a performance liability** and deliberately scopes its animation
   system to editor transitions. [tldraw DOCUMENTED]
6. Animation with **effects extending past bounds** is a documented culling exception. [tldraw DOCUMENTED]
7. No product documents a per-object keyframe animation model in the editor. [Documented gap]

**Why it matters:** Animation is easy to defer, but the *decision* about where it lives is not. If animation
is per-node keyframes, the renderer becomes time-dependent and export becomes ambiguous. If it is per-edge
transitions, the document stays static and a separate runtime animates it.

**Potential Spool approaches:**

- **A. No animation. Static documents.** Fine for a design tool; no prototyping.
- **B. A presentation mode** that steps between pages/frames with a simple cross-fade. Minimal; useful for
  "show the flow".
- **C. B + a prototype graph** (flows + connections + triggers + navigate/overlay/back actions). Figma-like
  navigation prototyping.
- **D. C + transitions** (dissolve/move/slide/push with duration and easing). Adds an interpolator over
  transform+opacity.
- **E. D + Smart Animate** (name-matched object pairing across two frames).
- **F. C + variables + conditionals + variants.** The full interactive-components model.
- **G. Per-object keyframes.** A different category entirely.

**Tradeoffs:** A→B→C is a coherent ladder. D adds a real interpolator. E requires stable names and a
diffing pass. F requires variables and instances. G is a fundamentally different (and much larger)
commitment.

[INFERRED] **The important decision is C/D vs. G**, and it should be made early because it constrains
whether object names are load-bearing, whether the renderer is time-parameterised, and whether export must
handle animated states.

[INFERRED] Figma's evidence strongly favours **C/D over G**: it keeps the document static, keeps the
renderer simple, keeps export unambiguous, and gives 90% of the prototyping value. Spool should at least
understand why before choosing.

**Decision: TBD — requires architecture review.** Strong recommendation to *not* choose G.

## Open questions

1. Is animation per-edge (transitions) or per-node (keyframes)?
2. Are object names load-bearing (Smart Animate matching)?
3. Is there a presentation mode, and is it a separate runtime?
4. Does the prototype graph require flows, or is the whole document a graph?
5. What is the runtime model — a stack, a state machine, or a script?
6. Do overlays participate in the navigation history?
7. Does export handle animated states?

## Sources

- Figma: "Guide to prototyping in Figma" (/360040314193); "Prototype actions" (/360040035874) ⭐⭐;
  "Connect your prototype" (/360040315773) ⭐ (trigger/action/animation structure); "Guide to auto layout"
  (/360040451373) (Smart Animate background constraint, scrolling overflow); "Overview of variables,
  collections, and modes" (/14506821864087) ⭐ (Motion animatable properties, Timing, Easing, keyframes);
  "Create and use variants" (/360056440594) (Change to, interactive components)
- tldraw: `sdk-features/animation.mdx` ⭐, `sdk-features/performance.mdx` ⭐ ("Avoid shape animations",
  canCull exceptions), `sdk-features/ticks.mdx`, `sdk-features/culling.mdx`
- Affinity: "About Studios" (/workspace-about-studios/) (no animation model documented)
- Canva: "Canva keyboard shortcuts" (/canva-keyboard-shortcuts/) (presentation mode `⌥⌘P`); third-party
  summaries of Magic Studio animations — THIRD-PARTY
- Spool prototype: `app/src/shell.rs` (`prototype_inspector`, `share_popover`)
