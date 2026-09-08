const BASE_WIDTH = 1280;
const BASE_HEIGHT = 820;
const MIN_SCALE = 1;
const MAX_SCALE = 1.5;

export function applyUiScale() {
  const width = window.innerWidth;
  const height = window.innerHeight;
  const scale = Math.min(width / BASE_WIDTH, height / BASE_HEIGHT);
  const next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale));
  document.documentElement.style.fontSize = `${(16 * next).toFixed(2)}px`;
  document.documentElement.style.setProperty("--ui-scale", String(next));
}
