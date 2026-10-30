// mdBook has no mermaid support of its own: swap each ```mermaid block for a div mermaid draws.
import("https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.esm.min.mjs").then(({ default: mermaid }) => {
  const dark = ["coal", "navy", "ayu"].some((theme) => document.documentElement.classList.contains(theme));
  mermaid.initialize({ startOnLoad: false, theme: dark ? "dark" : "default" });
  const nodes = [...document.querySelectorAll("code.language-mermaid")].map((code) => {
    const div = document.createElement("div");
    div.className = "mermaid";
    div.textContent = code.textContent;
    code.closest("pre").replaceWith(div);
    return div;
  });
  if (nodes.length) mermaid.run({ nodes });
});
