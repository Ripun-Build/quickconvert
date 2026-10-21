// Buttons (DESIGN §4): primary = accent fill, secondary = surface. Pressed scales to 0.97.
type Props = React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" };

export function Button({ variant = "secondary", className = "", type = "button", ...rest }: Props) {
  const look =
    variant === "primary"
      ? "bg-accent-fill text-on-accent active:bg-accent-fill-pressed"
      : "bg-surface text-primary inset-ring inset-ring-separator active:bg-surface-hover";
  return (
    <button
      type={type}
      className={`type-body-strong flex h-[30px] items-center justify-center gap-1.5 rounded-control px-3 transition-transform duration-75 active:scale-[0.97] disabled:opacity-40 ${look} ${className}`}
      {...rest}
    />
  );
}
