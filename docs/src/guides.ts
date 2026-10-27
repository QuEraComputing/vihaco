// Reading order for the author guide sidebar and previous/next links.
export interface Guide {
  slug: string;
  label: string;
  short: string;
}

export const guides: Guide[] = [
  { slug: "index", label: "Start here", short: "Start here" },
  { slug: "mental-model", label: "The mental model", short: "Mental model" },
  { slug: "first-machine", label: "Your first machine", short: "First machine" },
  { slug: "instructions", label: "Dialects and instructions", short: "Dialects" },
  { slug: "components", label: "Writing components", short: "Components" },
  { slug: "messages", label: "Preparing messages", short: "Messages" },
  { slug: "stack", label: "Stack inputs and outputs", short: "Stack protocols" },
  { slug: "composites", label: "Writing composites", short: "Composites" },
  { slug: "execution", label: "Executing one instruction", short: "Execution traits" },
  { slug: "effects", label: "Handling effects", short: "Effects" },
  { slug: "program-execution", label: "Running programs", short: "Programs" },
  { slug: "loadables", label: "Images and Loadables", short: "Loadables" },
  { slug: "parser", label: "Parsing source", short: "Parsing" },
  { slug: "parser-advanced", label: "Loading SST programs", short: "SST loading" },
  { slug: "instructions-advanced", label: "Binary formats", short: "Binary formats" },
  { slug: "patterns", label: "Patterns for larger machines", short: "Larger machines" },
  { slug: "testing", label: "Testing and extending", short: "Testing" },
  { slug: "parser-patterns", label: "Pattern parser reference", short: "Pattern reference" },
];
