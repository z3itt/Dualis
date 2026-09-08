import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { readUiStorage } from "@/lib/storage";
import { applyUiScale } from "@/lib/ui-scale";
import "./index.css";

applyUiScale();

try {
  const saved = JSON.parse(readUiStorage()) as { theme?: string };
  if (saved.theme === "dark") {
    document.documentElement.classList.add("dark");
  }
} catch {
  /* ignore */
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
