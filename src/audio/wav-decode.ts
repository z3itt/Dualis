export interface ParsedWav {
  sampleRate: number;
  channels: Float32Array[];
}

export function parseWavToFloatChannels(data: ArrayBuffer): ParsedWav {
  const view = new DataView(data);
  if (data.byteLength < 44 || readFourCC(view, 0) !== "RIFF" || readFourCC(view, 8) !== "WAVE") {
    throw new Error("Stem file is not a valid WAV");
  }

  let offset = 12;
  let audioFormat = 0;
  let channels = 0;
  let sampleRate = 44100;
  let bitsPerSample = 16;
  let blockAlign = 0;
  let dataOffset = 0;
  let dataSize = 0;

  while (offset + 8 <= data.byteLength) {
    const id = readFourCC(view, offset);
    const size = view.getUint32(offset + 4, true);
    const chunkStart = offset + 8;
    if (chunkStart + size > data.byteLength) {
      break;
    }
    if (id === "fmt ") {
      audioFormat = view.getUint16(chunkStart, true);
      channels = view.getUint16(chunkStart + 2, true);
      sampleRate = view.getUint32(chunkStart + 4, true);
      blockAlign = view.getUint16(chunkStart + 12, true);
      bitsPerSample = view.getUint16(chunkStart + 14, true);
      if (audioFormat === 0xfffe && size >= 26) {
        audioFormat = view.getUint16(chunkStart + 24, true);
      }
    } else if (id === "data") {
      dataOffset = chunkStart;
      dataSize = size;
    }
    offset = chunkStart + size + (size % 2);
  }

  if (!channels || !dataSize || !dataOffset) {
    throw new Error("WAV is missing audio data");
  }
  if (blockAlign === 0) {
    blockAlign = Math.max(1, (channels * bitsPerSample) / 8);
  }

  const frameCount = Math.floor(dataSize / blockAlign);
  const channelData = Array.from({ length: channels }, () => new Float32Array(frameCount));

  for (let frame = 0; frame < frameCount; frame += 1) {
    let pos = dataOffset + frame * blockAlign;
    for (let ch = 0; ch < channels; ch += 1) {
      channelData[ch][frame] = readWavSample(view, pos, bitsPerSample, audioFormat);
      pos += bitsPerSample / 8;
    }
  }

  return { sampleRate, channels: channelData };
}

function readFourCC(view: DataView, offset: number) {
  let out = "";
  for (let i = 0; i < 4; i += 1) {
    out += String.fromCharCode(view.getUint8(offset + i));
  }
  return out;
}

function readWavSample(view: DataView, pos: number, bitsPerSample: number, audioFormat: number) {
  if (audioFormat === 3 || bitsPerSample === 32) {
    return view.getFloat32(pos, true);
  }
  if (bitsPerSample === 16) {
    return view.getInt16(pos, true) / 32768;
  }
  if (bitsPerSample === 24) {
    const b0 = view.getUint8(pos);
    const b1 = view.getUint8(pos + 1);
    const b2 = view.getUint8(pos + 2);
    let value = b0 | (b1 << 8) | (b2 << 16);
    if (value & 0x800000) {
      value |= ~0xffffff;
    }
    return value / 8388608;
  }
  if (bitsPerSample === 8) {
    return (view.getUint8(pos) - 128) / 128;
  }
  throw new Error(`Unsupported WAV format (${bitsPerSample}-bit)`);
}
