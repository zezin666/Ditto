import "./styles/app.css";
import "./styles/tokens.generated.css";

const preferredTheme = window.matchMedia("(prefers-color-scheme: dark)").matches
  ? "dark"
  : "light";
document.documentElement.dataset.theme = preferredTheme;

