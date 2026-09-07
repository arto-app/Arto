import type { MermaidConfig } from "mermaid";
import { isDarkTheme, type Theme } from "./theme";

export interface MermaidThemeConfig {
  theme: MermaidConfig["theme"];
  themeVariables: Record<string, string>;
}

/** Reads a CSS custom property; the seam that lets tests supply colours. */
export type TokenResolver = (name: string) => string;

/**
 * Build Mermaid theme configuration aligned with Arto's design tokens.
 *
 * Mermaid bakes colours into the SVG it generates, so the values have to be
 * read out of the document rather than declared: which of GitHub's themes is
 * active is only known from the tokens the stylesheet resolved. A token that
 * resolves to nothing is left out, so Mermaid falls back to its own default
 * instead of drawing with an empty colour.
 */
export function buildMermaidThemeConfig(
  theme: Theme,
  resolve: TokenResolver = cssTokens(),
): MermaidThemeConfig {
  const themeVariables: Record<string, string> = {
    // Mermaid defaults to 16px which causes text to overflow node boxes
    fontSize: "14px",
  };
  for (const [variable, token] of Object.entries(themeVariableTokens)) {
    const value = resolve(token).trim();
    if (value) {
      themeVariables[variable] = value;
    }
  }
  return { theme: isDarkTheme(theme) ? "dark" : "default", themeVariables };
}

function cssTokens(): TokenResolver {
  const style = getComputedStyle(document.body);
  return (name) => style.getPropertyValue(name);
}

/** Mermaid's theme variables, each mapped to the Primer token it stands for. */
const themeVariableTokens: Record<string, string> = {
  // Global
  background: "--bgColor-default",
  primaryColor: "--bgColor-accent-emphasis",
  // Not the ink for `primaryColor`: Mermaid falls back to `primaryTextColor`
  // for every label it has no dedicated variable for — state diagrams,
  // requirement diagrams, quadrant and xy charts — and none of those sit on
  // the emphasis blue. The ink on emphasis is named where it is really on
  // emphasis (the git branch labels).
  primaryTextColor: "--fgColor-default",
  primaryBorderColor: "--borderColor-default",
  secondaryColor: "--bgColor-muted",
  secondaryTextColor: "--fgColor-default",
  secondaryBorderColor: "--borderColor-default",
  tertiaryColor: "--bgColor-neutral-muted",
  tertiaryTextColor: "--fgColor-default",
  tertiaryBorderColor: "--borderColor-default",
  lineColor: "--fgColor-default",
  textColor: "--fgColor-default",

  // Flowchart. `nodeTextColor` is named rather than left to its
  // `primaryTextColor` fallback so the label stays tied to the fill it is
  // written on, whatever `primaryTextColor` becomes.
  mainBkg: "--bgColor-muted",
  nodeTextColor: "--fgColor-default",
  nodeBorder: "--borderColor-default",
  clusterBkg: "--bgColor-inset",
  clusterBorder: "--borderColor-default",
  edgeLabelBackground: "--bgColor-default",

  // Sequence diagram
  actorBkg: "--bgColor-muted",
  actorBorder: "--borderColor-default",
  actorTextColor: "--fgColor-default",
  signalColor: "--fgColor-default",
  signalTextColor: "--fgColor-default",
  noteBkgColor: "--bgColor-muted",
  noteTextColor: "--fgColor-default",
  noteBorderColor: "--borderColor-default",
  labelBoxBkgColor: "--bgColor-muted",
  labelTextColor: "--fgColor-default",
  loopTextColor: "--fgColor-default",
  activationBkgColor: "--bgColor-neutral-muted",
  activationBorderColor: "--borderColor-emphasis",

  // State diagram
  labelColor: "--fgColor-default",

  // Class diagram
  classText: "--fgColor-default",

  // Git graph. The emphasis scale rather than fixed hues, so the branches
  // stay distinguishable under the colour-vision themes too.
  git0: "--bgColor-accent-emphasis",
  git1: "--bgColor-success-emphasis",
  git2: "--bgColor-attention-emphasis",
  git3: "--bgColor-danger-emphasis",
  gitBranchLabel0: "--fgColor-onEmphasis",
  gitBranchLabel1: "--fgColor-onEmphasis",
  gitBranchLabel2: "--fgColor-onEmphasis",
  gitBranchLabel3: "--fgColor-onEmphasis",
  gitInv0: "--bgColor-default",
};
