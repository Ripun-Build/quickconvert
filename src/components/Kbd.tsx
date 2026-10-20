// Shortcut key cap (DESIGN §3): caption, tertiary, 18×18, radius 5.
export function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="type-caption flex size-[18px] shrink-0 items-center justify-center rounded-kbd bg-surface text-tertiary tabular-nums">
      {children}
    </kbd>
  );
}
