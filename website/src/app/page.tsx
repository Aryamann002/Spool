"use client";

import { useEffect, useState, useSyncExternalStore } from "react";
import PuzzleLoader, { type PuzzleRevealMode } from "../components/puzzle/PuzzleLoader";
import { usePrefersReducedMotion } from "../components/puzzle/use-prefers-reduced-motion";
import WorldCanvas from "../components/WorldCanvas";

type ExperienceState =
  | "void"
  | "canvas"
  | "puzzle-reveal"
  | "puzzle-complete"
  | "puzzle-exit"
  | "ready-for-thread"
  | "thread"
  | "exploring";

type DebugState = Exclude<ExperienceState, "void" | "puzzle-exit">;

const PHASE_QUERY: Record<string, DebugState> = {
  canvas: "canvas",
  reveal: "puzzle-reveal",
  complete: "puzzle-complete",
  ready: "ready-for-thread",
  thread: "thread",
  explore: "exploring",
};

function subscribeToPhase(onChange: () => void): () => void {
  window.addEventListener("popstate", onChange);
  return () => window.removeEventListener("popstate", onChange);
}

function getPhaseSnapshot(): DebugState | null {
  const requestedPhase = new URLSearchParams(window.location.search).get("phase");
  return requestedPhase ? PHASE_QUERY[requestedPhase] ?? null : null;
}

function getServerPhaseSnapshot(): null {
  return null;
}

const TIMING = {
  canvas: 1450,
  reveal: 1300,
  hold: 360,
  exit: 320,
  ready: 520,
  thread: 540,
  reducedCanvas: 180,
  reducedHold: 100,
  reducedExit: 180,
  reducedReady: 80,
  reducedThread: 180,
} as const;

export default function Home() {
  const [experienceState, setExperienceState] = useState<ExperienceState>("void");
  const [replayState, setReplayState] = useState<ExperienceState | null>(null);
  const [puzzleMode, setPuzzleMode] = useState<PuzzleRevealMode>("veil");
  const [puzzleReplayId, setPuzzleReplayId] = useState(0);
  const debugState = useSyncExternalStore(
    subscribeToPhase,
    getPhaseSnapshot,
    getServerPhaseSnapshot,
  );
  const visibleState = replayState ?? debugState ?? experienceState;
  const reducedMotion = usePrefersReducedMotion();

  useEffect(() => {
    if (debugState) return;
    const frame = window.requestAnimationFrame(() => setExperienceState("canvas"));
    return () => window.cancelAnimationFrame(frame);
  }, [debugState]);

  useEffect(() => {
    const stateToAdvance = replayState ?? (debugState ? null : experienceState);
    if (!stateToAdvance) return;

    let delay: number;
    let nextState: ExperienceState;

    switch (stateToAdvance) {
      case "canvas":
        delay = reducedMotion ? TIMING.reducedCanvas : TIMING.canvas;
        nextState = reducedMotion ? "puzzle-complete" : "puzzle-reveal";
        break;
      case "puzzle-reveal":
        delay = TIMING.reveal;
        nextState = "puzzle-complete";
        break;
      case "puzzle-complete":
        delay = reducedMotion ? TIMING.reducedHold : TIMING.hold;
        nextState = "puzzle-exit";
        break;
      case "puzzle-exit":
        delay = reducedMotion ? TIMING.reducedExit : TIMING.exit;
        nextState = "ready-for-thread";
        break;
      case "ready-for-thread":
        delay = reducedMotion ? TIMING.reducedReady : TIMING.ready;
        nextState = "thread";
        break;
      case "thread":
        delay = reducedMotion ? TIMING.reducedThread : TIMING.thread;
        nextState = "exploring";
        break;
      default:
        return;
    }

    const timer = window.setTimeout(() => {
      if (replayState !== null) setReplayState(nextState);
      else setExperienceState(nextState);
    }, delay);
    return () => window.clearTimeout(timer);
  }, [debugState, experienceState, reducedMotion, replayState]);

  const showWorld = visibleState.startsWith("puzzle-")
    || visibleState === "ready-for-thread"
    || visibleState === "thread"
    || visibleState === "exploring";
  const puzzleStage = visibleState === "puzzle-reveal"
    ? "reveal"
    : visibleState === "puzzle-exit"
      ? "exit"
      : "complete";
  const showPuzzle = visibleState === "puzzle-reveal"
    || visibleState === "puzzle-complete"
    || visibleState === "puzzle-exit";
  const switchPuzzleMode = () => {
    setPuzzleMode((mode) => mode === "veil" ? "void" : "veil");
    setPuzzleReplayId((id) => id + 1);
    setExperienceState("puzzle-reveal");
    setReplayState("puzzle-reveal");
  };

  return (
    <main
      className="experience-shell"
      data-experience-state={visibleState}
      data-puzzle-mode={puzzleMode}
      aria-label="Spool creative canvas"
    >
      <div className="experience-viewport" data-camera-viewport>
        <div
          className="experience-world"
          data-camera-world
          data-reveal-mode={puzzleMode}
          data-puzzle-stage={showPuzzle ? puzzleStage : "idle"}
          aria-hidden={!showWorld}
        >
          <WorldCanvas phase={visibleState} />
        </div>
        {showPuzzle && <PuzzleLoader key={`${puzzleMode}-${puzzleReplayId}`} stage={puzzleStage} mode={puzzleMode} />}
      </div>
      <button
        className="puzzle-style-toggle"
        type="button"
        aria-label={`Reveal style: ${puzzleMode}. Switch to ${puzzleMode === "veil" ? "void" : "veil"}.`}
        onClick={switchPuzzleMode}
      >
        Reveal: {puzzleMode === "veil" ? "Veil" : "Void"}
      </button>
    </main>
  );
}
