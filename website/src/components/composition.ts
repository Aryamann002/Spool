export type WorldAnchor = {
  desktop: readonly [x: number, y: number];
  compact: readonly [x: number, y: number];
  rotation: number;
};

export const WORLD_ANCHORS = {
  opnest: { desktop: [18, 20], compact: [28, 17], rotation: -5 },
  miniBrowser: { desktop: [33, 21], compact: [79, 19], rotation: -7 },
  palette: { desktop: [44, 20], compact: [51, 24], rotation: 2 },
  paperSketch: { desktop: [55, 20], compact: [55, 20], rotation: 5 },
  eyeStudy: { desktop: [72, 19], compact: [72, 19], rotation: -3 },
  darkThoughtful: { desktop: [86, 20], compact: [85, 31], rotation: 5 },
  blackSketch: { desktop: [2, 30], compact: [9, 30], rotation: 4 },
  leftImageStack: { desktop: [16, 37], compact: [20, 37], rotation: -3 },
  japanesePoster: { desktop: [4, 47], compact: [7, 45], rotation: -7 },
  hero: { desktop: [50, 38], compact: [50, 39], rotation: 0 },
  rightImageStack: { desktop: [79, 43], compact: [77, 45], rotation: -4 },
  noteCard: { desktop: [80, 54], compact: [79, 55], rotation: -7 },
  humanSite: { desktop: [12, 64], compact: [17, 66], rotation: 5 },
  thoughtCard: { desktop: [8, 76], compact: [17, 80], rotation: 3 },
  wordmark: { desktop: [49, 66], compact: [50, 64], rotation: 0 },
  processSketch: { desktop: [14, 90], compact: [16, 93], rotation: -2 },
  editorialFragment: { desktop: [40, 88], compact: [41, 90], rotation: -1 },
  terminal: { desktop: [49, 89], compact: [68, 89], rotation: 2 },
  buildCard: { desktop: [93, 55], compact: [92, 62], rotation: 3 },
  ideasSite: { desktop: [83, 84], compact: [81, 84], rotation: -7 },
} as const satisfies Record<string, WorldAnchor>;

export type WorldRegion = keyof typeof WORLD_ANCHORS;

export const SCENE_ANCHORS = {
  home: "hero",
  gallery: "rightImageStack",
  features: "processSketch",
  useCases: "ideasSite",
} as const satisfies Record<string, WorldRegion>;

export type SceneAnchor = keyof typeof SCENE_ANCHORS;

export function anchorPosition(region: WorldRegion, compact = false) {
  const [x, y] = WORLD_ANCHORS[region][compact ? "compact" : "desktop"];
  return { x, y, rotation: WORLD_ANCHORS[region].rotation };
}
