import "./styles/tokens.generated.css";
import "./styles/app.css";
import { createRoot } from "react-dom/client";
import { StrictMode } from "react";
import { App } from "./App";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Settings root element is missing");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

