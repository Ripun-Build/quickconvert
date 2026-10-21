// Building blocks for the Settings window, in the style of macOS System Settings:
// grouped cards, label-left / control-right rows, and switches.

export function Group({ title, children }: { title?: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-1.5">
      {title && <h3 className="type-caption px-1 text-secondary">{title}</h3>}
      <div className="flex flex-col divide-y divide-separator rounded-card bg-surface inset-ring inset-ring-separator">
        {children}
      </div>
    </section>
  );
}

export function Row({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex min-h-11 items-center justify-between gap-4 px-3 py-2">
      <div className="min-w-0">
        <p className="type-body text-primary">{label}</p>
        {description && <p className="type-caption mt-0.5 text-tertiary">{description}</p>}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </div>
  );
}

export function Switch({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-5 w-[34px] shrink-0 rounded-pill transition-colors duration-150 ${checked ? "bg-accent" : "bg-tertiary"}`}
    >
      <span
        className={`absolute top-0.5 left-0.5 size-4 rounded-pill bg-on-accent shadow-control transition-transform duration-150 ${
          checked ? "translate-x-3.5" : ""
        }`}
      />
    </button>
  );
}
