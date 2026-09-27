export type EdgeFlag = -1 | 0 | 1;


const KNOT = [
  { x: 0.4, y: 0 },
  { x: 0.4, y: 0.045 },
  { x: 0.347, y: 0.044 },
  { x: 0.352, y: 0.074 },
  { x: 0.364, y: 0.147 },
  { x: 0.427, y: 0.2 },
  { x: 0.5, y: 0.2 },
  { x: 0.573, y: 0.2 },
  { x: 0.636, y: 0.147 },
  { x: 0.648, y: 0.074 },
  { x: 0.653, y: 0.044 },
  { x: 0.6, y: 0.045 },
  { x: 0.6, y: 0 },
];

function n(value: number): string {
  return String(Math.round(value * 10000) / 10000);
}

function edgeSegment(
  fromX: number,
  fromY: number,
  toX: number,
  toY: number,
  outX: number,
  outY: number,
  flag: EdgeFlag,
): string {
  const project = (px: number, py: number): [number, number] => {
    const x = fromX + px * (toX - fromX) + flag * py * outX;
    const y = fromY + px * (toY - fromY) + flag * py * outY;
    return [x, y];
  };

  if (flag === 0) {
    const [x, y] = project(1, 0);
    return `L ${n(x)} ${n(y)}`;
  }

  const commands: string[] = [];
  const start = project(KNOT[0].x, KNOT[0].y);
  commands.push(`L ${n(start[0])} ${n(start[1])}`);

  const curves: Array<[number, number, number, number, number, number]> = [
    [KNOT[1].x, KNOT[1].y, KNOT[2].x, KNOT[2].y, KNOT[3].x, KNOT[3].y],
    [KNOT[4].x, KNOT[4].y, KNOT[5].x, KNOT[5].y, KNOT[6].x, KNOT[6].y],
    [KNOT[7].x, KNOT[7].y, KNOT[8].x, KNOT[8].y, KNOT[9].x, KNOT[9].y],
    [KNOT[10].x, KNOT[10].y, KNOT[11].x, KNOT[11].y, KNOT[12].x, KNOT[12].y],
  ];

  for (const [c1x, c1y, c2x, c2y, ex, ey] of curves) {
    const [x1, y1] = project(c1x, c1y);
    const [x2, y2] = project(c2x, c2y);
    const [x3, y3] = project(ex, ey);
    commands.push(`C ${n(x1)} ${n(y1)} ${n(x2)} ${n(y2)} ${n(x3)} ${n(y3)}`);
  }

  const end = project(1, 0);
  commands.push(`L ${n(end[0])} ${n(end[1])}`);

  return commands.join(" ");
}

export function piecePath(edges: [EdgeFlag, EdgeFlag, EdgeFlag, EdgeFlag]): string {
  const [top, right, bottom, left] = edges;

  const start = [0, 0];
  const parts: string[] = [`M ${n(start[0])} ${n(start[1])}`];

  parts.push(edgeSegment(0, 0, 1, 0, 0, -1, top));
  parts.push(edgeSegment(1, 0, 1, 1, 1, 0, right));
  parts.push(edgeSegment(1, 1, 0, 1, 0, 1, bottom));
  parts.push(edgeSegment(0, 1, 0, 0, -1, 0, left));
  parts.push("Z");

  return parts.join(" ");
}

export function clipId(edges: [EdgeFlag, EdgeFlag, EdgeFlag, EdgeFlag]): string {
  return `spool-piece-${edges.join("_").replace(/-/g, "m")}`;
}
