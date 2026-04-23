export type FormattingActionId =
  | "h1"
  | "h2"
  | "h3"
  | "bold"
  | "italic"
  | "strikethrough"
  | "inlineCode"
  | "codeBlock"
  | "checklist"
  | "divider";

export interface TextSelection {
  from: number;
  to: number;
}

export interface FormattingEdit {
  from: number;
  to: number;
  insert: string;
  selection: TextSelection;
}

function lineBounds(text: string, position: number) {
  const lineStart = text.lastIndexOf("\n", Math.max(0, position - 1)) + 1;
  const nextBreak = text.indexOf("\n", position);
  const lineEnd = nextBreak === -1 ? text.length : nextBreak;
  return { lineStart, lineEnd };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function wrapInline(
  text: string,
  selection: TextSelection,
  marker: string,
): FormattingEdit {
  const selected = text.slice(selection.from, selection.to);
  const insert = `${marker}${selected}${marker}`;

  if (selection.from === selection.to) {
    const cursor = selection.from + marker.length;
    return {
      from: selection.from,
      to: selection.to,
      insert,
      selection: { from: cursor, to: cursor },
    };
  }

  return {
    from: selection.from,
    to: selection.to,
    insert,
    selection: {
      from: selection.from + marker.length,
      to: selection.to + marker.length,
    },
  };
}

function replaceLinePrefix(
  text: string,
  selection: TextSelection,
  nextPrefix: string,
  currentPrefixPattern: RegExp,
): FormattingEdit {
  const { lineStart, lineEnd } = lineBounds(text, selection.from);
  const line = text.slice(lineStart, lineEnd);
  const indent = line.match(/^\s*/)?.[0] ?? "";
  const content = line.slice(indent.length).replace(currentPrefixPattern, "");
  const insert = `${indent}${nextPrefix}${content}`;
  const oldPrefixLength = line.length - content.length;
  const newPrefixLength = insert.length - content.length;
  const shift = newPrefixLength - oldPrefixLength;

  return {
    from: lineStart,
    to: lineEnd,
    insert,
    selection: {
      from: clamp(selection.from + shift, lineStart + newPrefixLength, lineStart + insert.length),
      to: clamp(selection.to + shift, lineStart + newPrefixLength, lineStart + insert.length),
    },
  };
}

function insertDivider(text: string, selection: TextSelection): FormattingEdit {
  const { lineStart } = lineBounds(text, selection.from);
  const before = lineStart === 0
    ? ""
    : text.slice(Math.max(0, lineStart - 2), lineStart) === "\n\n"
      ? ""
      : "\n";
  const after = text.slice(lineStart, lineStart + 2) === "\n\n" ? "" : "\n\n";
  const insert = `${before}---${after}`;
  const cursor = lineStart + insert.length;

  return {
    from: lineStart,
    to: lineStart,
    insert,
    selection: { from: cursor, to: cursor },
  };
}

function insertCodeBlock(text: string, selection: TextSelection): FormattingEdit {
  const selected = text.slice(selection.from, selection.to);

  if (selection.from === selection.to) {
    const insert = "```\n\n```";
    const cursor = selection.from + 4;
    return {
      from: selection.from,
      to: selection.to,
      insert,
      selection: { from: cursor, to: cursor },
    };
  }

  const insert = `\`\`\`\n${selected}\n\`\`\``;
  return {
    from: selection.from,
    to: selection.to,
    insert,
    selection: {
      from: selection.from + 4,
      to: selection.from + 4 + selected.length,
    },
  };
}

export function getFormattingEdit(
  action: FormattingActionId,
  text: string,
  selection: TextSelection,
): FormattingEdit {
  switch (action) {
    case "bold":
      return wrapInline(text, selection, "**");
    case "italic":
      return wrapInline(text, selection, "*");
    case "strikethrough":
      return wrapInline(text, selection, "~~");
    case "inlineCode":
      return wrapInline(text, selection, "`");
    case "codeBlock":
      return insertCodeBlock(text, selection);
    case "divider":
      return insertDivider(text, selection);
    case "h1":
      return replaceLinePrefix(text, selection, "# ", /^#{1,6}\s+/);
    case "h2":
      return replaceLinePrefix(text, selection, "## ", /^#{1,6}\s+/);
    case "h3":
      return replaceLinePrefix(text, selection, "### ", /^#{1,6}\s+/);
    case "checklist":
      return replaceLinePrefix(text, selection, "- [ ] ", /(?:[-*+]\s+)?(?:\[\s\]\s+)?/);
  }
}

export function applyFormattingEdit(text: string, edit: FormattingEdit) {
  return `${text.slice(0, edit.from)}${edit.insert}${text.slice(edit.to)}`;
}
