import type { StemMode } from "@/lib/types";

type Listener = () => void;

export class DualStemEngine {
  private vocalsEl: HTMLAudioElement;
  private instEl: HTMLAudioElement;
  playing = false;
  duration = 0;
  private volume = 0.9;
  private mode: StemMode = "original";
  private loop = false;
  private endedArmed = false;
  private endedConsumed = false;
  private endedCallback: (() => void) | null = null;
  private listeners = new Set<Listener>();

  constructor() {
    this.vocalsEl = new Audio();
    this.instEl = new Audio();
    this.vocalsEl.preload = "auto";
    this.instEl.preload = "auto";
    this.vocalsEl.crossOrigin = "anonymous";
    this.instEl.crossOrigin = "anonymous";
    const syncDuration = () => {
      const left = Number.isFinite(this.vocalsEl.duration) ? this.vocalsEl.duration : 0;
      const right = Number.isFinite(this.instEl.duration) ? this.instEl.duration : 0;
      const next = left > 0 && right > 0 ? Math.min(left, right) : left || right;
      if (next > 0 && next !== this.duration) {
        this.duration = next;
        this.notify();
      }
    };
    this.vocalsEl.addEventListener("loadedmetadata", syncDuration);
    this.instEl.addEventListener("loadedmetadata", syncDuration);
    this.vocalsEl.addEventListener("ended", () => {
      if (this.loop) {
        this.instEl.currentTime = 0;
        void this.instEl.play().catch(() => undefined);
        return;
      }
      this.markEnded();
    });
  }

  onChange(listener: Listener) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  onEnded(listener: () => void) {
    this.endedCallback = listener;
  }

  setLoop(loop: boolean) {
    this.loop = loop;
    this.vocalsEl.loop = loop;
    this.instEl.loop = loop;
  }

  pollEnded() {
    if (this.loop || this.endedArmed || this.endedConsumed) {
      return;
    }
    if (this.vocalsEl.ended) {
      this.markEnded();
    }
  }

  consumeEnded() {
    if (!this.endedArmed) {
      return false;
    }
    this.endedArmed = false;
    this.endedConsumed = true;
    return true;
  }

  private markEnded() {
    if (this.loop || this.endedArmed || this.endedConsumed) {
      return;
    }
    this.endedArmed = true;
    this.playing = false;
    this.notify();
    this.endedCallback?.();
  }

  private notify() {
    this.listeners.forEach((fn) => fn());
  }

  currentTime() {
    return this.vocalsEl.currentTime || 0;
  }

  waveform(bars = 128) {
    return Array.from({ length: bars }, () => 0.12);
  }

  async load(vocalsUrl: string, instrumentalUrl: string) {
    this.stop();
    this.endedArmed = false;
    this.endedConsumed = false;
    this.vocalsEl.src = vocalsUrl;
    this.instEl.src = instrumentalUrl;
    this.duration = 0;
    this.playing = false;
    this.applyVolumes();
    this.vocalsEl.load();
    this.instEl.load();
    this.notify();
  }

  async play() {
    if (!this.vocalsEl.src || !this.instEl.src) {
      return;
    }
    this.syncCurrentTime();
    this.applyVolumes();
    await Promise.all([this.vocalsEl.play(), this.instEl.play()]);
    this.playing = true;
    this.notify();
  }

  pause() {
    this.vocalsEl.pause();
    this.instEl.pause();
    this.playing = false;
    this.notify();
  }

  seek(seconds: number) {
    const next = Math.max(0, Math.min(this.duration || seconds, seconds));
    const wasPlaying = this.playing;
    this.vocalsEl.pause();
    this.instEl.pause();
    this.vocalsEl.currentTime = next;
    this.instEl.currentTime = next;
    const ended = this.duration > 0 && next >= this.duration - 0.02;
    if (wasPlaying && !ended) {
      void this.play();
      return;
    }
    this.playing = false;
    this.notify();
  }

  setVolume(value: number) {
    this.volume = value;
    this.applyVolumes();
  }

  setMode(mode: StemMode) {
    this.mode = mode;
    this.applyVolumes();
  }

  private applyVolumes() {
    const vocalsGain = this.mode === "instrumental" ? 0 : 1;
    const instGain = this.mode === "vocals" ? 0 : 1;
    this.vocalsEl.volume = clamp01(this.volume * vocalsGain);
    this.instEl.volume = clamp01(this.volume * instGain);
    this.vocalsEl.muted = vocalsGain === 0;
    this.instEl.muted = instGain === 0;
  }

  private syncCurrentTime() {
    const t = this.vocalsEl.currentTime;
    if (Math.abs(this.instEl.currentTime - t) > 0.05) {
      this.instEl.currentTime = t;
    }
  }

  private stop() {
    this.vocalsEl.pause();
    this.instEl.pause();
    this.playing = false;
  }
}

function clamp01(value: number) {
  return Math.max(0, Math.min(1, value));
}
