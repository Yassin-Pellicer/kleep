import type { Root } from "hast";

const WORD_CLASS = "chat-word";

/**
 * Splits Markdown text nodes into word spans so each word can fade in with a
 * small stagger. Text inside code blocks is left untouched.
 */
export function rehypeFadeWords() {
  return (tree: Root) => {
    let wordIndex = 0;

    const walk = (node: any) => {
      if (!Array.isArray(node.children)) return;

      const nextChildren: any[] = [];

      for (const child of node.children) {
        if (child.type === "text") {
          if (
            node.type === "element" &&
            (node.tagName === "code" || node.tagName === "pre")
          ) {
            nextChildren.push(child);
            continue;
          }

          for (const part of child.value.split(/(\s+)/)) {
            if (!part) continue;

            if (/^\s+$/.test(part)) {
              nextChildren.push({ type: "text", value: part });
              continue;
            }

            nextChildren.push({
              type: "element",
              tagName: "span",
              properties: {
                className: [WORD_CLASS],
                style: `--word-index: ${wordIndex}`,
              },
              children: [{ type: "text", value: part }],
            });
            wordIndex += 1;
          }

          continue;
        }

        if (child.type === "element") {
          walk(child);
        }
        nextChildren.push(child);
      }

      node.children = nextChildren;
    };

    walk(tree);
  };
}
