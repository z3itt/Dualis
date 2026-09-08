import { describe, expect, it } from "vitest";
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { parseWavToFloatChannels } from "./wav-decode";

function writeMonoWav(path: string, samples: number[], bits: 16 | 24 | 32, float = false) {
  const channels = 1;
  const sampleRate = 44100;
  const bytesPerSample = bits / 8;
  const blockAlign = channels * bytesPerSample;
  const dataSize = samples.length * blockAlign;
  const buffer = Buffer.alloc(44 + dataSize);
  buffer.write("RIFF", 0);
  buffer.writeUInt32LE(36 + dataSize, 4);
  buffer.write("WAVE", 8);
  buffer.write("fmt ", 12);
  buffer.writeUInt32LE(float ? 18 : 16, 16);
  buffer.writeUInt16LE(float ? 3 : 1, 20);
  buffer.writeUInt16LE(channels, 22);
  buffer.writeUInt32LE(sampleRate, 24);
  buffer.writeUInt32LE(sampleRate * blockAlign, 28);
  buffer.writeUInt16LE(blockAlign, 32);
  buffer.writeUInt16LE(bits, 34);
  buffer.write("data", 36);
  buffer.writeUInt32LE(dataSize, 40);
  let offset = 44;
  for (const sample of samples) {
    if (float) {
      buffer.writeFloatLE(sample, offset);
    } else if (bits === 16) {
      buffer.writeInt16LE(Math.round(sample * 32767), offset);
    } else {
      const value = Math.round(sample * 8388607);
      buffer.writeUInt8(value & 0xff, offset);
      buffer.writeUInt8((value >> 8) & 0xff, offset + 1);
      buffer.writeUInt8((value >> 16) & 0xff, offset + 2);
    }
    offset += bytesPerSample;
  }
  writeFileSync(path, buffer);
}

describe("wav-decode", () => {
  it("decodes 24-bit pcm wav", () => {
    const dir = mkdtempSync(join(tmpdir(), "dualis-wav-"));
    const path = join(dir, "stem.wav");
    writeMonoWav(path, [0, 0.5, -0.75, 0.25], 24);
    const file = readFileSync(path);
    const parsed = parseWavToFloatChannels(file.buffer.slice(file.byteOffset, file.byteOffset + file.byteLength));
    expect(parsed.channels[0].length).toBe(4);
    expect(parsed.channels[0][1]).toBeCloseTo(0.5, 2);
    expect(parsed.channels[0][2]).toBeCloseTo(-0.75, 2);
    rmSync(dir, { recursive: true, force: true });
  });
});
