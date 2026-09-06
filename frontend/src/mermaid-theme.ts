import type { Theme } from "./theme";

export interface MermaidThemeConfig {
  theme: string;
  themeVariables: Record<string, string>;
}

/**
 * Build Mermaid theme configuration aligned with Arto's design tokens.
 *
 * Maps Arto CSS color variables to Mermaid themeVariables so diagrams
 * blend naturally with the app's light/dark theme.
 */
export function buildMermaidThemeConfig(theme: Theme): MermaidThemeConfig {
  if (theme === "dark") {
    return { theme: "dark", themeVariables: darkThemeVariables };
  }
  return { theme: "default", themeVariables: lightThemeVariables };
}

// Shared font size matching Arto's --font-size-base (14px)
// Mermaid defaults to 16px which causes text to overflow node boxes
const sharedFontVariables: Record<string, string> = {
  fontSize: "14px",
};

// Arto dark theme colors (from variables.css --dark-* tokens)
const darkThemeVariables: Record<string, string> = {
  ...sharedFontVariables,

  // Global
  background: "#0d1117", // --dark-content-bg
  primaryColor: "#1f6feb", // --dark-accent-bg
  primaryTextColor: "#e6edf3", // --dark-text-color
  primaryBorderColor: "#30363d", // --dark-border-color
  secondaryColor: "#1f2937", // --dark-bg-secondary
  secondaryTextColor: "#e6edf3", // --dark-text-color
  secondaryBorderColor: "#30363d", // --dark-border-color
  tertiaryColor: "#374151", // --dark-bg-tertiary
  tertiaryTextColor: "#e6edf3", // --dark-text-color
  tertiaryBorderColor: "#30363d", // --dark-border-color
  lineColor: "#e6edf3", // --dark-text-color
  textColor: "#e6edf3", // --dark-text-color

  // Flowchart
  mainBkg: "#1f2937", // --dark-bg-secondary
  nodeBorder: "#30363d", // --dark-border-color
  clusterBkg: "#161b22", // slightly darker than bg-secondary
  clusterBorder: "#30363d", // --dark-border-color
  edgeLabelBackground: "#1f2937", // --dark-bg-secondary

  // Sequence diagram
  actorBkg: "#1f2937", // --dark-bg-secondary
  actorBorder: "#30363d", // --dark-border-color
  actorTextColor: "#e6edf3", // --dark-text-color
  signalColor: "#e6edf3", // --dark-text-color
  signalTextColor: "#e6edf3", // --dark-text-color
  noteBkgColor: "#1f2937", // --dark-bg-secondary
  noteTextColor: "#e6edf3", // --dark-text-color
  noteBorderColor: "#30363d", // --dark-border-color
  labelBoxBkgColor: "#1f2937", // --dark-bg-secondary
  labelTextColor: "#e6edf3", // --dark-text-color
  loopTextColor: "#e6edf3", // --dark-text-color
  activationBkgColor: "#374151", // --dark-bg-tertiary
  activationBorderColor: "#484f58", // --dark-hover-border

  // State diagram
  labelColor: "#e6edf3", // --dark-text-color

  // Class diagram
  classText: "#e6edf3", // --dark-text-color

  // Git graph
  git0: "#1f6feb", // --dark-accent-bg
  git1: "#22c55e", // --success-color
  git2: "#dc8a2f", // --warning-color
  git3: "#dc3545", // --error-color
  gitBranchLabel0: "#e6edf3",
  gitBranchLabel1: "#e6edf3",
  gitBranchLabel2: "#e6edf3",
  gitBranchLabel3: "#e6edf3",
  gitInv0: "#0d1117",
};

// Arto light theme colors (from variables.css --light-* tokens)
const lightThemeVariables: Record<string, string> = {
  ...sharedFontVariables,

  // Global
  background: "#ffffff", // --light-content-bg
  primaryColor: "#0969da", // --light-accent-bg
  primaryTextColor: "#1f2328", // --light-text-color
  primaryBorderColor: "#d1d9e0", // --light-border-color
  secondaryColor: "#f9fafb", // --light-bg-secondary
  secondaryTextColor: "#1f2328", // --light-text-color
  secondaryBorderColor: "#d1d9e0", // --light-border-color
  tertiaryColor: "#ffffff", // --light-bg-tertiary
  tertiaryTextColor: "#1f2328", // --light-text-color
  tertiaryBorderColor: "#d1d9e0", // --light-border-color
  lineColor: "#1f2328", // --light-text-color
  textColor: "#1f2328", // --light-text-color

  // Flowchart
  mainBkg: "#f9fafb", // --light-bg-secondary
  nodeBorder: "#d1d9e0", // --light-border-color
  clusterBkg: "#f6f8fa", // --light-header-bg
  clusterBorder: "#d1d9e0", // --light-border-color
  edgeLabelBackground: "#ffffff", // --light-content-bg

  // Sequence diagram
  actorBkg: "#f9fafb", // --light-bg-secondary
  actorBorder: "#d1d9e0", // --light-border-color
  actorTextColor: "#1f2328", // --light-text-color
  signalColor: "#1f2328", // --light-text-color
  signalTextColor: "#1f2328", // --light-text-color
  noteBkgColor: "#f9fafb", // --light-bg-secondary
  noteTextColor: "#1f2328", // --light-text-color
  noteBorderColor: "#d1d9e0", // --light-border-color
  labelBoxBkgColor: "#f9fafb", // --light-bg-secondary
  labelTextColor: "#1f2328", // --light-text-color
  loopTextColor: "#1f2328", // --light-text-color
  activationBkgColor: "#ffffff", // --light-bg-tertiary
  activationBorderColor: "#b4bcc4", // --light-hover-border

  // State diagram
  labelColor: "#1f2328", // --light-text-color

  // Class diagram
  classText: "#1f2328", // --light-text-color

  // Git graph
  git0: "#0969da", // --light-accent-bg
  git1: "#22c55e", // --success-color
  git2: "#dc8a2f", // --warning-color
  git3: "#dc3545", // --error-color
  gitBranchLabel0: "#ffffff",
  gitBranchLabel1: "#ffffff",
  gitBranchLabel2: "#ffffff",
  gitBranchLabel3: "#ffffff",
  gitInv0: "#ffffff",
};
