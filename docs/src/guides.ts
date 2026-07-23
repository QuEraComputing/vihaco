// The ordered list of guide pages. Drives both the left "Guides" sidebar in
// Guide.astro and the prev/next pager at the foot of each guide. The `slug`
// matches the markdown file name under `src/pages/guide/` (and its route).
export interface Guide {
  slug: string;
  label: string;
  short: string;
}

export const guides: Guide[] = [
  { slug: "instructions", label: "Defining Instructions", short: "Instructions" },
  { slug: "instructions-advanced", label: "Advanced Instruction Usage", short: "Instructions · advanced" },
  { slug: "parser", label: "Parser Integration", short: "Parser" },
  { slug: "parser-patterns", label: "Pattern Parser Generator", short: "Parser · patterns" },
  { slug: "parser-advanced", label: "Advanced Parser Customization", short: "Parser · advanced" },
  { slug: "messages", label: "Using Messages", short: "Messages" },
  { slug: "components", label: "Building Components", short: "Components" },
  { slug: "observers", label: "Observing Effects", short: "Observers" },
  { slug: "composites", label: "Defining a Composite", short: "Composites" },
];
