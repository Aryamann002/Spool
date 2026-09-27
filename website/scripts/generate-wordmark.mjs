import { createRequire } from "node:module";
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const opentype = require("opentype.js");
const root = new URL("../", import.meta.url);
const fontPath = fileURLToPath(new URL("public/fonts/wobble.ttf", root));
const outputPath = fileURLToPath(new URL("src/components/wordmark-paths.ts", root));
const font = opentype.loadSync(fontPath);
const emSize = font.unitsPerEm;
const tracking = -85;
const letterDuration = 260;
const letterStagger = 80;
const sourceGlyphs = [
  { value: "S", fontName: "S" },
  { value: "p", fontName: "uniE029" },
  { value: "o", fontName: "o" },
  { value: "o", fontName: "o" },
  { value: "l", fontName: "uniE025" },
];

function clean(value) {
  return Number(value.toFixed(3));
}

function serializePath(commands) {
  return commands.map((command) => {
    switch (command.type) {
      case "M":
      case "L":
        return `${command.type}${clean(command.x)} ${clean(command.y)}`;
      case "Q":
        return `Q${clean(command.x1)} ${clean(command.y1)} ${clean(command.x)} ${clean(command.y)}`;
      case "C":
        return `C${clean(command.x1)} ${clean(command.y1)} ${clean(command.x2)} ${clean(command.y2)} ${clean(command.x)} ${clean(command.y)}`;
      case "Z":
        return "Z";
      default:
        throw new Error(`Unsupported outline command: ${command.type}`);
    }
  }).join("");
}

const glyphs = sourceGlyphs.map(({ value, fontName }) => {
  const glyph = font.nameToGlyph(fontName);
  if (!glyph || glyph.index === 0) throw new Error(`Missing required wordmark glyph: ${fontName}`);

  return {
    value,
    glyph,
    d: serializePath(glyph.getPath(0, 0, emSize).commands),
  };
});

let cursor = 0;
for (const [index, glyph] of glyphs.entries()) {
  glyph.x = cursor;
  cursor += glyph.glyph.advanceWidth;
  const nextGlyph = glyphs[index + 1];
  if (nextGlyph) cursor += font.getKerningValue(glyph.glyph, nextGlyph.glyph) + tracking;
}

const generatedGlyphs = glyphs.map((glyph) => ({
  value: glyph.value,
  x: clean(glyph.x),
  d: glyph.d,
}));
const totalAdvance = cursor;
const output = `// Generated from public/fonts/wobble.ttf; edit the generator, not this file.\nexport const WORDMARK_UNITS_PER_EM = ${emSize};\nexport const WORDMARK_TOTAL_ADVANCE = ${clean(totalAdvance)};\nexport const WORDMARK_LETTER_DURATION_MS = ${letterDuration};\nexport const WORDMARK_LETTER_STAGGER_MS = ${letterStagger};\nexport const WORDMARK_GLYPHS = ${JSON.stringify(generatedGlyphs, null, 2)} as const;\nexport const WORDMARK_REVEAL_DURATION_MS = WORDMARK_LETTER_DURATION_MS + (WORDMARK_GLYPHS.length - 1) * WORDMARK_LETTER_STAGGER_MS;\n`;

await writeFile(outputPath, output);
console.log(`Generated ${glyphs.length} Wobble glyphs.`);
