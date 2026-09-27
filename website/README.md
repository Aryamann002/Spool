# Spool

> Your next open-source local design agent.

Spool is a local design agent powered by small language models. This directory contains its interactive website.

## Vision

The website is a physical creative canvas. The visitor begins in a void; a paper/fabric canvas forms; temporary fixed-position puzzle silhouettes reveal the actual landing-page world underneath; the pieces leave; then (in a later phase) black thread draws the Spool wordmark before the camera pulls back to the larger world.

## Project guidance

- `AGENTS.md` — engineering and agent constraints
- `DESIGN.md` — visual and interaction direction
- `PLAN.md` — implementation phases
- `.agents/skills/emil-design-eng/SKILL.md` — Emil's general design-engineering philosophy, copied from the global skill

The existing visual references are in `public/images/`. A candidate Spool wordmark font is at `public/fonts/wobble.ttf`; verify it against the concept board before treating it as the exact logo typeface. Do not approximate the wordmark.

## Development

This project uses Bun (`bun.lock` and the `packageManager` field in `package.json`).

```bash
bun install
bun run dev
```

Open [http://localhost:3000](http://localhost:3000) to view the site. Use `bun run lint` and `bun run build` for validation.

## Philosophy

Spool should demonstrate its own capabilities through its website. The website is itself a design experiment.
