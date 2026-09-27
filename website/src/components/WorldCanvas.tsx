"use client";

import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { anchorPosition, SCENE_ANCHORS, type SceneAnchor, type WorldRegion } from "./composition";
import { WORDMARK_GLYPHS, WORDMARK_LETTER_DURATION_MS, WORDMARK_LETTER_STAGGER_MS, WORDMARK_TOTAL_ADVANCE, WORDMARK_UNITS_PER_EM } from "./wordmark-paths";

export type Camera = { x: number; y: number; scale: number };

type WorldPhase =
  | "void"
  | "canvas"
  | "puzzle-reveal"
  | "puzzle-complete"
  | "puzzle-exit"
  | "ready-for-thread"
  | "thread"
  | "exploring";

type WorldMetrics = {
  width: number;
  height: number;
  worldWidth: number;
  worldHeight: number;
  contentBounds: { left: number; top: number; right: number; bottom: number };
};

const WORLD_SCALE = 1;
const CONTENT_EDGE_ALLOWANCE = 0.2;
const CAMERA_TRAVEL_MS = 780;
const GITHUB_URL = "https://github.com/atpugvaraa/spool";
const DOCS_URL: string | null = null;
const DOWNLOAD_URL: string | null = null;

function DownloadAction({ className, icon = false }: { className: string; icon?: boolean }) {
  const content = <>{icon && <span>↓</span>} Download</>;
  if (DOWNLOAD_URL) {
    return <a className={className} href={DOWNLOAD_URL} download>{content}</a>;
  }

  return (
    <button className={className} type="button" disabled title="A product download is not available yet">
      {content}
    </button>
  );
}

function clampCamera(camera: Camera, metrics: WorldMetrics): Camera {
  const horizontalAllowance = metrics.width * CONTENT_EDGE_ALLOWANCE;
  const verticalAllowance = metrics.height * CONTENT_EDGE_ALLOWANCE;
  const minX = Math.max(
    metrics.width - metrics.worldWidth,
    metrics.width - metrics.contentBounds.right - horizontalAllowance,
  );
  const maxX = Math.min(0, -metrics.contentBounds.left + horizontalAllowance);
  const minY = Math.max(
    metrics.height - metrics.worldHeight,
    metrics.height - metrics.contentBounds.bottom - verticalAllowance,
  );
  const maxY = Math.min(0, -metrics.contentBounds.top + verticalAllowance);

  return {
    ...camera,
    x: Math.max(minX, Math.min(maxX, camera.x)),
    y: Math.max(minY, Math.min(maxY, camera.y)),
    scale: WORLD_SCALE,
  };
}

function centeredCamera(x: number, y: number, metrics: WorldMetrics): Camera {
  return clampCamera({
    x: metrics.width / 2 - x * WORLD_SCALE,
    y: metrics.height / 2 - y * WORLD_SCALE,
    scale: WORLD_SCALE,
  }, metrics);
}

const WORDMARK_CENTER = 350;
const WORDMARK_BASELINE = 223;

type ItemStyle = CSSProperties & {
  "--x": string;
  "--y": string;
  "--width": string;
  "--rotation": string;
};

type WordmarkStyle = ItemStyle & {
  "--letter-duration": string;
};

function itemStyle(x: number, y: number, width: number, rotation = 0): ItemStyle {
  return {
    "--x": `${x}%`,
    "--y": `${y}%`,
    "--width": `${width}px`,
    "--rotation": `${rotation}deg`,
  };
}

function anchoredStyle(region: WorldRegion, width: number) {
  const { x, y, rotation } = anchorPosition(region);
  return itemStyle(x, y, width, rotation);
}

function wordmarkStyle(): WordmarkStyle {
  return {
    ...anchoredStyle("wordmark", 700),
    "--letter-duration": `${WORDMARK_LETTER_DURATION_MS}ms`,
  };
}

export default function WorldCanvas({ phase }: { phase: WorldPhase }) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const worldRef = useRef<HTMLDivElement>(null);
  const artifactsRef = useRef<HTMLDivElement>(null);
  const wordmarkRef = useRef<SVGSVGElement>(null);
  const wordmarkGeometryRef = useRef<SVGGElement>(null);
  const metricsRef = useRef<WorldMetrics>({
    width: 0,
    height: 0,
    worldWidth: 0,
    worldHeight: 0,
    contentBounds: { left: 0, top: 0, right: 0, bottom: 0 },
  });
  const cameraRef = useRef<Camera>({ x: 0, y: 0, scale: 1 });
  const pointerRef = useRef<{ id: number; x: number; y: number } | null>(null);
  const navigationTimerRef = useRef<number | null>(null);
  const mobileNavToggleRef = useRef<HTMLButtonElement>(null);
  const navigationActive = phase === "exploring";
  const [dragging, setDragging] = useState(false);
  const [activeAnchor, setActiveAnchor] = useState<SceneAnchor | null>("home");
  const [mobileNavOpen, setMobileNavOpen] = useState(false);
  const wordmarkRevealStarted = phase === "thread" || phase === "exploring";

  useLayoutEffect(() => {
    const wordmark = wordmarkRef.current;
    if (!wordmark) return;

    const updateWordmarkGeometry = () => {
      const fontSize = Number.parseFloat(window.getComputedStyle(wordmark).fontSize);
      const scale = fontSize / WORDMARK_UNITS_PER_EM;
      const transform = `translate(${WORDMARK_CENTER} ${WORDMARK_BASELINE}) scale(${scale} ${scale}) translate(${-WORDMARK_TOTAL_ADVANCE / 2} 0)`;
      wordmarkGeometryRef.current?.setAttribute("transform", transform);
    };

    updateWordmarkGeometry();
    window.addEventListener("resize", updateWordmarkGeometry);
    return () => window.removeEventListener("resize", updateWordmarkGeometry);
  }, []);

  const writeCamera = useCallback((camera: Camera) => {
    cameraRef.current = camera;
    if (worldRef.current) {
      worldRef.current.style.transform = `translate3d(${camera.x}px, ${camera.y}px, 0) scale(1)`;
    }
  }, []);

  const navigateToAnchor = useCallback((anchor: SceneAnchor) => {
    const metrics = metricsRef.current;
    const world = worldRef.current;
    if (!world || !metrics.width || !metrics.height) return;

    const compact = metrics.width <= 720;
    const { x, y } = anchorPosition(SCENE_ANCHORS[anchor], compact);
    const worldOriginX = (metrics.worldWidth - metrics.width) / 2;
    const worldOriginY = (metrics.worldHeight - metrics.height) / 2;
    const target = centeredCamera(
      worldOriginX + (x / 100) * metrics.width,
      worldOriginY + (y / 100) * metrics.height,
      metrics,
    );

    if (navigationTimerRef.current) window.clearTimeout(navigationTimerRef.current);
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const duration = reducedMotion ? 100 : CAMERA_TRAVEL_MS;
    world.style.transition = `transform ${duration}ms var(--${reducedMotion ? "ease-out" : "ease-in-out"})`;
    writeCamera(target);
    setActiveAnchor(anchor);
    navigationTimerRef.current = window.setTimeout(() => {
      if (worldRef.current) worldRef.current.style.transition = "none";
      navigationTimerRef.current = null;
    }, duration + 50);
  }, [writeCamera]);

  const finishCameraTransition = useCallback(() => {
    if (navigationTimerRef.current) window.clearTimeout(navigationTimerRef.current);
    navigationTimerRef.current = null;
    if (worldRef.current) worldRef.current.style.transition = "none";
  }, []);

  useEffect(() => () => {
    if (navigationTimerRef.current) window.clearTimeout(navigationTimerRef.current);
  }, []);

  useEffect(() => {
    if (!mobileNavOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setMobileNavOpen(false);
      mobileNavToggleRef.current?.focus();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [mobileNavOpen]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;

    const measure = () => {
      const bounds = viewport.getBoundingClientRect();
      if (!bounds.width || !bounds.height) return;

      const world = worldRef.current;
      const artifacts = artifactsRef.current;
      if (!world || !artifacts) return;

      const current = cameraRef.current;
      const itemBounds = Array.from(artifacts.querySelectorAll<HTMLElement>(".world-item"))
        .map((element) => element.getBoundingClientRect());
      if (!itemBounds.length) return;

      const metrics: WorldMetrics = {
        width: bounds.width,
        height: bounds.height,
        worldWidth: world.offsetWidth,
        worldHeight: world.offsetHeight,
        contentBounds: {
          left: Math.min(...itemBounds.map((rect) => rect.left - current.x)),
          top: Math.min(...itemBounds.map((rect) => rect.top - current.y)),
          right: Math.max(...itemBounds.map((rect) => rect.right - current.x)),
          bottom: Math.max(...itemBounds.map((rect) => rect.bottom - current.y)),
        },
      };
      const previous = metricsRef.current;
      metricsRef.current = metrics;

      const nextCamera = !previous.width || !previous.height
        ? centeredCamera(metrics.worldWidth / 2, metrics.worldHeight / 2, metrics)
        : centeredCamera(
          previous.width / 2 - current.x,
          previous.height / 2 - current.y,
          metrics,
        );
      if (world.style.transition) finishCameraTransition();
      writeCamera(nextCamera);
    };

    const observer = new ResizeObserver(measure);
    observer.observe(viewport);
    measure();
    return () => observer.disconnect();
  }, [finishCameraTransition, writeCamera]);

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!navigationActive || event.button !== 0) return;
    if (event.target instanceof Element && event.target.closest("button, a")) return;

    const world = worldRef.current;
    if (world && navigationTimerRef.current) {
      const renderedTransform = window.getComputedStyle(world).transform;
      const renderedMatrix = new DOMMatrixReadOnly(renderedTransform === "none" ? undefined : renderedTransform);
      finishCameraTransition();
      writeCamera(clampCamera({
        ...cameraRef.current,
        x: renderedMatrix.m41,
        y: renderedMatrix.m42,
      }, metricsRef.current));
      setActiveAnchor(null);
    } else {
      setActiveAnchor(null);
    }

    pointerRef.current = { id: event.pointerId, x: event.clientX, y: event.clientY };
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
    setDragging(true);
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const pointer = pointerRef.current;
    if (!pointer || pointer.id !== event.pointerId) return;

    const metrics = metricsRef.current;
    const camera = cameraRef.current;
    const nextCamera = clampCamera({
      ...camera,
      x: camera.x + event.clientX - pointer.x,
      y: camera.y + event.clientY - pointer.y,
    }, metrics);
    pointerRef.current = { ...pointer, x: event.clientX, y: event.clientY };
    writeCamera(nextCamera);
  };

  const endPointerDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    const pointer = pointerRef.current;
    if (!pointer || pointer.id !== event.pointerId) return;

    pointerRef.current = null;
    setDragging(false);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  };


  return (
    <>
      <div
        className="world-camera-viewport"
        ref={viewportRef}
        data-interactive={navigationActive}
        data-dragging={dragging || undefined}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={endPointerDrag}
        onPointerCancel={endPointerDrag}
        onLostPointerCapture={endPointerDrag}
        aria-label="Draggable Spool creative world"
      >
        <div className="world-coordinate-space" data-world-coordinate-space ref={worldRef}>
          <div className="world-artifacts" ref={artifactsRef}>

        <figure className="world-item opnest-board" style={anchoredStyle("opnest", 354)} aria-label="A generated website with a landscape image">
          <div className="opnest-board__image"><span>opnest</span><small>your calmer internet.</small></div>
          <div className="opnest-board__meta"><b>Spool</b><span>Travel / Field notes / Journal</span><span>Design a quieter kind of web.</span></div>
        </figure>
        <article className="world-item mini-browser" style={anchoredStyle("miniBrowser", 156)} aria-label="A generated landscape website preview">
          <div className="mini-browser__image" />
          <div className="mini-browser__caption"><b>A calmer internet.</b><span>01 / 04</span></div>
        </article>
        <aside className="world-item pencil-note pencil-note--generated" style={itemStyle(5, 22, 112, -12)}>
          Generated<br />with Spool <span>↘</span>
        </aside>
        <article className="world-item swatch-note" style={anchoredStyle("palette", 132)} aria-label="Color swatches and prompt annotation">
          <div className="swatch-note__chips"><i /><i /><i /><i /></div>
          <p>From a simple<br />prompt...</p>
        </article>
        <div className="world-item paper-sketch" style={anchoredStyle("paperSketch", 116)} aria-label="A pencil sketch on a paper fragment">
          <span>✳</span><svg viewBox="0 0 100 60" aria-hidden="true"><path d="M19 18 52 8l27 10-29 12-31-12Zm0 0v23l31 12V30m29-12v22L50 53m7-31 7 3m-15 4 6 2" /></svg>
        </div>
        <figure className="world-item eye-study" style={anchoredStyle("eyeStudy", 178)} aria-label="A photographic eye study">
          <div className="eye-study__photo" />
          <figcaption>image study / light &amp; looking</figcaption>
        </figure>
        <article className="world-item dark-site dark-site--thoughtful" style={anchoredStyle("darkThoughtful", 220)} aria-label="A dark generated website preview">
          <div className="dark-site__bar"><b>Spool</b><span>Projects&nbsp;&nbsp; Studio&nbsp;&nbsp; About</span></div>
          <div className="dark-site__content"><strong>A more<br />thoughtful<br />web.</strong><div className="dark-site__landscape" /></div>
          <small>DESIGN / DEVELOP / SHARE</small>
        </article>
        <aside className="world-item exploration-index" style={itemStyle(96, 31, 104, 0)} aria-label="Exploration index">
          <b>+ explorations</b><span>01 / websites</span><span>02 / posters</span><span>03 / UI concepts</span><span>04 / illustrations</span><span>05 / presentations</span><span>06 / brands</span><span>07 / experiments</span>
        </aside>
        <aside className="world-item pencil-note pencil-note--imagine" style={itemStyle(73, 9, 192, -8)}>
          Websites.<br />Posters. UI.<br />Whatever you imagine. <span>↙</span>
        </aside>

        <figure className="world-item image-stack image-stack--left" style={anchoredStyle("leftImageStack", 176)} aria-label="Overlapping landscape and design photographs">
          <div className="image-stack__back" /><div className="image-stack__middle" /><div className="image-stack__front" />
        </figure>
        <div className="world-item japanese-poster" style={anchoredStyle("japanesePoster", 70)} aria-label="A vertical Japanese typography poster">
          <span>静<br />け<br />さ</span>
        </div>
        <div className="world-item black-sketch" style={anchoredStyle("blackSketch", 112)} aria-label="A chalk line illustration on black paper">
          <svg viewBox="0 0 120 84" aria-hidden="true"><path d="m8 65 22-34 7 29 24-43 3 34 28-25-13 41 27-7M12 22l40 49m12-66L29 76m44-54 28 31" /></svg>
        </div>
        <article className="world-item side-copy" style={itemStyle(13, 46, 124, -2)}>
          <span>IDEAS</span><span>LAYOUTS</span><span>COMPONENTS</span><span>IN SECONDS</span><b>↘</b>
        </article>

        <section className="world-item hero-copy" style={anchoredStyle("hero", 700)} aria-label="Spool, your next open-source local design agent">
          <h1>your next open-source<br />local design agent</h1>
          <p>Drive smart and tasteful local agents for all your design needs.</p>
          <DownloadAction className="hero-download" icon />
          <a
            className="hero-code"
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer"
            aria-disabled={!navigationActive || undefined}
            tabIndex={navigationActive ? 0 : -1}
            onClick={(event) => { if (!navigationActive) event.preventDefault(); }}
          >
            Steal our code (legally) <b>⟶</b>
          </a>
        </section>

        <figure className="world-item dark-site dark-site--human" style={anchoredStyle("humanSite", 260)} aria-label="A generated editorial website about human interfaces">
          <div className="dark-site__bar"><b>/ studio</b><span>Work&nbsp;&nbsp; About&nbsp;&nbsp; Contact</span></div>
          <div className="dark-site__content"><strong>Make<br />interfaces<br />feel human<br />again.</strong><div className="flower-photo" /></div>
        </figure>
        <article className="world-item thought-card" style={anchoredStyle("thoughtCard", 228)} aria-label="A small generated visual gallery">
          <div className="thought-card__top">◉ &nbsp; Brief &nbsp;&nbsp; About &nbsp;&nbsp; Contact</div>
          <div className="thought-card__photos"><i /><i /><i /></div>
          <strong>turn thoughts<br />into visuals.</strong>
        </article>
        <div className="world-item process-sketch" style={anchoredStyle("processSketch", 212)} aria-label="A hand-drawn design process diagram">
          <span>Prompt&nbsp;&nbsp;&nbsp; Design&nbsp;&nbsp;&nbsp; Preview&nbsp;&nbsp;&nbsp; Iterate&nbsp;&nbsp;&nbsp; Export</span>
          <svg viewBox="0 0 180 94" aria-hidden="true"><path d="m43 20 24-12 27 10-24 13-27-11Zm0 0v23l27 12V31m24-13v24L70 55m48-29 24-11 22 10-23 12-23-11Zm0 0v22l23 12V27m22-12v25l-22 9m-89 24 21-10 19 9-21 11-19-10Zm0 0v18l19 10V73m21-9v18l-21 9" /></svg>
        </div>

        <svg
          className="world-item identity-wordmark"
          style={wordmarkStyle()}
          viewBox="0 0 700 280"
          preserveAspectRatio="none"
          ref={wordmarkRef}
          data-reveal-started={wordmarkRevealStarted || undefined}
          role="img"
          aria-label="Spool"
        >
          <title>Spool</title>
          <g ref={wordmarkGeometryRef} className="thread-logo__wordmark">
            {WORDMARK_GLYPHS.map((glyph, index) => (
              <g key={`wordmark-position-${index}`} transform={`translate(${glyph.x} 0)`}>
                <g
                  className="thread-logo__letter"
                  style={{ "--letter-delay": `${index * WORDMARK_LETTER_STAGGER_MS}ms` } as CSSProperties}
                >
                  <path d={glyph.d} />
                </g>
              </g>
            ))}
          </g>
        </svg>
        <aside className="world-item pencil-note pencil-note--styles" style={itemStyle(24, 76, 126, 8)}>
          Different styles.<br />Same ideas. <span>↗</span>
        </aside>
        <article className="world-item editorial-fragment" style={anchoredStyle("editorialFragment", 240)} aria-label="Editorial type and photographic image">
          <div className="editorial-fragment__photo" />
          <div><h2>Small models.<br />Big ideas.</h2><p>Run locally. Stay private.<br />Make it yours.</p></div>
        </article>
        <article className="world-item local-terminal" style={anchoredStyle("terminal", 166)} aria-label="Spool local generation terminal">
          <div className="local-terminal__chrome"><i /><i /><i /><span>⌕</span></div>
          <pre>{"> spool generate\n> thinking...\n> creating layout...\n> refining visuals...\n> done."}</pre>
        </article>
        <aside className="world-item pencil-note pencil-note--local" style={itemStyle(60, 87, 130, -9)}>
          Local LMs.<br />Real creativity. <span>↙</span>
        </aside>
        <article className="world-item note-card" style={anchoredStyle("noteCard", 180)}>
          <span>“Design tools<br />should be personal<br />again.”</span>
        </article>
        <article className="world-item build-card" style={anchoredStyle("buildCard", 150)} aria-label="Generated editorial layout with a landscape photograph">
          <h3>Build faster<br />with better<br />taste.</h3><div />
        </article>
        <figure className="world-item image-stack image-stack--right" style={anchoredStyle("rightImageStack", 190)} aria-label="A collage of sky, leaves, and landscape photographs">
          <div className="image-stack__back" /><div className="image-stack__middle" /><div className="image-stack__front" />
        </figure>
        <article className="world-item dark-site dark-site--ideas" style={anchoredStyle("ideasSite", 254)} aria-label="A generated website concept about ideas taking shape">
          <div className="dark-site__bar"><b>Spool</b><span>Projects&nbsp;&nbsp; Docs&nbsp;&nbsp; GitHub</span></div>
          <div className="dark-site__content"><strong>Ideas<br />take<br />shape<br />here.</strong><div className="earth-image" /></div>
          <small>LOCAL BY DESIGN</small>
        </article>
        <aside className="world-item process-index" style={itemStyle(94, 78, 86, 2)}><span>Concept</span><span>Generate</span><span>Refine</span><span>Export</span></aside>
        <aside className="world-item pencil-note pencil-note--drag" style={itemStyle(77, 67, 152, -8)}>
          Drag anywhere<br />to explore <span>✋</span>
        </aside>
        <aside className="world-item pencil-note pencil-note--same" style={itemStyle(96, 94, 138, -5)}>
          Same tool.<br />Different worlds.
        </aside>
        <figure className="world-item flower-photo-card" style={itemStyle(88, 92, 80, 4)} aria-label="Small botanical photograph"><div /></figure>

        <div className="world-item plus-mark plus-mark--one" style={itemStyle(9, 35, 24)} aria-hidden="true">+</div>
        <div className="world-item plus-mark plus-mark--two" style={itemStyle(65, 31, 24)} aria-hidden="true">+</div>
        <div className="world-item plus-mark plus-mark--three" style={itemStyle(93, 39, 24)} aria-hidden="true">+</div>
        <div className="world-item plus-mark plus-mark--four" style={itemStyle(75, 66, 24)} aria-hidden="true">+</div>
        <div className="world-item plus-mark plus-mark--five" style={itemStyle(3, 91, 24)} aria-hidden="true">+</div>
          </div>
        </div>
      </div>
      <header
        className="site-nav"
        aria-label="Spool navigation"
        data-enabled={navigationActive}
        data-mobile-open={mobileNavOpen || undefined}
      >
        <button
          className="site-brand"
          type="button"
          aria-label="Spool home"
          disabled={!navigationActive}
          data-current={activeAnchor === "home" || undefined}
          onClick={() => navigateToAnchor("home")}
        >
          <span className="site-brand__mark" aria-hidden="true">⌘</span><span>Spool</span>
        </button>
        <button
          className="site-menu-toggle"
          type="button"
          ref={mobileNavToggleRef}
          disabled={!navigationActive}
          aria-expanded={mobileNavOpen}
          aria-controls="spool-main-navigation"
          onClick={() => setMobileNavOpen((open) => !open)}
        >
          Explore <span aria-hidden="true">{mobileNavOpen ? "−" : "+"}</span>
        </button>
        <nav className="site-links" id="spool-main-navigation" aria-label="Main navigation" data-open={mobileNavOpen}>
          {(Object.keys(SCENE_ANCHORS) as SceneAnchor[]).map((anchor) => (
            <button
              key={anchor}
              type="button"
              disabled={!navigationActive}
              data-current={activeAnchor === anchor || undefined}
              onClick={() => {
                navigateToAnchor(anchor);
                setMobileNavOpen(false);
              }}
            >
              {anchor === "useCases" ? "Use Cases" : anchor[0].toUpperCase() + anchor.slice(1)}
            </button>
          ))}
          {DOCS_URL ? (
            <a href={DOCS_URL} target="_blank" rel="noreferrer">Docs</a>
          ) : (
            <button type="button" disabled title="Documentation is not available yet">Docs</button>
          )}
          <a
            className="mobile-github"
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer"
            aria-disabled={!navigationActive || undefined}
            tabIndex={navigationActive ? 0 : -1}
            onClick={(event) => { if (!navigationActive) event.preventDefault(); }}
          >
            GitHub ↗
          </a>
        </nav>
        <div className="site-actions">
          <a
            className="site-github"
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer"
            aria-disabled={!navigationActive || undefined}
            tabIndex={navigationActive ? 0 : -1}
            onClick={(event) => { if (!navigationActive) event.preventDefault(); }}
          >
            <span className="github-mark" aria-hidden="true">◉</span><span>GitHub</span>
          </a>
          <DownloadAction className="download-small" />
        </div>
      </header>
    </>
  );
}
