import { defineConfig } from "astro/config";

// `site` + `base` are read from env in CI so the same astro config powers
// three deploy targets (the repo is private, so Pages serves the gh-pages root
// at a `*.pages.github.io` subdomain — i.e. base `/`, not `/vihaco`):
//   · local dev:        VIHACO_SITE=http://localhost:4321   VIHACO_BASE=/
//   · main → gh-pages:  VIHACO_SITE=https://<pages-host>     VIHACO_BASE=/
//   · PR preview deploy:VIHACO_SITE=https://<pages-host>     VIHACO_BASE=/pr-preview/pr-<N>
const site = process.env.VIHACO_SITE ?? "https://probable-adventure-1qn9gkl.pages.github.io";
const base = process.env.VIHACO_BASE ?? "/";

// Self-contained rehype plugin: prefix every root-absolute internal link
// (`/guide/...`, `/quickstart`, …) with the deploy base so the markdown
// guides stay correct under both the gh-pages root and per-PR preview
// subpaths — without importing any external visitor.
const baseNoSlash = base.replace(/\/$/, "");
function rehypeBaseLinks() {
  const prefix = (node) => {
    if (node.type === "element" && node.tagName === "a") {
      const href = node.properties?.href;
      if (typeof href === "string" && href.startsWith("/") && !href.startsWith("//")) {
        node.properties.href = baseNoSlash + href;
      }
    }
    if (node.children) for (const child of node.children) prefix(child);
  };
  return (tree) => prefix(tree);
}

// The guide ```rust blocks are also compiled as rustdoc doctests by the
// `vihaco-doctests` crate. Some carry rustdoc "hidden" setup lines (`# …`)
// that make a block self-contained for the compiler but should not appear on
// the page. This plugin applies rustdoc's hidden-line rules to displayed Rust
// code: drop lines whose first non-space char is `#` followed by a space (or a
// bare `#`), and un-escape a leading `##` to `#`. Attribute lines (`#[…]`,
// `#![…]`) are left untouched.
function rehypeStripRustdocHidden() {
  const stripLine = (line) => {
    const lead = line.length - line.trimStart().length;
    const body = line.slice(lead);
    if (body === "#") return null;
    if (body.startsWith("# ")) return null;
    if (body.startsWith("##")) return line.slice(0, lead) + body.slice(1);
    return line;
  };
  const isRust = (node) => {
    const cls = node.properties?.className;
    const arr = Array.isArray(cls) ? cls : cls ? [cls] : [];
    return arr.some((c) => typeof c === "string" && c.startsWith("language-rust"));
  };
  const visit = (node) => {
    if (node.type === "element" && node.tagName === "code" && isRust(node)) {
      for (const child of node.children ?? []) {
        if (child.type === "text") {
          child.value = child.value
            .split("\n")
            .map(stripLine)
            .filter((l) => l !== null)
            .join("\n");
        }
      }
    }
    if (node.children) for (const child of node.children) visit(child);
  };
  return (tree) => visit(tree);
}

export default defineConfig({
  site,
  base,
  trailingSlash: "ignore",
  devToolbar: { enabled: false },
  markdown: {
    // We theme code with highlight.js client-side (see Base.astro) using the
    // same design tokens as the rest of the page, so disable Astro's build-time
    // highlighter. Fenced blocks still render as `<pre><code class="language-…">`,
    // which is exactly what the client highlighter opts in on.
    syntaxHighlight: false,
    rehypePlugins: [rehypeStripRustdocHidden, rehypeBaseLinks],
  },
});
