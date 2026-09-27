import { piecePath, type EdgeFlag } from "./puzzle-path";

export type PuzzlePiece = {
  id: string;
  path: string;
  row: number;
  column: number;
  x: number;
  y: number;
  rotation: 0;
  enterDelay: number;
};

type PuzzleGrid = {
  columns: number;
  rows: number;
};

const DESKTOP_GRID: PuzzleGrid = { columns: 8, rows: 4 };
const COMPACT_GRID: PuzzleGrid = { columns: 5, rows: 5 };
const REVEAL_GROUP_INTERVAL_MIN_MS = 90;
const REVEAL_GROUP_INTERVAL_RANGE_MS = 23;
const REVEAL_OFFSET_RANGE_MS = 31;
const MAX_GROUP_SIZE = 5;

function horizontalEdge(row: number, column: number): EdgeFlag {
  return (row + column) % 2 === 0 ? 1 : -1;
}

function verticalEdge(row: number, column: number): EdgeFlag {
  return (row + column) % 2 === 0 ? -1 : 1;
}

function oppositeEdge(edge: EdgeFlag): EdgeFlag {
  if (edge === 1) return -1;
  if (edge === -1) return 1;
  return 0;
}

export function getPuzzleGrid(compact = false): PuzzleGrid {
  return compact ? COMPACT_GRID : DESKTOP_GRID;
}

export function buildPuzzle(compact = false): PuzzlePiece[] {
  const { columns, rows } = getPuzzleGrid(compact);
  const pieces = Array.from({ length: columns * rows }, (_, index) => {
    const row = Math.floor(index / columns);
    const column = index % columns;
    const edges: [EdgeFlag, EdgeFlag, EdgeFlag, EdgeFlag] = [
      row === 0 ? 0 : oppositeEdge(verticalEdge(row - 1, column)),
      column === columns - 1 ? 0 : horizontalEdge(row, column),
      row === rows - 1 ? 0 : verticalEdge(row, column),
      column === 0 ? 0 : oppositeEdge(horizontalEdge(row, column - 1)),
    ];

    return {
      id: `reveal-${index + 1}`,
      path: piecePath(edges),
      row,
      column,
      x: (column + 0.5) / columns,
      y: (row + 0.5) / rows,
      rotation: 0 as const,
      enterDelay: 0,
    };
  });

  return pieces;
}

export function schedulePuzzleReveal(pieces: PuzzlePiece[]): PuzzlePiece[] {
  const random = Math.random;
  const order = pieces.map((_, index) => index);
  for (let index = order.length - 1; index > 0; index -= 1) {
    const swapIndex = Math.floor(random() * (index + 1));
    [order[index], order[swapIndex]] = [order[swapIndex], order[index]];
  }

  const scheduledPieces = pieces.map((piece) => ({ ...piece, enterDelay: 0 }));
  const groupCount = Math.ceil(order.length / 3.2);
  let cursor = 0;
  let batch = 0;
  let batchStart = 0;
  let previousGroupSize = 0;

  while (cursor < order.length) {
    const remaining = order.length - cursor;
    const groupsAfter = groupCount - batch - 1;
    const smallestPossible = Math.max(1, remaining - groupsAfter * MAX_GROUP_SIZE);
    const largestPossible = Math.min(MAX_GROUP_SIZE, remaining - groupsAfter);
    const possibleSizes = Array.from(
      { length: largestPossible - smallestPossible + 1 },
      (_, offset) => smallestPossible + offset,
    );
    const variedSizes = possibleSizes.filter((size) => size !== previousGroupSize);
    const choices = variedSizes.length ? variedSizes : possibleSizes;
    const groupSize = choices[Math.floor(random() * choices.length)];

    for (const pieceIndex of order.slice(cursor, cursor + groupSize)) {
      scheduledPieces[pieceIndex].enterDelay = batchStart + Math.floor(random() * REVEAL_OFFSET_RANGE_MS);
    }

    cursor += groupSize;
    batch += 1;
    previousGroupSize = groupSize;
    batchStart += REVEAL_GROUP_INTERVAL_MIN_MS + Math.floor(random() * REVEAL_GROUP_INTERVAL_RANGE_MS);
  }

  return scheduledPieces;
}
