import githubMarkdownLightCss from "github-markdown-css/github-markdown-light.css?inline";
import githubMarkdownDarkCss from "github-markdown-css/github-markdown-dark.css?inline";
import { createThemeStyle, type Theme } from "./theme";

const lightThemeStyle = createThemeStyle(githubMarkdownLightCss, { enabled: true });
const darkThemeStyle = createThemeStyle(githubMarkdownDarkCss);

export function mount(): void {
  lightThemeStyle.mount();
  darkThemeStyle.mount();
}

export function setTheme(theme: Theme): void {
  switch (theme) {
    case "light":
      lightThemeStyle.enable();
      darkThemeStyle.disable();
      console.debug("Light theme of GitHub Markdown CSS applied");
      break;
    case "dark":
      lightThemeStyle.disable();
      darkThemeStyle.enable();
      console.debug("Dark theme of GitHub Markdown CSS applied");
      break;
  }
}
