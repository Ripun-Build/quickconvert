// Actions for the selected files (SPECS §5): type to filter, ↑/↓ + Enter, or 1–9.
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Search } from "lucide-react";
import { ActionRow } from "../components/ActionRow";
import { buildMenu, filterMenu, flatActions, withLabel, type ActionDef, type InputKind } from "../lib/actions";
import { convertLabel } from "../lib/office";
import type { FileSet } from "../lib/types";

const LIST_ID = "action-list";
const rowId = (i: number) => `action-${i}`;

interface Props {
  set: FileSet;
  onRun: (action: ActionDef, scope?: InputKind) => void;
}

export function ActionList({ set, onRun }: Props) {
  const [query, setQuery] = useState("");
  const [highlight, setHighlight] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollable, setScrollable] = useState(false);

  const menu = useMemo(
    () =>
      withLabel(
        buildMenu(set.files.map((f) => f.kind)),
        "office_to_pdf",
        convertLabel(set.files.filter((f) => f.kind === "document")),
      ),
    [set],
  );
  const filtered = useMemo(() => filterMenu(menu, query), [menu, query]);
  const flat = useMemo(() => flatActions(filtered), [filtered]);
  const scopeOf = (action: ActionDef) =>
    filtered.sections.find((s) => s.items.some((i) => i.enabled && i.action === action))?.scope;

  useEffect(() => setHighlight(0), [query, set]);

  useEffect(() => {
    document.getElementById(rowId(highlight))?.scrollIntoView({ block: "nearest" });
  }, [highlight]);

  useLayoutEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const check = () => setScrollable(el.scrollHeight > el.clientHeight + 1);
    check();
    const observer = new ResizeObserver(check);
    observer.observe(el);
    return () => observer.disconnect();
  }, [flat.length]);

  const run = (i: number) => {
    const action = flat[i];
    if (action) onRun(action, scopeOf(action));
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (flat.length === 0) return;
      const step = e.key === "ArrowDown" ? 1 : -1;
      setHighlight((h) => (h + step + flat.length) % flat.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(highlight);
    } else if (e.key === "Escape" && query) {
      e.stopPropagation(); // Esc clears the search (it never closes the popup)
      setQuery("");
    } else if (!query && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      run(Number(e.key) - 1);
    }
  };

  let index = -1;
  return (
    <div className="flex min-h-0 flex-col">
      <div className="px-3 pb-2">
        <label className="flex h-8 items-center gap-2 rounded-control bg-surface px-2 focus-within:ring-[3px] focus-within:ring-focus-ring">
          <Search size={14} strokeWidth={1.75} className="shrink-0 text-tertiary" />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="Type an action…"
            aria-label="Search actions"
            aria-controls={LIST_ID}
            aria-activedescendant={flat.length ? rowId(highlight) : undefined}
            className="type-body min-w-0 flex-1 bg-transparent text-primary outline-none placeholder:text-tertiary"
          />
        </label>
      </div>

      <div
        ref={listRef}
        id={LIST_ID}
        role="listbox"
        aria-label="Actions"
        // Keep keyboard focus in the search field when rows are clicked.
        onMouseDown={(e) => e.preventDefault()}
        className={`min-h-0 overflow-y-auto px-2 pb-2 ${scrollable ? "scroll-fade" : ""}`}
      >
        {menu.incompatible && !query && (
          <p className="type-caption px-[10px] pb-1 text-secondary">These files can't be converted together.</p>
        )}
        {filtered.sections.length === 0 && <p className="type-body px-[10px] py-2 text-secondary">No matching actions</p>}
        {filtered.sections.map((section) => (
          <div key={section.title} role="group" aria-label={section.title}>
            <p className="type-caption px-[10px] pt-2 pb-1 tracking-[0.04em] text-tertiary uppercase">
              {section.title}
            </p>
            {section.items.map(({ action, enabled, reason }) => {
              if (!enabled) {
                return (
                  <ActionRow
                    key={`${section.title}-${action.id}`}
                    action={action}
                    highlighted={false}
                    disabledReason={reason}
                  />
                );
              }
              index++;
              const i = index;
              return (
                <ActionRow
                  key={`${section.title}-${action.id}`}
                  id={rowId(i)}
                  action={action}
                  shortcut={!query && i < 9 ? i + 1 : undefined}
                  highlighted={i === highlight}
                  onHover={() => setHighlight(i)}
                  onSelect={() => run(i)}
                />
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
